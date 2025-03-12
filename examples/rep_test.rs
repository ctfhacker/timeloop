#![feature(thread_id_value)]

use rand::prelude::SliceRandom;

use std::io::Read;
use std::time::Duration;

use timeloop::RepititionTester;

fn rdtsc() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

#[derive(Debug)]
pub enum Collection {
    Linear,
    BTreeMap,
    HashMap,
}

pub struct TestParameters {
    pub allocation: Allocation,
}

impl TestParameters {
    /// # Panics
    ///
    /// * Reused buffer wasn't properly reset
    pub fn get_buffer(&mut self) -> Vec<u8> {
        match self.allocation {
            Allocation::New => Vec::new(),
            Allocation::NewWithCapacity => Vec::with_capacity(self.expected_file_size),
            Allocation::Reused => {
                let mut result = self
                    .buffer
                    .take()
                    .expect("Reused buffer was not set back after take()");
                result.clear();
                result
            }
        }
    }
}

#[allow(clippy::missing_panics_doc)]
pub fn test_read(params: &mut TestParameters) -> Vec<u8> {
    std::fs::read(params.file).unwrap()
}

#[allow(clippy::missing_panics_doc)]
pub fn test_file_read(params: &mut TestParameters) -> Vec<u8> {
    let mut file = std::fs::File::open(params.file).unwrap();

    let mut result = params.get_buffer();

    file.read_to_end(&mut result)
        .expect("Failed to read file with read_to_end");

    result
}

#[allow(clippy::missing_panics_doc, clippy::cast_possible_truncation)]
pub fn test_write(params: &mut TestParameters) -> Vec<u8> {
    let mut result = params.get_buffer();

    for i in 0..params.expected_file_size {
        result.push(i as u8);
    }

    result
}
type NamedTestFunction = (&'static str, fn(&mut TestParameters) -> Vec<u8>);

fn main() {
    const FILE: &str = "/tmp/testfile";
    const EXPECTED_FILE_SIZE: usize = 1024 * 1024 * 1024;

    let mut rng = rand::thread_rng();

    // Create the test file if it doesn't exist
    #[allow(clippy::cast_possible_truncation)]
    if !std::path::Path::new(FILE).exists() {
        let buf: Vec<u8> = (0..EXPECTED_FILE_SIZE).map(|_| rdtsc() as u8).collect();
        std::fs::write(FILE, buf).expect("Failed to write test file");
    }

    let funcs: &mut [NamedTestFunction] = &mut [
        ("Linear sweep", test_linear_sweep),
        ("BTreeMap", test_btreemap),
    ];

    let mut params = TestParameters {
        buffer: None,
        allocation: Allocation::New,
    };

    for _ in 0..3 {
        // Randomly choose which function to test
        funcs.shuffle(&mut rng);

        for func in funcs.iter() {
            for alloc_strategy in [
                Allocation::New,
                Allocation::NewWithCapacity,
                Allocation::Reused,
            ] {
                params.allocation = alloc_strategy;

                let mut tester = RepititionTester::new(Duration::from_secs(5));

                while tester.is_testing() {
                    // Start the timer for this iteration
                    tester.start();

                    // Execute the function in question
                    let result = func.1(&mut params);

                    // Stop the timer for this iteration
                    tester.stop();

                    // Check for valid results
                    assert!(
                        result.len() == EXPECTED_FILE_SIZE,
                        "Found {} != {EXPECTED_FILE_SIZE}",
                        result.len()
                    );

                    // Reset the buffer to be reused again
                    params.buffer = Some(result);
                }

                println!(
                    "----- {:20?} | {} -----",
                    format!("{:?}", params.allocation),
                    func.0
                );
                tester.results_with_throughput(EXPECTED_FILE_SIZE).print();
            }
        }
    }
}
