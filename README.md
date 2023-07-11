# Timeloop

An attempt to make a library out of the benchmark code I repeatedly write.

Created during the [Performance Aware Programming](https://computerenhance.com) series by
Casey Muratori.

## Output

```
Calculated OS frequency: 3911972350
Total time: 600.22ms (2348036270 cycles)
    TIMER | HITS | TIMES
   Phase1 | 1    |      391468413 cycles 16.67% ( 16.67% total time with child timers)
   Phase2 | 1    |      782703049 cycles 33.33% ( 33.33% total time with child timers)
   Phase3 | 1    |     1173862351 cycles 49.99% ( 49.99% total time with child timers)
Remainder |      |           2457 cycles  0.00%
```

## Example program

```rust
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
```
