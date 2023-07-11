#![feature(lazy_cell)]
use timeloop::Profiler;

timeloop::impl_enum!(
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum BasicTimers {
        Phase1,
        Phase2,
        Phase3,
    }
);

const STACK_SIZE: usize = 16;
timeloop::create_profiler!(BasicTimers, STACK_SIZE);

fn main() {
    timeloop::start!(BasicTimers::Phase1);
    std::thread::sleep_ms(100);
    timeloop::stop!(BasicTimers::Phase1);

    {
        timeloop::scoped_timer!(BasicTimers::Phase2);
        std::thread::sleep_ms(200);
    }

    {
        timeloop::scoped_timer!(BasicTimers::Phase3);
        std::thread::sleep_ms(300);
    }

    // Print the timer state
    timeloop::print!();
}
