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
    timeloop::work!(BasicTimers::Phase1, {
        std::thread::sleep(Duration::from_millis(100));
    });

    // Example of the scoped_timer! macro
    {
        timeloop::scoped_timer!(BasicTimers::Phase2);
        std::thread::sleep(Duration::from_millis(200));
    }

    // Example of the work! macro returning a value
    let value = timeloop::work!(BasicTimers::Phase3, {
        std::thread::sleep(Duration::from_millis(300));
        10
    });

    // Print the timer state
    timeloop::print!();

    println!("Value: {value}");
}
