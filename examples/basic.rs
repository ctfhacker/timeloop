#![feature(thread_id_value)]

use std::time::Duration;

timeloop::create_profiler!();

fn main() {
    // Start the global timer for the profiler
    timeloop::start_profiler!();

    let faults = timeloop::get_page_faults();
    println!("Page faults: {faults}");

    // Example of the work! macro
    timeloop::time_work!("phase1", {
        std::thread::sleep(Duration::from_millis(100));
    });

    // Example of the scoped_timer! macro
    {
        timeloop::scoped_timer!("phase2");
        std::thread::sleep(Duration::from_millis(200));
    }

    // Example of the work! macro returning a value
    let value = timeloop::time_work!("phase3", {
        std::thread::sleep(Duration::from_millis(300));
        10
    });

    // Print the timer state
    timeloop::print!();

    println!("Value: {value}");
}
