#![feature(lazy_cell)]
#![feature(thread_id_value)]

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

const END: usize = 10;
const SLEEP_INTERVAL: Duration = Duration::from_millis(2);

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

fn thread_func(i: usize, counter: Arc<AtomicU32>) {
    timeloop::start_thread!();

    timeloop::time_work!("Increment counter", {
        for _ in 0..i {
            counter.fetch_add(1, Ordering::SeqCst);
        }
    });

    top();

    timeloop::stop_thread!();
}

fn main() {
    let start = std::time::Instant::now();
    let counter = Arc::new(AtomicU32::new(0));
    let num_threads = 16;
    let iters = 8;

    timeloop::start_profiler!();

    for _ in 0..iters {
        let mut threads =
            timeloop::time_work!("Allocate thread vec", { Vec::with_capacity(num_threads) });

        for _ in 0..num_threads {
            let counter = timeloop::time_work!("Clone counter", { counter.clone() });

            let t = timeloop::time_work!("Spawn Thread", {
                std::thread::spawn(move || thread_func(1000, counter))
            });

            timeloop::time_work!("Push Thread", {
                threads.push(t);
            });
        }

        timeloop::time_work!("Join Threads", {
            for thread in threads {
                let _ = thread.join();
            }
        });
    }

    // Print the timer state
    timeloop::print!();

    println!("Time: {:?}", start.elapsed());
    println!("Counter: {:?}", counter.load(Ordering::SeqCst));
}
