# lending_thread_pool

[![Documentation](https://docs.rs/lending_thread_pool/badge.svg)](https://docs.rs/lending_thread_pool/)
[![Crates.io](https://img.shields.io/crates/v/lending_thread_pool.svg)](https://crates.io/crates/lending_thread_pool)
[![Build status](https://github.com/cdellacqua/lending_thread_pool.rs/workflows/CI/badge.svg)](https://github.com/cdellacqua/lending_thread_pool.rs/actions/workflows/ci.yml)

A thread pool where workers can lend their data to their tasks.

This library implements an admittedly simple thread pool with a peculiar feature: workers
can hold on to some data and lend it to their tasks, thus greatly simplifying lifetimes
in some scenarios, e.g. when you want to show the status of each thread (idle/running).

## Usage

Here is a basic example:

```rust
let cores = 4;

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
```

As you might notice, `ThreadPool::new` doesn't take a number, but rather a `Vec`, which can contain anything.
The number of threads in the pool will correspond to the length of the provided `Vec`.

Each item in the `Vec` will be moved to the corresponding thread. This way, when a worker gets assigned a task,
it will be able to temporarily lend its data, through a mutable reference, to the task closure.

This lending mechanism makes it possible for tasks to share information. A common scenario where you might
want to take advantage of this feature is showing some progress bars, for example, using [indicatif](https://crates.io/crates/indicatif) we can do something like this:

```rust
use indicatif::{MultiProgress, ProgressBar};
use std::{thread::sleep, time::Duration};
use lending_thread_pool::ThreadPool;

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
```

For complete examples, you can explore the project [examples directory](https://github.com/cdellacqua/lending_thread_pool.rs/tree/main/examples).
