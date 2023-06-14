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
}

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
        }
    }

    /// Start the given timer
    pub fn start(&mut self, timer: T) {
        self.start_timers[timer.into()] = rdtsc();
    }

    /// Stop the given timer
    pub fn stop(&mut self, timer: T) {
        let timer_index = timer.into();

        // Add the elapsed time to this current timer and the total time
        let curr_time = rdtsc() - self.start_timers[timer_index];
        self.elapsed_timers[timer_index] += curr_time;
        self.total_time += curr_time;
    }

    /// Print a basic percentage-based status of the timers state
    pub fn print(&self) {
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
    }

    /// Print the current state of the timers with a per iteration count
    pub fn print_for_iterations(&self, iters: usize) {
        todo!()
    }
}
