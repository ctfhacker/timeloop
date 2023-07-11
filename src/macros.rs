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
#[cfg(feature = "enable")]
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
#[cfg(feature = "enable")]
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
#[cfg(feature = "enable")]
macro_rules! raw_timer {
    ($timer:expr) => {{
        _ScopedTimer::new($timer)
    }};
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! start_profiler {
    () => {
        unsafe {
            {
                crate::TIMELOOP_PROFILER.start();
            }
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! print {
    () => {
        unsafe {
            crate::TIMELOOP_PROFILER.print();
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! scoped_timer {
    ($timer:expr) => {
        let _timer = _ScopedTimer::new($timer);
    };
}

// Disable feature macros

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! print {
    () => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! work {
    ($timer:expr, $work:expr) => {{
        $work
    }};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! raw_timer {
    ($timer:expr) => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! start_profiler {
    () => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! create_profiler {
    ($timer_kind:ident) => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! scoped_timer {
    ($timer:expr) => {};
}
