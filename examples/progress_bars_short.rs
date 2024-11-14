use indicatif::{MultiProgress, ProgressBar};
use lending_thread_pool::ThreadPool;
use std::{thread::sleep, time::Duration};

pub fn main() {
	// Assuming we have 4 cores
	let cores = 4;
	// Simulating 16 expensive things to do in parallel
	let tasks = 16;

	let multi_pb = MultiProgress::new();

	let main_pb = multi_pb.add(ProgressBar::new(tasks));

	// Initialize a thread pool where workers own their
	// respective indicatif progress bars.
	let mut pool = ThreadPool::new(
		(0..cores)
			.map(|_| multi_pb.add(ProgressBar::new(10)))
			.collect::<Vec<_>>(),
	);

	for _ in 0..tasks {
		// Simulate a long operation on the main thread
		sleep(Duration::from_millis(100));

		// We can now recycle the worker progress bar for each task
		pool.enqueue(move |progress_bar| {
			// Simulate a series of long operations on the worker thread
			for _ in 0..10 {
				sleep(Duration::from_millis(10));
				progress_bar.inc(1);
			}
			progress_bar.reset();
		});
		main_pb.inc(1);
	}
}
