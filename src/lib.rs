#![feature(variant_count)]
#![feature(generic_const_exprs)]

use std::fmt::Debug;
use std::mem::variant_count;
use std::time::{Duration, Instant};

/// The provided `Timer` struct that takes an abstract enum with the available subtimers
/// to keep track of
///
/// ```rust
/// enum SubTimer {
///     Phase1,
///     Phase2,
///     Phase3
/// }
/// let timer = Timer::<SubTimer>::new();
/// ```
///
#[derive(Debug)]
pub struct Timer<T>
where
    [(); variant_count::<T>()]:,
    T: std::fmt::Debug,
{
    /// The calculated OS timer frequency
    os_timer_freq: f64,

    /// The maximum length of the variants used for padding
    variant_length: usize,

    /// Start time of the entire structure
    total_time: u64,

    /// The start times for the current timers
    start_timers: [u64; variant_count::<T>()],

    /// The elapsed times for the current timers
    elapsed_timers: [u64; variant_count::<T>()],

    /// The global starting time for this set of timers
    start_time: u64,
}

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

const REMAINING_TIME_LABEL: &'static str = "Remainder";

impl<T> Timer<T>
where
    [(); variant_count::<T>()]:,
    T: Debug,
    T: Into<usize>,
    T: TryFrom<usize>,
    <T as TryFrom<usize>>::Error: Debug,
{
    /// Create a new timer struct
    pub fn new() -> Self {
        //
        let timeout = Duration::from_millis(100);

        let start = Instant::now();
        let clock_start = rdtsc();
        while start.elapsed() < timeout {}
        let clock_end = rdtsc();

        let os_timer_freq = (clock_end - clock_start) as f64 / timeout.as_secs_f64();
        println!("Calculated OS frequency: {os_timer_freq}");

        let mut variant_length = REMAINING_TIME_LABEL.len();
        for i in 0..variant_count::<T>() {
            let Ok(timer) = T::try_from(i) else {
                continue;
            };

            // Update the variant length to be the maximum length
            variant_length = variant_length.max(format!("{timer:?}").len());
        }

        Self {
            os_timer_freq,
            variant_length,
            total_time: 0,
            start_timers: [0; variant_count::<T>()],
            elapsed_timers: [0; variant_count::<T>()],
            start_time: rdtsc(),
        }
    }

    /// Start the given timer
    #[inline(always)]
    pub fn start(&mut self, timer: T) {
        self.start_timers[timer.into()] = rdtsc();
    }

    /// Stop the given timer
    #[inline(always)]
    pub fn stop(&mut self, timer: T) {
        let timer_index = timer.into();

        // Add the elapsed time to this current timer and the total time
        let curr_time = rdtsc() - self.start_timers[timer_index];
        self.elapsed_timers[timer_index] += curr_time;
    }

    /// Add to the overall total time of the timer. Used pre-dominately to check
    /// if the current timing setup is missing any pieces of execution.
    #[inline(always)]
    pub fn add_to_total(&mut self, cycles: u64) {
        self.total_time += cycles;
    }

    /// Print a basic percentage-based status of the timers state
    pub fn print(&mut self) {
        // Check if the timer is being checked from the initialization time
        // If so, reset the self.total_time at the end of the print
        let mut using_global_timer = false;

        // Set the current elapsed time
        if self.total_time == 0 {
            self.total_time = rdtsc() - self.start_time;
            using_global_timer = true;
        }

        println!(
            "Total time: {:8.2?} ({} cycles)",
            std::time::Duration::from_secs_f64(self.total_time as f64 / self.os_timer_freq),
            self.total_time
        );

        let mut other = self.total_time;

        for (i, val) in self.elapsed_timers.iter().enumerate() {
            let timer = T::try_from(i).unwrap();

            other -= *val;

            let percent = *val as f64 / self.total_time as f64 * 100.;

            println!(
                "{:<width$} | {val:14.2?} cycles {percent:5.2}%",
                format!("{timer:?}"),
                width = self.variant_length
            );
        }

        println!(
            "{:<width$} | {other:14.2?} cycles {:5.2}%",
            REMAINING_TIME_LABEL,
            other as f64 / self.total_time as f64 * 100.,
            width = self.variant_length
        );

        // Reset the total time if we're relying on the global start time
        if using_global_timer {
            self.total_time = 0;
        }
    }

    /// Print the current state of the timers with a per iteration count
    pub fn print_for_iterations(&self, iters: usize) {
        todo!()
    }
}
