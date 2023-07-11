#![allow(incomplete_features)]
#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(stmt_expr_attributes)]
#![feature(generic_const_exprs)]

use std::fmt::Debug;
use std::mem::variant_count;
use std::time::{Duration, Instant};

mod macros;
pub use macros::*;

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

        // Calculate the longest subtimer variant name
        for i in 0..variant_count::<T>() {
            let Ok(timer) = T::try_from(i) else {
                continue;
            };

            // Update the variant length to be the maximum length (capped at 60 chars)
            variant_length = variant_length.max(format!("{timer:?}").len()).min(60);
        }

        let total_time = stop_time - self.start_time;

        println!(
            "Total time: {:8.2?} ({} cycles)",
            std::time::Duration::from_secs_f64(total_time as f64 / os_timer_freq),
            total_time
        );

        let mut other = total_time as isize;

        // Calculate the maximum width of the hits column
        let mut hit_width = "HITS".len();
        for Timer { hits, .. } in self.timers.iter() {
            hit_width = hit_width.max(format!("{hits}").len());
        }

        println!("{:>width$} | HITS | TIMES", "TIMER", width = variant_length);

        for (i, timer) in self.timers.iter().enumerate() {
            let Timer {
                inclusive_time,
                exclusive_time,
                hits,
            } = *timer;

            other -= exclusive_time as isize;
            let percent = exclusive_time as f64 / total_time as f64 * 100.;

            // Include the total time if it was included
            let inclusive_time_str = {
                if inclusive_time > 0 {
                    let total_time_percent = inclusive_time as f64 / total_time as f64 * 100.;
                    format!("({total_time_percent:6.2}% total time with child timers)")
                } else {
                    String::new()
                }
            };

            // Print the stats for this timer
            println!(
                "{:>width$} | {hits:<hit_width$} | {exclusive_time:14.2?} cycles {percent:5.2}% {inclusive_time_str}",
                format!("{:?}", T::try_from(i).unwrap()),
                width = variant_length,
                hit_width = hit_width
            );
        }

        // Print the remaining
        println!(
            "{:<width$} | {:<hit_width$} | {other:14.2?} cycles {:5.2}%",
            REMAINING_TIME_LABEL,
            "",
            other as f64 / total_time as f64 * 100.,
            width = variant_length,
            hit_width = hit_width
        );
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
