#![allow(incomplete_features)]
#![feature(variant_count)]
#![feature(stmt_expr_attributes)]
#![feature(generic_const_exprs)]
#![feature(let_chains)]

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Read;
use std::time::{Duration, Instant};

mod macros;

mod repitition_tester;
pub use repitition_tester::{RepititionTester, TestResults};

pub use timeloop_proc_macro::*;

// Check to ensure the profiler is explictly enabled or disabled
#[cfg(not(any(feature = "enable", feature = "disable")))]
compile_error!("Turn on the `enable` or `disable` feature");

/// A timed block
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq)]
pub struct Timer {
    /// The amount of time spent in this timing block (without child blocks)
    pub exclusive_time: u64,

    /// The amount of time spent in this timing block (including child blocks)
    pub inclusive_time: u64,

    /// The number of times this block was hit
    pub hits: u64,

    /// The number of bytes processed in this timing block
    pub bytes_processed: u64,
}

impl Timer {
    const fn const_default() -> Self {
        Self {
            exclusive_time: 0,
            inclusive_time: 0,
            hits: 0,
            bytes_processed: 0,
        }
    }
}

impl std::ops::Add for Timer {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            exclusive_time: self.exclusive_time + rhs.exclusive_time,
            inclusive_time: self.inclusive_time + rhs.inclusive_time,
            hits: self.hits + rhs.hits,
            bytes_processed: self.bytes_processed + rhs.bytes_processed,
        }
    }
}

/// A timed block
#[derive(Default, Clone)]
struct TimerResult {
    pub name: String,
    pub exclusive_time: u64,
    pub inclusive_time_str: String,
    pub hits: u64,
    pub percent: f64,
    pub throughput_str: String,
}

const MAX_TIMERS: usize = 256;

/// The provided `Timer` struct that takes an abstract enum with the available subtimers
/// to keep track of
#[derive(Debug, Clone)]
pub struct Profiler<const THREADS: usize> {
    /// The global elapsed
    pub thread_times: [u64; THREADS],

    /// The status of the the thread indexed timer
    pub thread_status: [ThreadTimerStatus; THREADS],

    /// Timer name mapped to its index
    pub timer_name_to_index: BTreeMap<&'static str, u32>,

    /// The index to allocate for the next timer
    pub next_index: u32,

    pub timer_names: [&'static str; MAX_TIMERS],

    /// Current timers available
    pub timers: [[Timer; MAX_TIMERS]; THREADS],

    /// For each timer, tracks the number of consecutive times it was hit with less than 500 cycles
    pub short_timer_streak: [u32; MAX_TIMERS],
    /// For each timer, tracks the number of times it was hit with less than 500 cycles (for streak logic)
    pub short_timer_hits: [u32; MAX_TIMERS],
    /// Mark timers as ignored (removed from reporting)
    pub ignored_timer: [bool; MAX_TIMERS],
}

/// Get the page faults from the current process
///
/// # Panics
///
/// * Cannot open /proc/self/stat
#[must_use]
pub fn get_page_faults() -> u64 {
    let mut proc_stat = [0u8; 0x400];

    let mut file = std::fs::File::open("/proc/self/stat").expect("Cannot open /proc/self/stat");

    // Read the file into the stack buffer
    let _bytes_read = file
        .read(&mut proc_stat)
        .expect("Failed to read /proc/self/stat");

    // Split the buffer by whitespace and skip to the first page fault entry
    let mut stats = proc_stat.split(|byte| *byte == b' ').skip(9);

    // Parse the minor page faults
    let minor_page_faults = stats.next().unwrap();
    let minor_page_faults = std::str::from_utf8(minor_page_faults).unwrap();
    let minor_page_faults = minor_page_faults.parse::<u64>().unwrap();

    let _cminflt = stats.next().unwrap();

    // Parse the major page faults
    let major_page_faults = stats.next().unwrap();
    let major_page_faults = std::str::from_utf8(major_page_faults).unwrap();
    let major_page_faults = major_page_faults.parse::<u64>().unwrap();

    // Return the faults
    minor_page_faults + major_page_faults
}

fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// The current thread timer status
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThreadTimerStatus {
    Stopped,
    Running,
}

const REMAINING_TIME_LABEL: &str = "Remainder";

impl<const THREADS: usize> Default for Profiler<THREADS> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const THREADS: usize> Profiler<THREADS> {
    /// Create a new timer struct
    #[must_use]
    pub const fn new() -> Self {
        Self {
            thread_times: [0; THREADS],
            thread_status: [ThreadTimerStatus::Stopped; THREADS],
            timer_name_to_index: BTreeMap::new(),
            next_index: 0,
            timers: [[Timer::const_default(); MAX_TIMERS]; THREADS],
            timer_names: [""; MAX_TIMERS],
            short_timer_streak: [0; MAX_TIMERS],
            short_timer_hits: [0; MAX_TIMERS],
            ignored_timer: [false; MAX_TIMERS],
        }
    }

    pub fn get_timer_index(&mut self, timer_name: &'static str) -> usize {
        if let Some(index) = self.timer_name_to_index.get(timer_name) {
            return *index as usize;
        }

        // Not yet seen timer. Add it to the profiler
        let curr_index = self.next_index;
        self.timer_name_to_index.insert(timer_name, curr_index);
        self.next_index += 1;
        *self
            .timer_names
            .get_mut(curr_index as usize)
            .expect("DAFAQ") = timer_name;
        curr_index as usize
    }

    /// Get the timer for a given thread
    ///
    /// # Panics
    ///
    /// If the given `thread_id` is too large
    pub fn get_timer(&mut self, thread_id: usize, timer: &'static str) -> &Timer {
        let index = self.get_timer_index(timer);

        if let Some(timers) = self.timers.get(thread_id) {
            &timers[index]
        } else {
            panic!("Unknown thread id: {thread_id}");
        }
    }

    /// Get the &mut timer for a given thread
    ///
    /// # Panics
    ///
    /// If the given `thread_id` is too large
    pub fn get_timer_mut(&mut self, thread_id: usize, timer: &'static str) -> &mut Timer {
        let index = self.get_timer_index(timer);

        // If this timer is already ignored, just return the timer (but don't update streaks)
        if self.ignored_timer[index] {
            if let Some(timers) = self.timers.get_mut(thread_id) {
                return &mut timers[index];
            } else {
                panic!("Unknown thread id: {thread_id}");
            }
        }

        // Only update streaks if not ignored
        // We'll check the last exclusive_time delta and update the streak logic
        // This logic assumes get_timer_mut is called after a timer is updated (e.g., after a block)
        let timer_ref = if let Some(timers) = self.timers.get_mut(thread_id) {
            &mut timers[index]
        } else {
            panic!("Unknown thread id: {thread_id}");
        };

        // Only check if timer was hit
        if timer_ref.exclusive_time > 0 && timer_ref.hits > 0 {
            // Compute the average cycles per hit for this timer
            let avg_cycles = timer_ref.exclusive_time / timer_ref.hits;
            if avg_cycles < 500 {
                self.short_timer_streak[index] += 1;
                self.short_timer_hits[index] += 1;
                if self.short_timer_streak[index] >= 10 && self.short_timer_hits[index] >= 10 {
                    self.ignored_timer[index] = true;
                    let name = self.timer_names[index];
                    eprintln!("[timeloop] Ignoring timer '{name}' (index {index}) due to short interval (avg {avg_cycles} cycles/hit for 10+ hits)");
                }
            } else {
                self.short_timer_streak[index] = 0;
            }
        }

        timer_ref
    }

    /// Start the timer for the given thread
    pub fn start(&mut self, thread_id: usize) {
        if self.thread_status[thread_id] != ThreadTimerStatus::Stopped {
            println!("Attempted to start an already started timer on thread {thread_id}");
        }

        self.thread_times[thread_id] = self.thread_times[thread_id].wrapping_sub(rdtsc());
        self.thread_status[thread_id] = ThreadTimerStatus::Running;
    }

    /// Stop the timer for the given thread
    pub fn stop(&mut self, thread_id: usize) {
        if self.thread_status[thread_id] != ThreadTimerStatus::Running {
            println!("Attempted to stop an already stopped timer {thread_id}");
        }

        self.thread_times[thread_id] = self.thread_times[thread_id].wrapping_add(rdtsc());
        self.thread_status[thread_id] = ThreadTimerStatus::Stopped;
    }

    /// Print a basic percentage-based status of the timers state
    #[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
    pub fn print(&mut self) {
        // Immediately stop the profiler's timer at the beginning of this function
        let stop_time = rdtsc();

        for thread_id in 0..THREADS {
            // Check if this timer is running and stop it if it is
            if self.thread_status[thread_id] == ThreadTimerStatus::Running {
                eprintln!("Thread {thread_id} was still running during print. Stopping it.");
                self.thread_times[thread_id] = self.thread_times[thread_id].wrapping_add(stop_time);
                self.thread_status[thread_id] = ThreadTimerStatus::Running;
            }
        }

        // Initialize the accumulated timers across all threads
        let mut acc = [Timer::default(); MAX_TIMERS];

        // Fold all of the current timers into the first one
        let mut total_time_cycles = 0;
        for thread_id in 0..THREADS {
            // Ignore thread if it wasn't used
            if self.timers[thread_id].iter().map(|x| x.hits).sum::<u64>() == 0 {
                continue;
            }

            // Get the current thread time
            let thread_time = self.thread_times[thread_id];

            // Add this thread's time to the total time
            total_time_cycles += thread_time;

            for (timer_index, timer) in acc.iter_mut().enumerate() {
                let Timer {
                    inclusive_time,
                    exclusive_time,
                    hits,
                    bytes_processed,
                } = self.timers[thread_id][timer_index];

                // Add the current timer to the accumulated timer
                timer.inclusive_time += inclusive_time;
                timer.exclusive_time += exclusive_time;
                timer.hits += hits;
                timer.bytes_processed += bytes_processed;
            }
        }

        let os_timer_freq = calculate_os_frequency();
        eprintln!("Calculated OS frequency: {os_timer_freq}");

        let mut variant_length = REMAINING_TIME_LABEL.len();
        let mut hits_col_width = 1;

        // Calculate the longest timer name
        let max_timer_name = self.timer_names.iter().map(|x| x.len()).max().unwrap_or(0);

        // Update the variant length to be the maximum length (capped at 60 chars)
        variant_length = variant_length.max(max_timer_name).min(60);

        let total_time_secs = total_time_cycles as f64 / os_timer_freq;

        eprintln!(
            "Total time: {:8.2?} ({total_time_cycles} cycles)",
            std::time::Duration::from_secs_f64(total_time_secs)
        );

        let mut other = total_time_cycles;

        // Calculate the maximum width of the hits column
        let mut hit_width = "HITS".len();
        for Timer { hits, .. } in &acc {
            hit_width = hit_width.max(format!("{hits}").len());
        }

        let mut not_hit = Vec::new();
        let mut results = Vec::new();

        for (i, timer) in acc.iter().enumerate() {
            // Ignore timers that are marked as ignored
            if self.ignored_timer[i] {
                continue;
            }
            let Timer {
                inclusive_time,
                exclusive_time,
                hits,
                bytes_processed,
            } = *timer;

            // If this timer wasn't hit, add it to the not hit list
            if hits == 0 {
                not_hit.push(i);
                continue;
            }

            other = other.wrapping_sub(exclusive_time);
            let percent = exclusive_time as f64 / total_time_cycles as f64 * 100.;

            // Include the total time if it was included
            let mut inclusive_time_str = String::new();

            if inclusive_time > 0 {
                let total_time_percent = inclusive_time as f64 / total_time_cycles as f64 * 100.;

                if (total_time_percent - percent).abs() >= 0.1 {
                    inclusive_time_str = format!("({total_time_percent:5.2}% with child timers)");
                }
            }

            let mut throughput_str = String::new();
            if bytes_processed > 0 {
                // const MEGABYTE: f64 = 1024.0 * 1024.0;
                const GIGABYTE: f64 = 1024.0 * 1024.0 * 1024.0;

                let time = inclusive_time as f64 / os_timer_freq;

                let bytes_per_sec = bytes_processed as f64 / time;
                let gbs_per_sec = bytes_per_sec / GIGABYTE;
                throughput_str = format!("{gbs_per_sec:5.3} GBs/sec");
            }

            hits_col_width = hits_col_width.max(format!("{hits}").len());

            let name = self.timer_names[i];
            let name = name[..name.len().min(variant_length)].to_string();

            results.push(TimerResult {
                name,
                exclusive_time,
                inclusive_time_str,
                hits,
                percent,
                throughput_str,
            });
        }

        results.sort_by_key(|timer| timer.exclusive_time);

        eprintln!(
            "{:<width$} | {:^hits_width$}",
            "TIMER",
            "HITS",
            width = variant_length,
            hits_width = hits_col_width
        );

        for TimerResult {
            name,
            hits,
            exclusive_time,
            percent,
            inclusive_time_str,
            throughput_str,
        } in results.iter().rev()
        {
            // Print the stats for this timer
            eprintln!(
                "{name:<variant_length$} | {hits:<hit_width$} | {exclusive_time:14.2?} cycles {percent:6.2}% | {inclusive_time_str} {throughput_str}",
            );
        }

        // Print the remaining
        eprintln!(
            "{:<width$} | {:<hit_width$} | {other:14.2?} cycles {:6.2}%",
            REMAINING_TIME_LABEL,
            "",
            other as f64 / total_time_cycles as f64 * 100.,
            width = variant_length,
            hit_width = hit_width
        );

        /*
        println!("The following timers were not hit");
        for timer in not_hit {
            println!("{:?}", T::try_from(timer).unwrap());
        }
        */
    }
}

/// Calculate the OS frequency by timing a small timeout using `rdtsc`
#[allow(clippy::cast_precision_loss)]
fn calculate_os_frequency() -> f64 {
    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    let clock_start = rdtsc();
    while start.elapsed() < timeout {}
    let clock_end = rdtsc();

    (clock_end - clock_start) as f64 / timeout.as_secs_f64()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    // Helper function to simulate a tiny sleep (busy-wait, since sleep may not be accurate for ns)
    fn sleep_approx_nanos(ns: u64) {
        let start = Instant::now();
        while start.elapsed().as_nanos() < ns as u128 {}
    }

    #[test]
    fn test_short_interval_timer_removal() {
        let mut profiler = Profiler::<1>::default();

        // 10ns function, called 10 times
        for _ in 0..10 {
            let timer = profiler.get_timer_mut(0, "10ns_fn");
            let start = rdtsc();
            sleep_approx_nanos(10);
            let end = rdtsc();
            timer.exclusive_time += end - start;
            timer.hits += 1;
        }

        // 10ms function, called 10 times
        for _ in 0..10 {
            let timer = profiler.get_timer_mut(0, "10ms_fn");
            let start = rdtsc();
            thread::sleep(Duration::from_millis(10));
            let end = rdtsc();
            timer.exclusive_time += end - start;
            timer.hits += 1;
        }

        // 100ms function, called 10 times
        for _ in 0..10 {
            let timer = profiler.get_timer_mut(0, "100ms_fn");
            let start = rdtsc();
            thread::sleep(Duration::from_millis(100));
            let end = rdtsc();
            timer.exclusive_time += end - start;
            timer.hits += 1;
        }

        // After 10 cycles, the 10ns_fn should be ignored
        let idx_10ns = profiler.timer_name_to_index["10ns_fn"] as usize;
        let idx_10ms = profiler.timer_name_to_index["10ms_fn"] as usize;
        let idx_100ms = profiler.timer_name_to_index["100ms_fn"] as usize;

        assert!(profiler.ignored_timer[idx_10ns], "10ns_fn should be ignored");
        assert!(!profiler.ignored_timer[idx_10ms], "10ms_fn should NOT be ignored");
        assert!(!profiler.ignored_timer[idx_100ms], "100ms_fn should NOT be ignored");

        // Optionally, print to visually inspect
        profiler.print();
    }
}
