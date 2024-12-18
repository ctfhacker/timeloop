#![feature(thread_id_value)]

use std::time::Duration;

const END: usize = 10;
const SLEEP_INTERVAL: Duration = Duration::from_millis(50);

timeloop::create_profiler!();

#[timeloop::profile]
fn first(val: &mut usize) {
    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    // Force First->First recursion for the first two iterations
    if *val < 2 {
        *val += 1;
        first(val);
    } else if *val % 2 == 0 {
        // Force First->Second->First->Second..ect recursion moving forward
        *val += 1;
        second(val);
    }
}

#[timeloop::profile]
fn second(val: &mut usize) {
    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    if *val % 2 == 1 {
        *val += 1;
        first(val);
    }
}

#[timeloop::profile]
fn top() {
    std::thread::sleep(SLEEP_INTERVAL / 2);

    let mut counter = 0;
    first(&mut counter);
}

fn main() {
    timeloop::start_profiler!();

    let start = std::time::Instant::now();

    timeloop::time_work!("Total", {
        top();
        top();
    });

    println!("Time: {:?}", start.elapsed());

    // Print the timer state
    timeloop::print!();
}
