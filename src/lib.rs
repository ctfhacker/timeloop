#![allow(incomplete_features)]
#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(stmt_expr_attributes)]
#![feature(generic_const_exprs)]

use std::fmt::Debug;
use std::io::Read;
use std::mem::variant_count;
use std::time::{Duration, Instant};

mod macros;
pub use macros::*;

mod repitition_tester;
pub use repitition_tester::{RepititionTester, TestResults};

// Check to ensure the profiler is explictly enabled or disabled
#[cfg(not(any(feature = "enable", feature = "disable")))]
compile_error!("Turn on the `enable` or `disable` feature");

/// A timed block
#[derive(Default, Copy, Clone)]
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

/// The provided `Timer` struct that takes an abstract enum with the available subtimers
/// to keep track of
pub struct Profiler<T>
where
    [(); variant_count::<T>()]:,
    T: std::fmt::Debug,
{
    /// The global starting time for this set of timers
    pub start_time: u64,

    /// Current timers available
    pub timers: [Timer; variant_count::<T>()],
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

const REMAINING_TIME_LABEL: &'static str = "Remainder";

impl<T> Profiler<T>
where
    [(); variant_count::<T>()]:,
    T: Debug + Copy + Clone + PartialEq + Eq,
    T: Into<usize>,
    T: TryFrom<usize>,
    <T as TryFrom<usize>>::Error: Debug,
{
    /// Create a new timer struct
    pub const fn new() -> Self {
        Self {
            start_time: 0,
            timers: [Timer {
                inclusive_time: 0,
                exclusive_time: 0,
                hits: 0,
                bytes_processed: 0,
            }; variant_count::<T>()],
        }
    }

    /// Start the timer for the profiler itself
    #[inline(always)]
    pub fn start(&mut self) {
        self.start_time = rdtsc();
    }

    /// Print a basic percentage-based status of the timers state
    pub fn print(&mut self) {
        // Immediately stop the profiler's timer at the beginning of this function
        let stop_time = rdtsc();

        assert!(self.start_time > 0, "Profiler was not started.");

        let os_timer_freq = calculate_os_frequency();
        println!("Calculated OS frequency: {os_timer_freq}");

        let mut variant_length = REMAINING_TIME_LABEL.len();
        let mut hits_col_width = 1;

        // Calculate the longest subtimer variant name
        for i in 0..variant_count::<T>() {
            let Ok(timer) = T::try_from(i) else {
                continue;
            };

            // Update the variant length to be the maximum length (capped at 60 chars)
            variant_length = variant_length.max(format!("{timer:?}").len()).min(60);
        }

        let total_time_cycles = stop_time - self.start_time;
        let total_time_secs = total_time_cycles as f64 / os_timer_freq;

        println!(
            "Total time: {:8.2?} ({total_time_cycles} cycles)",
            std::time::Duration::from_secs_f64(total_time_secs)
        );

        let mut other = total_time_cycles as isize;

        // Calculate the maximum width of the hits column
        let mut hit_width = "HITS".len();
        for Timer { hits, .. } in self.timers.iter() {
            hit_width = hit_width.max(format!("{hits}").len());
        }

        let mut not_hit = Vec::new();
        let mut results = Vec::new();

        for (i, timer) in self.timers.iter().enumerate() {
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

            let name = format!("{:?}", T::try_from(i).unwrap());
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

        println!(
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
            println!(
                "{name:<width$} | {hits:<hit_width$} | {exclusive_time:14.2?} cycles {percent:6.2}% | {inclusive_time_str} {throughput_str}",
                width = variant_length,
                hit_width = hit_width
            );
        }

        // Print the remaining
        println!(
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
