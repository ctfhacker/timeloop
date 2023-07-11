#![feature(lazy_cell)]
use std::time::Duration;

const END: usize = 10;
const SLEEP_INTERVAL: Duration = Duration::from_millis(50);

timeloop::impl_enum!(
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum BasicTimers {
        Total,
        Top,
        First,
        Second,
    }
);

timeloop::create_profiler!(BasicTimers);

fn first(val: &mut usize) {
    timeloop::scoped_timer!(BasicTimers::First);

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

fn second(val: &mut usize) {
    timeloop::scoped_timer!(BasicTimers::Second);

    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    if *val % 2 == 1 {
        *val += 1;
        first(val);
    }
}

fn top() {
    timeloop::scoped_timer!(BasicTimers::Top);

    std::thread::sleep(SLEEP_INTERVAL / 2);

    let mut counter = 0;
    first(&mut counter);
}

fn main() {
    timeloop::start_profiler!();

    let start = std::time::Instant::now();

    timeloop::work!(BasicTimers::Total, {
        top();
        top();
    });

    println!("Time: {:?}", start.elapsed());

    // Print the timer state
    timeloop::print!();
}
