# Timeloop

An attempt to make a library out of the benchmark code I repeatedly write.

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

```
use timeloop::Timer;

#[derive(Debug)]
pub enum BasicTimers {
    Phase1,
    Phase2,
    Phase3,
}

impl Into<usize> for BasicTimers {
    fn into(self) -> usize {
        self as usize
    }
}

impl TryFrom<usize> for BasicTimers {
    type Error = &'static str;
    fn try_from(value: usize) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BasicTimers::Phase1),
            1 => Ok(BasicTimers::Phase2),
            2 => Ok(BasicTimers::Phase3),
            _ => Err("Unknown Timer value"),
        }
    }
}

fn rdtsc() -> u64 {
    unsafe { core::arch::x86_64::_rdtsc() }
}

fn main() {
    let mut timer = Timer::<BasicTimers>::new();
    let total_start = rdtsc();

    timer.start(BasicTimers::Phase1);
    std::thread::sleep_ms(100);
    timer.stop(BasicTimers::Phase1);

    timer.start(BasicTimers::Phase2);
    std::thread::sleep_ms(200);
    timer.stop(BasicTimers::Phase2);

    timer.start(BasicTimers::Phase3);
    std::thread::sleep_ms(300);
    timer.stop(BasicTimers::Phase3);

    let total_time = rdtsc() - total_start;

    // Add to the total time
    timer.add_to_total(total_time);

    // Print the timer state
    timer.print();
}
```
