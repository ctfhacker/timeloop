#![feature(lazy_cell)]
use std::time::Duration;

const STACK_SIZE: usize = 1024;
const END: usize = 32;
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

timeloop::create_profiler!(BasicTimers, STACK_SIZE);

fn first(val: &mut usize) {
    timeloop::scoped_timer!(BasicTimers::First);

    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    if *val < 2 {
        *val += 1;
        first(val);
    }

    if *val % 2 == 0 {
        *val += 1;
        second(val);
    }

    // timeloop::stop!(BasicTimers::Top);
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
    timeloop::start!(BasicTimers::Total);
    top();
    timeloop::stop!(BasicTimers::Total);

    // Print the timer state
    timeloop::print!();
}
