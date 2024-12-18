#![allow(incomplete_features)]
#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(stmt_expr_attributes)]
#![feature(generic_const_exprs)]
#![feature(inline_const)]
#![feature(let_chains)]

use std::collections::BTreeMap;
use std::fmt::Debug;
use std::io::Read;
use std::time::{Duration, Instant};

mod macros;
pub use macros::*;

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

const MAX_TIMERS: usize = 128;

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
}

/// Get the page faults from the current process
#[must_use]
pub fn get_page_faults() -> u64 {
    let mut proc_stat = [0u8; 0x400];

    let mut file = std::fs::File::open("/proc/self/stat").expect("Cannot open /proc/self/stat");

    // Read the file into the stack buffer
    file.read(&mut proc_stat)
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

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

/// The current thread timer status
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ThreadTimerStatus {
    Stopped,
    Running,
}

const REMAINING_TIME_LABEL: &'static str = "Remainder";

impl<const THREADS: usize> Profiler<THREADS> {
    /// Create a new timer struct
    pub const fn new() -> Self {
        Self {
            thread_times: [0; THREADS],
            thread_status: [ThreadTimerStatus::Stopped; THREADS],
            timer_name_to_index: BTreeMap::new(),
            next_index: 0,
            timers: [[Timer::const_default(); MAX_TIMERS]; THREADS],
            timer_names: [""; MAX_TIMERS],
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
        self.timer_names[curr_index as usize] = timer_name;
        curr_index as usize
    }

    pub fn get_timer(&mut self, thread_id: usize, timer: &'static str) -> &Timer {
        let index = self.get_timer_index(timer);

        if let Some(timers) = self.timers.get(thread_id) {
            &timers[index]
        } else {
            panic!("Unknown thread id: {thread_id}");
        }
    }

    pub fn get_timer_mut(&mut self, thread_id: usize, timer: &'static str) -> &mut Timer {
        let index = self.get_timer_index(timer);

        if let Some(timers) = self.timers.get_mut(thread_id) {
            &mut timers[index]
        } else {
            panic!("Unknown thread id: {thread_id}");
        }
    }

    /// Start the timer for the given thread
    #[inline(always)]
    pub fn start(&mut self, thread_id: usize) {
        if self.thread_status[thread_id] != ThreadTimerStatus::Stopped {
            println!("Attempted to start an already started timer on thread {thread_id}");
        }

        self.thread_times[thread_id] = self.thread_times[thread_id].wrapping_sub(rdtsc());
        self.thread_status[thread_id] = ThreadTimerStatus::Running;
    }

    /// Stop the timer for the given thread
    #[inline(always)]
    pub fn stop(&mut self, thread_id: usize) {
        if self.thread_status[thread_id] != ThreadTimerStatus::Running {
            println!("Attempted to stop an already stopped timer {thread_id}");
        }

        self.thread_times[thread_id] = self.thread_times[thread_id].wrapping_add(rdtsc());
        self.thread_status[thread_id] = ThreadTimerStatus::Stopped;
    }

    /// Print a basic percentage-based status of the timers state
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

            for timer_index in 0..MAX_TIMERS {
                let Timer {
                    inclusive_time,
                    exclusive_time,
                    hits,
                    bytes_processed,
                } = self.timers[thread_id][timer_index];

                // Add the current timer to the accumulated timer
                acc[timer_index].inclusive_time += inclusive_time;
                acc[timer_index].exclusive_time += exclusive_time;
                acc[timer_index].hits += hits;
                acc[timer_index].bytes_processed += bytes_processed;
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

        let mut other = total_time_cycles as isize;

        // Calculate the maximum width of the hits column
        let mut hit_width = "HITS".len();
        for Timer { hits, .. } in acc.iter() {
            hit_width = hit_width.max(format!("{hits}").len());
        }

        let mut not_hit = Vec::new();
        let mut results = Vec::new();

        for (i, timer) in acc.iter().enumerate() {
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

            other -= exclusive_time as isize;
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

            let name = format!("{}", self.timer_names[i]);
            let name = name[..name.len().min(variant_length)].to_string();

            results.push(TimerResult {
                name,
                hits,
                exclusive_time,
                percent,
                inclusive_time_str,
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
                "{name:<width$} | {hits:<hit_width$} | {exclusive_time:14.2?} cycles {percent:6.2}% | {inclusive_time_str} {throughput_str}",
                width = variant_length,
                hit_width = hit_width
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
fn calculate_os_frequency() -> f64 {
    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    let clock_start = rdtsc();
    while start.elapsed() < timeout {}
    let clock_end = rdtsc();

    (clock_end - clock_start) as f64 / timeout.as_secs_f64()
}
