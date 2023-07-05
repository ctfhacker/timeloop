# Timeloop

An attempt to make a library out of the benchmark code I repeatedly write.

Created during the [Performance Aware Programming](https://computerenhance.com) series by
Casey Muratori.

## Output

```
Calculated OS frequency: 3911988490
Total time: 600.44ms (2348925345 cycles)
Phase1    |      391751972 cycles 16.68%
Phase2    |      783002154 cycles 33.33%
Phase3    |     1174170612 cycles 49.99%
Remainder |            607 cycles  0.00%
```

## Example program

```rust
#![feature(lazy_cell)]
use timeloop::Timer;

timeloop::impl_enum!(
    #[derive(Debug, Copy, Clone, Eq, PartialEq)]
    pub enum BasicTimers {
        Phase1,
        Phase2,
        Phase3,
    }
);

// Create the local profiler
const CALL_STACK_SIZE: usize = 16;
timeloop::create_profiler!(BasicTimers, CALL_STACK_SIZE);

fn main() {
    // Start and stop a timer manually
    timeloop::start!(BasicTimers::Phase1);
    std::thread::sleep_ms(100);
    timeloop::stop!(BasicTimers::Phase1);

    // Use the scope to start and stop a timer
    {
        timeloop::scoped_timer!(BasicTimers::Phase2);
        std::thread::sleep_ms(200);
    }

    // Use the scope to start and stop a timer
    {
        timeloop::scoped_timer!(BasicTimers::Phase3);
        std::thread::sleep_ms(300);
    }

    // Print the timer state
    timeloop::print!();
}
```
