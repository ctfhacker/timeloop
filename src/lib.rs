#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(generic_const_exprs)]

use std::fmt::Debug;
use std::mem::variant_count;
use std::time::{Duration, Instant};

#[cfg(not(any(feature = "enable", feature = "disable")))]
compile_error!("Turn on the `enable` or `disable` feature");

/// Macro for creating various functions needed for the profiler
/// over the enum of profile points, such as Into<usize> and TryFrom<usize>
#[macro_export]
macro_rules! impl_enum {
    (   // Base case of an enum that we want only one item of
        $(#[$attr:meta])*
        pub enum $name:ident {
            $(
                $(#[$inner:ident $($args:tt)*])*
                $field:vis $var_name:ident,
            )* $(,)?
        }
    ) => {
        $(#[$attr])*
        #[allow(non_camel_case_types)]
        pub enum $name {
            $(
                $(#[$inner $($args)*])*
                $field $var_name,
            )*
        }

        impl Into<usize> for $name {
            fn into(self) -> usize {
                self as usize
            }
        }

        impl TryFrom<usize> for $name {
            type Error = &'static str;
            fn try_from(value: usize) -> Result<Self, Self::Error> {
                // Dummy starting block that is not used
                if value == 0x1337_1337_1337_1337 {
                    // Not used
                    unreachable!()
                }

                $(
                    else if $name::$var_name as usize == value {
                        return Ok($name::$var_name);
                    }
                )*

                else {
                    Err("Unknown value")
                }
            }
        }
    }
}

#[macro_export]
macro_rules! create_profiler {
    ($timer_kind:ident, $stack_size:expr) => {
        use std::sync::{Arc, LazyLock, Mutex};

        // Create the static profiler
        static TIMELOOP_TIMER: LazyLock<Arc<Mutex<timeloop::Timer<$timer_kind, $stack_size>>>> =
            LazyLock::new(|| {
                Arc::new(Mutex::new(
                    timeloop::Timer::<BasicTimers, $stack_size>::new(),
                ))
            });

        pub struct _ScopedTimer {
            timer: $timer_kind,
        }

        impl _ScopedTimer {
            fn new(timer: $timer_kind) -> Self {
                timeloop::start!(timer);

                _ScopedTimer { timer }
            }
        }

        impl Drop for _ScopedTimer {
            fn drop(&mut self) {
                timeloop::stop!(self.timer);
            }
        }
    };
}

#[macro_export]
macro_rules! start {
    ($timer:expr) => {
        #[cfg(feature = "enable")]
        crate::TIMELOOP_TIMER.lock().unwrap().start($timer);
    };
}

#[macro_export]
macro_rules! stop {
    ($timer:expr) => {
        #[cfg(feature = "enable")]
        crate::TIMELOOP_TIMER.lock().unwrap().stop($timer);
    };
}

#[macro_export]
macro_rules! print {
    () => {
        #[cfg(feature = "enable")]
        crate::TIMELOOP_TIMER.lock().unwrap().print();
    };
}

#[macro_export]
macro_rules! scoped_timer {
    ($timer:expr) => {
        #[cfg(feature = "enable")]
        let _timer = _ScopedTimer::new($timer);
    };
}

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
pub struct Timer<T, const STACK_SIZE: usize>
where
    [(); variant_count::<T>()]:,
    T: std::fmt::Debug,
{
    /// The calculated OS timer frequency
    os_timer_freq: f64,

    /// The global starting time for this set of timers
    start_time: u64,

    /// The maximum length of the variants used for padding
    variant_length: usize,

    /// Start time of the entire structure
    total_time: u64,

    /// The start times for the current timers
    start_timers: [u64; variant_count::<T>()],

    /// The elapsed times for the current timers
    elapsed_timers: [u64; variant_count::<T>()],

    /// The total times for the current timers (including sub timers)
    total_timers: [u64; variant_count::<T>()],

    /// The stop times for the current timers
    total_stop_timers: [u64; variant_count::<T>()],

    /// Stack of subtimers which might call each other
    call_stack: [Option<T>; STACK_SIZE],

    /// Current index to write to the call stack
    call_stack_index: usize,
}

#[inline(always)]
fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

const REMAINING_TIME_LABEL: &'static str = "Remainder";

impl<T, const STACK_SIZE: usize> Timer<T, STACK_SIZE>
where
    [(); variant_count::<T>()]:,
    T: Debug + Copy + Clone + PartialEq + Eq,
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

        // Calculate the longest subtimer variant name
        for i in 0..variant_count::<T>() {
            let Ok(timer) = T::try_from(i) else {
                continue;
            };

            // Update the variant length to be the maximum length (capped at 60 chars)
            variant_length = variant_length.max(format!("{timer:?}").len()).min(60);
        }

        Self {
            os_timer_freq,
            variant_length,
            total_time: 0,
            start_timers: [0; variant_count::<T>()],
            elapsed_timers: [0; variant_count::<T>()],
            total_timers: [0; variant_count::<T>()],
            total_stop_timers: [0; variant_count::<T>()],
            start_time: rdtsc(),
            call_stack: [None; STACK_SIZE],
            call_stack_index: 0,
        }
    }

    /// Start the given timer
    pub fn start(&mut self, timer: T) {
        // If there is already a timer running, add the current elapsed
        // time for that timer. For example, if timer A starts, and then
        // timer B starts without finishing A:
        //
        // Timer A starts --> +
        //                    | \
        //                    |  -- Calculate this current elapsed time for A
        //                    | /
        // Timer B starts --> +
        // Timer B stops  --> +
        //                    |
        // Timer A stops  --> +
        if let Some(parent) = self.call_stack[self.call_stack_index.saturating_sub(1)] {
            let parent: usize = parent.into();
            let curr_time = rdtsc() - self.start_timers[parent];

            self.elapsed_timers[parent] += curr_time;
        }

        // Start this timer
        self.start_timers[timer.into()] = rdtsc();

        // If this is the first time seeing this timer, add this time as the start of this
        // block
        if self.total_timers[timer.into()] == 0 {
            self.total_timers[timer.into()] = self.start_timers[timer.into()];
        }

        // Add this timer to the call stack
        assert!(
            self.call_stack_index < STACK_SIZE,
            "Too deep call stack. Increase your Timer::<_, STACK_SIZE> value."
        );

        // Add the current timer to the call stack
        self.call_stack[self.call_stack_index] = Some(timer);
        self.call_stack_index += 1;
    }

    /// Stop the given timer
    pub fn stop(&mut self, timer: T) {
        // Ensure there was at least a timer started
        assert!(
            self.call_stack_index > 0,
            "Tried to stop timer {timer:?} without starting it."
        );

        let Some(parent) = self.call_stack[self.call_stack_index - 1] else {
            panic!("Attempt to stop timer {timer:?} without starting it.");
        };

        if parent != timer {
            panic!(
                "Mis-match start/stop pairs. Tried to stop {timer:?}, but found timer {parent:?}"
            );
        }

        // Update the stop timer for this timer
        self.total_stop_timers[timer.into()] = rdtsc();

        // Add the elapsed time to this current timer and the total time
        let timer_index = timer.into();
        let curr_time = rdtsc() - self.start_timers[timer_index];
        self.elapsed_timers[timer_index] += curr_time;

        // Found a correct start/stop pair. Can pop the stack.
        self.call_stack_index -= 1;
        self.call_stack[self.call_stack_index] = None;

        // Reset the next parent's timer since we returned from it's called block
        if self.call_stack_index > 0 {
            self.call_stack[self.call_stack_index - 1].map(|curr_parent| {
                self.start_timers[curr_parent.into()] = rdtsc();
            });
        }
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
            println!("total: {}", self.total_time);
            using_global_timer = true;
        }

        println!(
            "Total time: {:8.2?} ({} cycles)",
            std::time::Duration::from_secs_f64(self.total_time as f64 / self.os_timer_freq),
            self.total_time
        );

        let mut other = self.total_time as isize;

        for (i, val) in self.elapsed_timers.iter().enumerate() {
            let timer = T::try_from(i).unwrap();

            other -= *val as isize;

            let percent = *val as f64 / self.total_time as f64 * 100.;

            // Include the total time if it was included
            let total_time = {
                let total_time = self.total_stop_timers[i] - self.total_timers[i];

                if total_time > 0 {
                    let total_time_percent = total_time as f64 / self.total_time as f64 * 100.;
                    format!("({total_time_percent:6.2}% total time with child timers)")
                } else {
                    String::new()
                }
            };

            // Print the stats for this timer
            println!(
                "{:<width$} | {val:14.2?} cycles {percent:5.2}% {total_time}",
                format!("{timer:?}"),
                width = self.variant_length
            );
        }

        // Print the remaining
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
