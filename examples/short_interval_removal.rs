#![feature(thread_id_value)]

use std::time::Duration;

timeloop::create_profiler!();

fn sleep_approx_nanos(ns: u64) {
    let start = std::time::Instant::now();
    while start.elapsed().as_nanos() < ns as u128 {}
}

fn main() {
    // Start the global timer for the profiler
    timeloop::start_profiler!();

    // 10ns function, called 10 times
    for _ in 0..10 {
        timeloop::time_work!("10ns_fn", {
            sleep_approx_nanos(10);
        });
    }

    // 10ms function, called 10 times
    for _ in 0..10 {
        timeloop::time_work!("10ms_fn", {
            std::thread::sleep(Duration::from_millis(10));
        });
    }

    // 100ms function, called 10 times
    for _ in 0..10 {
        timeloop::time_work!("100ms_fn", {
            std::thread::sleep(Duration::from_millis(100));
        });
    }

    // Print the timer state
    timeloop::print!();
}