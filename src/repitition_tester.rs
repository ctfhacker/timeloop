//! Implements `RepitionTester`
use std::time::{Duration, Instant};

enum TestingState {
    Uninit,
    Running,
    Error(&'static str),
}

/// The results
#[derive(Copy, Clone, Debug)]
pub struct TestResults {
    /// Number of times the test was executed
    count: u64,

    /// Total time the tests took
    total_time: u64,

    /// The longest test time
    max_time: u64,

    /// The shortest test time
    min_time: u64,
}

impl Default for TestResults {
    fn default() -> Self {
        Self {
            count: 0,
            total_time: 0,
            max_time: 0,
            min_time: u64::MAX,
        }
    }
}

pub struct RepititionTester {
    /// The current state of this tester
    state: TestingState,

    /// How long to run the tests
    duration: Duration,

    /// The current timer used to track when to stop the test
    start_time: Instant,

    /// Number of times this tester has been started
    start_count: u64,

    /// Number of times this tester has been stopped
    stop_count: u64,

    /// The time taken by the current test
    elapsed_time: u64,

    /// The results of this current test
    pub results: TestResults,
}

fn rdtsc() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

impl RepititionTester {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        Self {
            state: TestingState::Running,
            duration,
            start_time: std::time::Instant::now(),
            start_count: 0,
            stop_count: 0,
            elapsed_time: 0,
            results: TestResults::default(),
        }
    }

    pub fn is_testing(&mut self) -> bool {
        if !matches!(self.state, TestingState::Running) {
            return false;
        }

        if self.start_time.elapsed() >= self.duration {
            return false;
        }

        if self.start_count > 0 {
            if self.start_count != self.stop_count {
                self.state = TestingState::Error("Unmatched start and stop blocks");
            }

            if matches!(self.state, TestingState::Running) {
                // Increment the number of tests
                self.results.count += 1;

                // Add this test's time to the total overall time
                self.results.total_time += self.elapsed_time;

                if self.elapsed_time < self.results.min_time {
                    self.results.min_time = self.elapsed_time;
                    self.start_time = Instant::now();
                    // println!("New min! {:?}", self.results.min_time);
                }

                if self.elapsed_time > self.results.max_time {
                    self.results.max_time = self.elapsed_time;
                }

                self.start_count = 0;
                self.stop_count = 0;
                self.elapsed_time = 0;
            }
        }

        // Keep testing!
        true
    }

    pub fn start(&mut self) {
        self.start_count += 1;
        self.elapsed_time = self.elapsed_time.wrapping_sub(rdtsc());
    }

    pub fn stop(&mut self) {
        self.stop_count += 1;
        self.elapsed_time = self.elapsed_time.wrapping_add(rdtsc());
    }
}
