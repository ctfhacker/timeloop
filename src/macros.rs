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
        pub const NUM_THREADS: usize = 4096;

        // Create the static profiler
        static mut TIMELOOP_PROFILER: timeloop::Profiler<$timer_kind, NUM_THREADS> =
            timeloop::Profiler::<$timer_kind, NUM_THREADS>::new();

        // The current node being profiled, used to save who called which timer
        static mut PROFILER_PARENT: [Option<$timer_kind>; crate::NUM_THREADS] =
            [None; crate::NUM_THREADS];

        pub struct _ScopedTimer {
            /// This kind of this current timer
            timer: $timer_kind,

            /// The starting time for this timer
            start_time: u64,

            /// The parent of this timer
            parent: Option<$timer_kind>,

            /// The former inclusive time for this type of timer
            old_inclusive_time: u64,

            /// Number of bytes during this timer
            bytes_processed: u64,
        }

        /// Get the ID for the current thread that is guarenteed to be non-zero
        pub fn thread_id() -> usize {
            let thread_id = std::thread::current().id().as_u64().get() as usize;

            assert!(
                thread_id < crate::NUM_THREADS,
                "Too many threads. Increase timeloop::NUM_THREADS"
            );

            extern "C" {
                fn pthread_self() -> u64;
            }

            thread_id
        }

        impl _ScopedTimer {
            fn new(timer: $timer_kind) -> Self {
                _ScopedTimer::_new(timer, 0)
            }

            fn new_with_bandwidth(timer: $timer_kind, bytes_processed: u64) -> Self {
                _ScopedTimer::_new(timer, bytes_processed)
            }

            fn _new(timer: $timer_kind, bytes_processed: u64) -> Self {
                let thread_id = thread_id();

                // Get the parent timer for this new timer
                let parent = unsafe {
                    let parent = PROFILER_PARENT[thread_id];
                    PROFILER_PARENT[thread_id] = Some(timer);
                    parent
                };

                let timer_index: usize = timer.into();

                let old_inclusive_time = unsafe {
                    crate::TIMELOOP_PROFILER.timers[thread_id][timer_index].inclusive_time
                };

                _ScopedTimer {
                    timer,
                    start_time: unsafe { std::arch::x86_64::_rdtsc() },
                    parent,
                    old_inclusive_time,
                    bytes_processed,
                }
            }
        }

        impl Drop for _ScopedTimer {
            fn drop(&mut self) {
                let thread_id = thread_id();

                unsafe {
                    // Reset the current parent node
                    PROFILER_PARENT[thread_id] = self.parent;

                    // Get the timer index for this current timer
                    let timer_index: usize = self.timer.into();

                    // Calculate the elapsed time for this timer
                    let stop_time = unsafe { std::arch::x86_64::_rdtsc() };
                    let elapsed = stop_time - self.start_time;

                    // If there is a parent timer, remove this elapsed time from the parent
                    if let Some(parent) = self.parent {
                        let mut parent_timer =
                            &mut crate::TIMELOOP_PROFILER.timers[thread_id][parent as usize];
                        parent_timer.exclusive_time =
                            parent_timer.exclusive_time.wrapping_sub(elapsed);
                    }

                    let mut curr_timer =
                        &mut crate::TIMELOOP_PROFILER.timers[thread_id][timer_index];

                    // Update this timer's elapsed time
                    curr_timer.exclusive_time = curr_timer.exclusive_time.wrapping_add(elapsed);

                    // Specifically overwritting this timer to always
                    curr_timer.inclusive_time = self.old_inclusive_time + elapsed;

                    // Add this the number of bytes processed by this timer
                    curr_timer.bytes_processed += self.bytes_processed;

                    // Increment the hit count
                    curr_timer.hits += 1;
                }
            }
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! time_work {
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
macro_rules! time_work_with_bandwidth {
    ($timer:expr, $bytes:expr, $work:expr) => {{
        {
            timeloop::scoped_bandwidth_timer!($timer, $bytes);

            let result = $work;
            result
        }
    }};
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! raw_timer {
    ($timer:expr) => {{
        crate::_ScopedTimer::new($timer)
    }};
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! start_thread {
    () => {
        unsafe {
            {
                let thread_id = crate::thread_id();
                crate::TIMELOOP_PROFILER.start(thread_id);
            }
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! start_profiler {
    () => {
        unsafe {
            {
                let thread_id = crate::thread_id();
                crate::TIMELOOP_PROFILER.start(thread_id);
            }
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! stop_thread {
    () => {
        unsafe {
            {
                let thread_id = crate::thread_id();
                crate::TIMELOOP_PROFILER.stop(thread_id);
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
macro_rules! print_with_iterations {
    ($iters:expr) => {
        unsafe {
            crate::TIMELOOP_PROFILER.print_with_iterations($iters);
        }
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! scoped_timer {
    ($timer:expr) => {
        let _timer = crate::_ScopedTimer::new($timer);
    };
}

#[macro_export]
#[cfg(feature = "enable")]
macro_rules! scoped_bandwidth_timer {
    ($timer:expr, $bytes:expr) => {
        let _timer = crate::_ScopedTimer::new_with_bandwidth($timer, $bytes);
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
macro_rules! time_work {
    ($timer:expr, $work:expr) => {{
        {
            let result = $work;
            result
        }
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
macro_rules! start_thread {
    () => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! stop_thread {
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

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! print_with_iterations {
    ($iters:expr) => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! scoped_bandwidth_timer {
    ($timer:expr, $bytes:expr) => {};
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! start_thread {
    () => {
        unsafe {}
    };
}

#[macro_export]
#[cfg(not(feature = "enable"))]
macro_rules! stop_thread {
    () => {
        unsafe {}
    };
}
