use std::time::Duration;

timeloop::impl_enum!(
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum BasicTimers {
        Phase1,
        Phase2,
        Phase3,
    }
);

timeloop::create_profiler!(BasicTimers);

fn main() {
    // Start the global timer for the profiler
    timeloop::start_profiler!();

    // Example of the work! macro
    timeloop::time_work_with_bandwidth!(BasicTimers::Phase1, 1024 * 1024 * 1024, {
        std::thread::sleep(Duration::from_millis(1000));
    });

    // Print the timer state
    timeloop::print!();
}
