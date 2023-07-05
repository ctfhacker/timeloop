#![feature(lazy_cell)]
use std::time::Duration;

const STACK_SIZE: usize = 1024;
const END: usize = 128;
const SLEEP_INTERVAL: Duration = Duration::from_millis(50);

timeloop::impl_enum!(
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum BasicTimers {
        Total,
        Top,
        Inner,
    }
);

timeloop::create_profiler!(BasicTimers, STACK_SIZE);

fn top(val: &mut usize) {
    timeloop::scoped_timer!(BasicTimers::Top);

    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    if *val < 2 {
        *val += 1;
        top(val);
    }

    if *val % 2 == 0 {
        *val += 1;
        inner(val);
    }

    // timeloop::stop!(BasicTimers::Top);
}

fn inner(val: &mut usize) {
    timeloop::scoped_timer!(BasicTimers::Inner);

    if *val >= END {
        return;
    }

    std::thread::sleep(SLEEP_INTERVAL);

    if *val % 2 == 1 {
        *val += 1;
        top(val);
    }
}

fn main() {
    timeloop::start!(BasicTimers::Total);
    let mut counter = 0;
    top(&mut counter);
    timeloop::stop!(BasicTimers::Total);

    // Print the timer state
    timeloop::print!();
}
