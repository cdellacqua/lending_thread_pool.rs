use std::{io, thread::sleep, time::Duration};

use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use lending_thread_pool::ThreadPool;
use tracing::Level;
use tracing_subscriber::fmt::MakeWriter;

pub fn main() {
	let cores = 4;
	let tasks = 16;

	let multi_pb = MultiProgress::new();

	tracing_subscriber::fmt()
		.without_time()
		.with_target(false)
		.with_thread_names(true)
		.with_writer(IndicatifWriter(multi_pb.clone()))
		.with_max_level(Level::TRACE)
		.init();

	let main_pb = multi_pb.add(
		ProgressBar::new(tasks)
			.with_style(
				ProgressStyle::with_template("[{bar:40}] {spinner} {wide_msg}")
					.unwrap()
					.progress_chars("=>-"),
			)
			.with_message("enqueueing tasks..."),
	);
	main_pb.enable_steady_tick(Duration::from_millis(100));

	let mut pool = ThreadPool::new(
		(0..cores)
			.map(|i| {
				multi_pb.add(
					ProgressBar::new(10 * (i + 1))
						.with_style(
							ProgressStyle::with_template("└ [{bar:40}] {spinner} {wide_msg}")
								.unwrap()
								.progress_chars("=>-"),
						)
						.with_message("idle"),
				)
			})
			.collect::<Vec<_>>(),
	);

	for _ in 0..tasks {
		sleep(Duration::from_millis(100));
		pool.enqueue(move |progress_bar| {
			progress_bar.set_message("preparing...");
			sleep(Duration::from_millis(progress_bar.length().unwrap() * 10));
			progress_bar.set_message("processing...");
			for _ in 0..progress_bar.length().unwrap() {
				sleep(Duration::from_millis(10));
				progress_bar.inc(1);
			}
			sleep(Duration::from_millis(100));
			progress_bar.reset();
			progress_bar.set_message("idle");
		});
		main_pb.inc(1);
	}
	assert_eq!(main_pb.position(), main_pb.length().unwrap());
	main_pb.set_message("joining...");

	// once dropped, the pool will automatically join all worker threads.
	// if you need to control when threads are joined, you can manually
	// drop the pool or call pool.join()

	pool.join(); // or, equivalently, drop(pool);
	drop(main_pb);
	println!("main thread finished");
}

#[derive(Debug, Clone)]
struct IndicatifWriter(indicatif::MultiProgress);

impl io::Write for IndicatifWriter {
	fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
		self.0.suspend(|| io::stdout().write(buf))
	}

	fn flush(&mut self) -> std::io::Result<()> {
		self.0.suspend(|| io::stdout().flush())
	}
}

impl<'a> MakeWriter<'a> for IndicatifWriter {
	type Writer = IndicatifWriter;

	fn make_writer(&'a self) -> Self::Writer {
		Self(self.0.clone())
	}
}
