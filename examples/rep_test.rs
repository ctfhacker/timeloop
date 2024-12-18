#![feature(thread_id_value)]

use rand::prelude::SliceRandom;

use std::io::Read;
use std::time::Duration;

use timeloop::RepititionTester;

fn rdtsc() -> u64 {
    unsafe { std::arch::x86_64::_rdtsc() }
}

#[derive(Debug)]
pub enum Allocation {
    New,
    NewWithCapacity,
    Reused,
}

pub struct TestParameters {
    pub file: &'static str,
    pub expected_file_size: u64,
    pub buffer: Option<Vec<u8>>,
    pub allocation: Allocation,
}

impl TestParameters {
    pub fn get_buffer(&mut self) -> Vec<u8> {
        match self.allocation {
            Allocation::New => Vec::new(),
            Allocation::NewWithCapacity => Vec::with_capacity(self.expected_file_size as usize),
            Allocation::Reused => {
                let mut result = self.buffer.take().expect("Reused buffer not available");
                result.clear();
                result
            }
        }
    }
}

pub fn test_read(params: &mut TestParameters) -> Vec<u8> {
    std::fs::read(params.file).unwrap()
}

pub fn test_file_read(params: &mut TestParameters) -> Vec<u8> {
    let mut file = std::fs::File::open(params.file).unwrap();

    let mut result = params.get_buffer();

    file.read_to_end(&mut result)
        .expect("Failed to read file with read_to_end");

    result
}

pub fn test_write(params: &mut TestParameters) -> Vec<u8> {
    let mut result = params.get_buffer();

    for i in 0..params.expected_file_size {
        result.push(i as u8);
    }

    result
}

fn test_libc(params: &mut TestParameters) -> Vec<u8> {
    let path_cstr = std::ffi::CString::new(params.file).unwrap();
    let file_fd = unsafe { libc::open(path_cstr.as_ptr(), libc::O_RDONLY) };

    if file_fd == -1 {
        panic!("Failed to open the file");
    }

    // Determine the file size
    let file_size = unsafe { libc::lseek(file_fd, 0, libc::SEEK_END) };
    if file_size == -1 {
        panic!("Failed to seek to the end of the file");
    }

    // Reset the file descriptor back to the beginning to read
    unsafe {
        libc::lseek(file_fd, 0, libc::SEEK_SET);
    }

    // Map the file into memory
    let mmap_ptr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            file_size as libc::size_t,
            libc::PROT_READ,
            libc::MAP_PRIVATE,
            file_fd,
            0 as libc::off_t,
        )
    };

    if mmap_ptr == libc::MAP_FAILED {
        panic!("Failed to mmap the file");
    }

    // Read the content from the mmap'd memory
    let mut result = Vec::with_capacity(params.expected_file_size as usize);

    let read_size = unsafe {
        libc::read(
            file_fd,
            result.as_mut_ptr() as *mut libc::c_void,
            file_size as libc::size_t,
        )
    };

    if read_size == -1 {
        panic!("Failed to read from the file");
    }

    // Unmap the file
    if unsafe { libc::munmap(mmap_ptr, file_size as libc::size_t) } != 0 {
        panic!("Failed to munmap the file");
    }

    // Close the file descriptor
    if unsafe { libc::close(file_fd) } == -1 {
        panic!("Failed to close the file");
    }

    // Set the buffer length based on the actual read size
    unsafe {
        result.set_len(read_size as usize);
    }

    result
}

fn main() {
    const FILE: &'static str = "/tmp/testfile";
    const EXPECTED_FILE_SIZE: u64 = 1024 * 1024 * 1024;

    let mut rng = rand::thread_rng();

    // Create the test file if it doesn't exist
    if !std::path::Path::new(FILE).exists() {
        let buf: Vec<u8> = (0..EXPECTED_FILE_SIZE).map(|_| rdtsc() as u8).collect();
        std::fs::write(FILE, buf).expect("Failed to write test file");
    }

    let funcs: &mut [(&'static str, fn(&mut TestParameters) -> Vec<u8>)] = &mut [
        ("File::open -> read_to_end", test_file_read),
        ("std::fs::read", test_read),
        ("libc", test_libc),
        ("write", test_write),
    ];

    let mut params = TestParameters {
        file: FILE,
        expected_file_size: EXPECTED_FILE_SIZE,
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
                        result.len() == EXPECTED_FILE_SIZE as usize,
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
