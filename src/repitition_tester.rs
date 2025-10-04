//! Implements `RepitionTester`
use std::time::{Duration, Instant};
use crate::rdtsc;

#[allow(dead_code)]
enum TestingState {
    // Uninit,
    Running,
    Error(&'static str),
}

/// Statistics for an individual test case
#[derive(Copy, Clone, Debug, Default)]
pub struct TestCase {
    /// The time (in cycles) for this test case
    pub cycles: u64,

    /// The time [`Duration`] for this test case
    pub time: Duration,

    /// The page faults for this iteration
    pub page_faults: u64,

    /// Bytes per second if throughput is given
    pub bytes_per_second: Option<f64>,

    /// Number of bytes processed per page fault
    pub bytes_per_page_fault: Option<f64>,
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
    pub max: TestCase,

    /// The statistics for the shortest test case
    pub min: TestCase,

    /// The statistics for the average test case
    pub avg: TestCase,
}

impl Default for TestResults {
    fn default() -> Self {
        Self {
            count: 0,
            total_time: 0,
            total_page_faults: 0,
            max: TestCase {
                cycles: 0,
                ..Default::default()
            },
            min: TestCase {
                cycles: u64::MAX,
                ..Default::default()
            },
            avg: TestCase {
                cycles: u64::MAX,
                ..Default::default()
            },
        }
    }
}

impl TestResults {
    /// Print the results of this test
    pub fn print(&self) {
        let TestResults {
            count: _,
            total_time: _,
            total_page_faults: _,
            max,
            min,
            avg,
        } = self;

        for (title, results) in [("Min", min), ("Max", max), ("Avg", avg)] {
            print!("{title}: {:8.2?} ({:8.2?})", results.cycles, results.time);

            if let Some(bytes_per_second) = results.bytes_per_second {
                let (num, unit) = if bytes_per_second > 1024. * 1024. * 1024. {
                    (bytes_per_second / 1024. / 1024. / 1024., "GB")
                } else if bytes_per_second > 1024. * 1024. {
                    (bytes_per_second / 1024. / 1024., "MB")
                } else if bytes_per_second > 1024. {
                    (bytes_per_second / 1024., "KB")
                } else {
                    (bytes_per_second, "B")
                };

                print!(
                    " {num:8.2} {unit}/sec | PageFaults: {}",
                    results.page_faults,
                );
            }

            println!();
        }

        /*
        println!(
            "Max: {:12?} ({max_time_secs:8.2?}) {} Faults: {:8}",
            self.max.cycles,
            max_byte_per_sec.unwrap_or_else(|| "".to_string()),
            self.max.page_faults,
        );
        println!(
            "Avg: {:12.0?} ({avg_time_secs:8.2?}) {} Faults: {:8}",
            self.total_time as f64 / self.count as f64,
            avg_byte_per_sec.unwrap_or_else(|| "".to_string()),
            self.total_page_faults / self.count
        );
        */
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
    results: TestResults,
}

impl RepititionTester {
    #[must_use]
    pub fn new(duration: Duration) -> Self {
        let _pf = crate::get_page_faults();

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

    /// Get the results of a test
    pub fn results(&mut self) -> TestResults {
        self._results(None)
    }

    /// Get the results of a test that used `bytes` number of bytes
    pub fn results_with_throughput(&mut self, bytes: usize) -> TestResults {
        self._results(Some(bytes))
    }

    #[allow(clippy::cast_precision_loss)]
    fn _results(&mut self, bytes: Option<usize>) -> TestResults {
        let os_freq = crate::calculate_os_frequency();

        if self.results.count == 0 {
            return TestResults::default();
        }

        self.results.min.time =
            std::time::Duration::from_secs_f64(self.results.min.cycles as f64 / os_freq);
        self.results.max.time =
            std::time::Duration::from_secs_f64(self.results.max.cycles as f64 / os_freq);
        self.results.max.time = std::time::Duration::from_secs_f64(
            self.results.total_time as f64 / self.results.count as f64 / os_freq,
        );

        // Update the average time results
        self.results.avg.cycles = self.results.total_time / self.results.count;
        self.results.avg.time = Duration::from_secs_f64(
            self.results.total_time as f64 / os_freq / self.results.count as f64,
        );

        // Adjust each of the min, max
        if let Some(bytes) = bytes {
            for curr_results in [
                &mut self.results.min,
                &mut self.results.max,
                &mut self.results.avg,
            ] {
                curr_results.bytes_per_second =
                    Some(bytes as f64 / curr_results.time.as_secs_f64());

                if curr_results.page_faults > 0 {
                    curr_results.bytes_per_page_fault =
                        Some(bytes as f64 / curr_results.page_faults as f64);
                }
            }
        }

        self.results
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

                if self.elapsed_time < self.results.min.cycles {
                    self.results.min.cycles = self.elapsed_time;
                    self.results.min.page_faults = self.page_faults;
                    self.start_time = Instant::now();
                    // println!("New min! {:?}", self.results.min_time);
                }

                if self.elapsed_time > self.results.max.cycles {
                    self.results.max.cycles = self.elapsed_time;
                    self.results.max.page_faults = self.page_faults;
                }
            }
        }

        // Reset the stats
        self.start_count = 0;
        self.stop_count = 0;
        self.elapsed_time = 0;
        self.page_faults = 0;

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
