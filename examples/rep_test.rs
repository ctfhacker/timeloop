use std::time::Duration;
use timeloop::{RepititionTester, TestResults};

fn rdtsc() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

pub fn test_read_to_string(file: &str) -> TestResults {
    let mut tester = RepititionTester::new(Duration::from_secs(5));

    while tester.is_testing() {
        tester.start();
        let mut result = std::fs::read_to_string(file);
        tester.stop();

        assert!(result.unwrap().len() > 0);
    }

    tester.results
}

fn main() {
    let file = "./examples/rep_test.rs";
    let read_to_string_results = test_read_to_string(file);

    println!("{:?}", read_to_string_results);
}
