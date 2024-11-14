#![allow(clippy::tabs_in_doc_comments)]

use std::{
	collections::VecDeque,
	fmt, mem,
	sync::{Arc, Condvar, Mutex},
	thread::{self, JoinHandle},
};

use tracing::debug;

/// The Thread Pool struct. This can be constructed using the [`ThreadPool::new`] method.
///
/// # Examples
///
/// ```
/// use lending_thread_pool::ThreadPool;
///
/// let mut pool = ThreadPool::new(
/// 	(0..4)
/// 		.map(|i| format!("Hello from worker {i}"))
/// 		.collect::<Vec<_>>(),
/// );
///
///
/// for _ in 0..16 {
/// 	pool.enqueue(|greeting| { println!("{greeting}"); });
/// }
/// ```
#[derive(Debug)]
pub struct ThreadPool<WorkerData: Send + 'static = ()> {
	inner: Arc<ThreadPoolShared<WorkerData>>,
	workers: Vec<JoinHandle<()>>,
}

type Task<WorkerData> = Box<dyn FnOnce(&mut WorkerData) + Send>;

enum PoolQueue<WorkerData: Send + 'static> {
	Done,
	Todo(VecDeque<Task<WorkerData>>),
}

impl<WorkerData: Send + 'static> fmt::Debug for PoolQueue<WorkerData> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		match self {
			Self::Done => write!(f, "Done"),
			Self::Todo(ref tasks) => write!(f, "Todo({})", tasks.len()),
		}
	}
}

impl<WorkerData: Send + 'static> PoolQueue<WorkerData> {
	fn dequeue(&mut self) -> Result<Option<Task<WorkerData>>, ()> {
		match self {
			Self::Done => Err(()),
			Self::Todo(ref mut tasks) => Ok(tasks.pop_front()),
		}
	}
}

#[derive(Debug)]
struct ThreadPoolShared<WorkerData: Send + 'static> {
	workers_condvar: Condvar,
	pool_condvar: Condvar,
	max_pending_tasks: usize,
	pending_tasks: Mutex<PoolQueue<WorkerData>>,
}

impl<WorkerData: Send + 'static> ThreadPool<WorkerData> {
	/// Construct a thread pool given a Vec of WorkerData. The number of workers
	/// will correspond to the length of the Vec, so will the queue size for pending tasks.
	/// Note that WorkerData can be any type. Each worker thread will own its corresponding WorkerData.
	///
	/// # Panics
	/// - if Vec is empty
	///
	/// # Examples
	///
	/// ```
	/// use lending_thread_pool::ThreadPool;
	///
	/// let mut pool = ThreadPool::new(
	/// 	(0..4)
	/// 		.map(|i| format!("Hello from worker {i}"))
	/// 		.collect::<Vec<_>>(),
	/// );
	///
	///
	/// for _ in 0..16 {
	/// 	pool.enqueue(|greeting| { println!("{greeting}"); });
	/// }
	/// ```
	pub fn new(workers_data: Vec<WorkerData>) -> Self {
		let max_pending = workers_data.len();
		Self::new_with_queue_size(workers_data, max_pending)
	}

	/// Construct a thread pool given a Vec of WorkerData. The number of workers
	/// will correspond to the length of the Vec. Note that WorkerData can be any
	/// type you want. Each worker thread will own its corresponding WorkerData.
	///
	/// # Panics
	/// - if Vec is empty
	/// - if max_pending_tasks is 0.
	///
	/// # Examples
	///
	/// ```
	/// use lending_thread_pool::ThreadPool;
	///
	/// let mut pool = ThreadPool::new_with_queue_size(
	/// 	(0..4)
	/// 		.map(|i| format!("Hello from worker {i}"))
	/// 		.collect::<Vec<_>>(),
	/// 	2,
	/// );
	///
	///
	/// for _ in 0..16 {
	/// 	pool.enqueue(|greeting| { println!("{greeting}"); });
	/// }
	/// ```
	pub fn new_with_queue_size(workers_data: Vec<WorkerData>, max_pending_tasks: usize) -> Self {
		assert_ne!(
			workers_data.len(),
			0,
			"workers_data must contain at least one item"
		);
		assert_ne!(
			max_pending_tasks, 0,
			"max_pending_tasks must be greater than 0"
		);

		let inner = Arc::new(ThreadPoolShared {
			workers_condvar: Default::default(),
			pool_condvar: Default::default(),
			pending_tasks: Mutex::new(PoolQueue::Todo(VecDeque::new())),
			max_pending_tasks,
		});
		let workers = workers_data
			.into_iter()
			.enumerate()
			.map(|(i, mut worker_data)| {
				let inner_clone = inner.clone();
				thread::Builder::new()
					.name(format!("w({i})"))
					.spawn(move || loop {
						let ThreadPoolShared {
							pending_tasks,
							workers_condvar,
							pool_condvar,
							..
						} = &*inner_clone;
						let mut guard = pending_tasks.lock().unwrap();

						let pending_task = loop {
							match guard.dequeue() {
								Ok(None) => {
									debug!("waiting for tasks...");
									guard = workers_condvar.wait(guard).unwrap();
								}
								Ok(Some(task)) => break Some(task),
								Err(_) => break None,
							}
						};

						if let Some(task) = pending_task {
							pool_condvar.notify_all();
							drop(guard);
							debug!("running task...");
							(task)(&mut worker_data);
						} else {
							debug!("quitting...");
							break;
						}
					})
					.expect("thread to be spawned")
			})
			.collect::<Vec<_>>();

		Self { inner, workers }
	}

	/// Enqueue a task in the pool.
	///
	/// # Blocking
	///
	/// This method is blocking. It waits for the task queue to have at least one empty
	/// slot before returning.
	pub fn enqueue<Task: FnOnce(&mut WorkerData) + Send + 'static>(&mut self, task: Task) {
		let mut guard = self.inner.pending_tasks.lock().unwrap();

		loop {
			match &mut *guard {
				PoolQueue::Todo(ref mut tasks) => {
					if tasks.len() >= self.inner.max_pending_tasks {
						debug!("waiting for available workers...");
						guard = self.inner.pool_condvar.wait(guard).unwrap();
					} else {
						tasks.push_back(Box::new(task));
						self.inner.workers_condvar.notify_one();
						debug!("added pending task");
						return;
					}
				}
				PoolQueue::Done => unreachable!(
					"Enqueue shouldn't be callable on a joined (thus consumed) thread pool"
				),
			}
		}
	}

	/// Signal to all worker threads that they should exit once finished with their current task,
	/// then joins all their handles.
	///
	/// Note: join is automatically called on drop.
	pub fn join(mut self) {
		self.join_by_ref();
	}

	fn join_by_ref(&mut self) {
		let mut guard = self.inner.pending_tasks.lock().unwrap();

		loop {
			match &mut *guard {
				// already joined
				PoolQueue::Done => return,
				PoolQueue::Todo(tasks) if tasks.is_empty() => break,
				_ => {
					debug!("waiting for idle...");
					guard = self.inner.pool_condvar.wait(guard).unwrap();
				}
			}
		}
		debug!("sending stop request...");
		*guard = PoolQueue::Done;
		drop(guard);
		self.inner.workers_condvar.notify_all();
		debug!("joining...");
		let workers = mem::take(&mut self.workers);
		for w in workers {
			w.join().unwrap();
		}
	}
}

impl<WorkerData: Send + 'static> Drop for ThreadPool<WorkerData> {
	fn drop(&mut self) {
		self.join_by_ref();
	}
}