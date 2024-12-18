#![feature(lazy_cell)]
#![feature(thread_id_value)]

use std::time::Duration;

const END: usize = 10;
const SLEEP_INTERVAL: Duration = Duration::from_millis(10);

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

fn thread_func(i: usize) {
    timeloop::start_thread!();

    timeloop::scoped_timer!("Total");

    /*
    for _ in 0..i {
        timeloop::time_work!(BasicTimers::CoreSpecific, {
            std::thread::sleep(SLEEP_INTERVAL);
        });
    }
    */

    top();
    top();

    timeloop::stop_thread!();
}

fn main() {
    timeloop::start_profiler!();

    let start = std::time::Instant::now();

    for k in 0..50 {
        let mut threads = Vec::with_capacity(8);

        for i in 1..=4 {
            let t = timeloop::time_work!("Spawn Thread", {
                std::thread::spawn(move || thread_func(k * 50 + i))
            });

            timeloop::time_work!("Push Thread", {
                threads.push(t);
            });
        }

        timeloop::time_work!("Join Threads", {
            for thread in threads {
                thread.join();
            }
        });
    }

    println!("Time: {:?}", start.elapsed());

    // Print the timer state
    timeloop::print!();
}
