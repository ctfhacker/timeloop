#![feature(thread_id_value)]
use std::time::Duration;

timeloop::create_profiler!();

fn main() {
    // Start the global timer for the profiler
    timeloop::start_profiler!();

    // Example of the work! macro
    timeloop::time_work_with_bandwidth!("Phase 1", 1024 * 1024 * 1024, {
        std::thread::sleep(Duration::from_millis(1000));
    });

    // Print the timer state
    timeloop::print!();
}
