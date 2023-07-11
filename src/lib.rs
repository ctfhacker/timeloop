#![feature(lazy_cell)]
#![feature(variant_count)]
#![feature(generic_const_exprs)]
#![feature(stmt_expr_attributes)]

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
    ($timer_kind:ident) => {
        // Create the static profiler
        static mut TIMELOOP_PROFILER: timeloop::Profiler<$timer_kind> =
            timeloop::Profiler::<$timer_kind>::new();

        // The current node being profiled, used to save who called which timer
        static mut PROFILER_PARENT: Option<$timer_kind> = None;

        pub struct _ScopedTimer {
            /// This kind of this current timer
            timer: $timer_kind,

            /// The starting time for this timer
            start_time: u64,

            /// The parent of this timer
            parent: Option<$timer_kind>,

            /// The former inclusive time for this type of timer
            old_inclusive_time: u64,
        }

        impl _ScopedTimer {
            fn new(timer: $timer_kind) -> Self {
                // Get the parent timer for this new timer
                let parent = unsafe {
                    let parent = PROFILER_PARENT;
                    PROFILER_PARENT = Some(timer);
                    parent
                };

                let timer_index: usize = timer.into();

                let old_inclusive_time =
                    unsafe { crate::TIMELOOP_PROFILER.timers[timer_index].inclusive_time };

                _ScopedTimer {
                    timer,
                    start_time: unsafe { core::arch::x86_64::_rdtsc() },
                    parent,
                    old_inclusive_time,
                }
            }
        }

        impl Drop for _ScopedTimer {
            fn drop(&mut self) {
                unsafe {
                    // Reset the current parent node
                    PROFILER_PARENT = self.parent;

                    // Get the timer index for this current timer
                    let timer_index: usize = self.timer.into();

                    // Calculate the elapsed time for this timer
                    let stop_time = unsafe { core::arch::x86_64::_rdtsc() };
                    let elapsed = stop_time - self.start_time;

                    // If there is a parent timer, remove this elapsed time from the parent
                    if let Some(parent) = self.parent {
                        let mut parent_timer =
                            &mut crate::TIMELOOP_PROFILER.timers[parent as usize];
                        parent_timer.exclusive_time -= elapsed;
                    }

                    let mut curr_timer = &mut crate::TIMELOOP_PROFILER.timers[timer_index];

                    // Update this timer's elapsed time
                    curr_timer.exclusive_time += elapsed;

                    // Specifically overwritting this timer to always
                    curr_timer.inclusive_time = self.old_inclusive_time + elapsed;

                    // Increment the hit count
                    curr_timer.hits += 1;
                }
            }
        }
    };
}

#[macro_export]
macro_rules! work {
    ($timer:expr, $work:expr) => {{
        {
            timeloop::scoped_timer!($timer);
            let result = $work;
            result
        }
    }};
}

#[macro_export]
macro_rules! raw_timer {
    ($timer:expr) => {{
        #[cfg(feature = "enable")]
        _ScopedTimer::new($timer)
    }};
}

#[macro_export]
macro_rules! start_profiler {
    () => {
        unsafe {
            #[cfg(feature = "enable")]
            crate::TIMELOOP_PROFILER.start();
        }
    };
}

#[macro_export]
macro_rules! print {
    () => {
        unsafe {
            #[cfg(feature = "enable")]
            crate::TIMELOOP_PROFILER.print();
        }
    };
}

#[macro_export]
macro_rules! scoped_timer {
    ($timer:expr) => {
        #[cfg(feature = "enable")]
        let _timer = _ScopedTimer::new($timer);
    };
}

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

    /// Print the current state of the timers with a per iteration count
    pub fn print_for_iterations(&self, iters: usize) {
        todo!()
    }
}

// Calculate the OS frequency by timing a small timeout using `rdtsc`
fn calculate_os_frequency() -> f64 {
    let timeout = Duration::from_millis(100);
    let start = Instant::now();
    let clock_start = rdtsc();
    while start.elapsed() < timeout {}
    let clock_end = rdtsc();

    (clock_end - clock_start) as f64 / timeout.as_secs_f64()
}
