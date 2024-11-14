use std::{thread::sleep, time::Duration};

use lending_thread_pool::ThreadPool;
use tracing::Level;

pub fn main() {
	tracing_subscriber::fmt()
		.with_max_level(Level::TRACE)
		.init();

	let cores = num_cpus::get() as u64;

	let mut pool = ThreadPool::new(
		(0..cores)
			.map(|i| format!("Hello from worker {i}"))
			.collect::<Vec<_>>(),
	);

	for _ in 0..cores {
		pool.enqueue(move |worker_greetings| {
			println!("Message from worker: {worker_greetings}");
			sleep(Duration::from_millis(100));
		});
	}

	// once dropped, the pool will automatically join all worker threads.
	// if you need to control when threads are joined, you can manually
	// drop the pool or call pool.join()

	pool.join(); // or, equivalently, drop(pool);
	println!("main thread finished");
}
