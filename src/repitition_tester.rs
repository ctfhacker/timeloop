//! Implements `RepitionTester`
use std::time::{Duration, Instant};

enum TestingState {
    // Uninit,
    Running,
    Error(&'static str),
}

/// Statistics for an individual test case
#[derive(Copy, Clone, Debug, Default)]
pub struct TestCase {
    /// The time (in cycles) for this iteration
    time: u64,

    /// The page faults for this iteration
    page_faults: u64,
}

/// The results for a repitition tester
#[derive(Copy, Clone, Debug)]
pub struct TestResults {
    /// Number of times the test was executed
    count: u64,

    /// Total time the tests took (in cycles)
    total_time: u64,

    /// Total number of page faults during the test
    total_page_faults: u64,

    /// The statistics for the longest test case
    max: TestCase,

    /// The statistics for the shortest test case
    min: TestCase,
}

impl Default for TestResults {
    fn default() -> Self {
        Self {
            count: 0,
            total_time: 0,
            total_page_faults: 0,
            max: TestCase {
                time: 0,
                ..Default::default()
            },
            min: TestCase {
                time: u64::MAX,
                ..Default::default()
            },
        }
    }
}

impl TestResults {
    fn _print(&self, byte_count: Option<u64>) {
        // Get the OS frequency
        let os_freq = crate::calculate_os_frequency();

        let min_time_secs = std::time::Duration::from_secs_f64(self.min.time as f64 / os_freq);
        let max_time_secs = std::time::Duration::from_secs_f64(self.max.time as f64 / os_freq);
        let avg_time_secs = std::time::Duration::from_secs_f64(
            self.total_time as f64 / self.count as f64 / os_freq,
        );

        let mut min_byte_per_sec = None;
        let mut max_byte_per_sec = None;
        let mut avg_byte_per_sec = None;

        if let Some(byte_count) = byte_count {
            if self.min.page_faults > 0 {
                min_byte_per_sec = Some(format!(
                    "MB/sec {:8.2?} KB/fault {:8.2}",
                    byte_count as f64 / min_time_secs.as_secs_f64() / 1024. / 1024.,
                    byte_count as f64 / self.min.page_faults as f64 / 1024.
                ));
            }

            if self.max.page_faults > 0 {
                max_byte_per_sec = Some(format!(
                    "MB/sec {:8.2?} KB/fault {:8.2}",
                    byte_count as f64 / max_time_secs.as_secs_f64() / 1024. / 1024.,
                    byte_count as f64 / self.max.page_faults as f64 / 1024.
                ));
            }

            avg_byte_per_sec = Some(format!(
                "MB/sec {:8.2?} KB/fault {:8.2}",
                byte_count as f64 / avg_time_secs.as_secs_f64() / 1024. / 1024.,
                (byte_count * self.count) as f64 / self.total_page_faults as f64 / 1024.
            ));
        };

        println!(
            "Min: {:12?} ({min_time_secs:8.2?}) {} Faults: {:8}",
            self.min.time,
            min_byte_per_sec.unwrap_or_else(|| "".to_string()),
            self.min.page_faults,
        );
        println!(
            "Max: {:12?} ({max_time_secs:8.2?}) {} Faults: {:8}",
            self.max.time,
            max_byte_per_sec.unwrap_or_else(|| "".to_string()),
            self.max.page_faults,
        );
        println!(
            "Avg: {:12.0?} ({avg_time_secs:8.2?}) {} Faults: {:8}",
            self.total_time as f64 / self.count as f64,
            avg_byte_per_sec.unwrap_or_else(|| "".to_string()),
            self.total_page_faults / self.count
        );
    }

    /// Print the results of this test
    pub fn print(&self) {
        self._print(None);
    }

    /// Print the results with the number of bytes processed for this test
    pub fn print_with_bytes(&self, bytes: u64) {
        self._print(Some(bytes));
    }
}

pub struct RepititionTester {
    /// The current state of this tester
    state: TestingState,

    /// How long to run the tests
    duration: Duration,

    /// The current timer used to track when to stop the test
    start_time: Instant,

    /// The current number of page faults seen
    page_faults: u64,

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
            page_faults: 0,
            start_count: 0,
            stop_count: 0,
            elapsed_time: 0,
            results: TestResults::default(),
        }
    }

    pub fn reset(&mut self) {
        self.state = TestingState::Running;
        self.start_time = std::time::Instant::now();
        self.page_faults = 0;
        self.start_count = 0;
        self.stop_count = 0;
        self.elapsed_time = 0;
        self.results = TestResults::default();
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
                self.results.total_page_faults += self.page_faults;

                if self.elapsed_time < self.results.min.time {
                    self.results.min.time = self.elapsed_time;
                    self.results.min.page_faults = self.page_faults;
                    self.start_time = Instant::now();
                    // println!("New min! {:?}", self.results.min_time);
                }

                if self.elapsed_time > self.results.max.time {
                    self.results.max.time = self.elapsed_time;
                    self.results.max.page_faults = self.page_faults;
                }

                // Reset the stats
                self.start_count = 0;
                self.stop_count = 0;
                self.elapsed_time = 0;
                self.page_faults = 0;
            }
        }

        // Keep testing!
        true
    }

    pub fn start(&mut self) {
        self.start_count += 1;
        self.elapsed_time = self.elapsed_time.wrapping_sub(rdtsc());
        self.page_faults = self.page_faults.wrapping_sub(crate::get_page_faults());
    }

    pub fn stop(&mut self) {
        self.stop_count += 1;
        self.elapsed_time = self.elapsed_time.wrapping_add(rdtsc());
        self.page_faults = self.page_faults.wrapping_add(crate::get_page_faults());
    }
}
