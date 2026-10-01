//! Owned, nonblocking CPU work, independent of any async runtime.
use std::sync::Arc;

/// A computation and the number of input bytes it retains while queued.
pub struct Task {
	pub input_bytes: usize,
	compute: Box<dyn FnOnce() + Send>,
}
impl Task {
	pub fn new(
		input_bytes: usize,
		compute: impl FnOnce() + Send + 'static,
	) -> Self {
		Self {
			input_bytes,
			compute: Box::new(compute),
		}
	}
	pub fn run(self) {
		(self.compute)();
	}
}

pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// Rejected tasks remain owned by the caller, including their inputs.
pub trait Executor: Send + Sync {
	fn try_submit(&self, task: Task) -> Result<(), Task>;
	/// Registers a callback for newly available queue space.
	fn on_available(&self, _wake: Wake) {}
}

pub struct Direct;
impl Executor for Direct {
	fn try_submit(&self, task: Task) -> Result<(), Task> {
		task.run();
		Ok(())
	}
}

pub fn default_executor() -> Arc<dyn Executor> {
	#[cfg(target_arch = "wasm32")]
	{
		Arc::new(Direct)
	}
	#[cfg(not(target_arch = "wasm32"))]
	{
		Arc::new(ThreadExecutor::new(None))
	}
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::ThreadExecutor;
#[cfg(not(target_arch = "wasm32"))]
mod native {
	use super::*;
	use std::{
		collections::VecDeque,
		sync::{Condvar, Mutex},
		thread::{self, JoinHandle},
	};

	const QUEUE_TASKS: usize = 16;
	const QUEUE_BYTES: usize = 64 * 1024 * 1024;
	#[derive(Default)]
	struct Queue {
		tasks: VecDeque<Task>,
		bytes: usize,
		closed: bool,
		wakes: Vec<std::sync::Weak<dyn Fn() + Send + Sync>>,
	}
	/// Workers start with the first accepted task and are joined on drop.
	pub struct ThreadExecutor {
		queue: Arc<(Mutex<Queue>, Condvar)>,
		workers: Mutex<Vec<JoinHandle<()>>>,
		concurrency: usize,
		stack: Option<usize>,
	}
	impl ThreadExecutor {
		pub fn new(stack: Option<usize>) -> Self {
			Self {
				queue: Arc::new((Mutex::new(Queue::default()), Condvar::new())),
				workers: Mutex::new(Vec::new()),
				concurrency: thread::available_parallelism()
					.map_or(1, |n| n.get())
					.saturating_sub(1)
					.clamp(1, 4),
				stack,
			}
		}
		pub fn shutdown(&self) {
			let tasks = {
				let mut queue = self.queue.0.lock().unwrap();
				queue.closed = true;
				queue.bytes = 0;
				std::mem::take(&mut queue.tasks)
			};
			drop(tasks);
			self.queue.1.notify_all();
			for worker in self.workers.lock().unwrap().drain(..) {
				if worker.join().is_err() {
					log::warn!("CPU worker panicked");
				}
			}
		}
	}
	impl Executor for ThreadExecutor {
		fn try_submit(&self, task: Task) -> Result<(), Task> {
			let mut workers = self.workers.lock().unwrap();
			let mut queue = self.queue.0.lock().unwrap();
			if queue.closed
				|| queue.tasks.len() >= QUEUE_TASKS
				|| task.input_bytes > QUEUE_BYTES.saturating_sub(queue.bytes)
			{
				return Err(task);
			}
			if workers.is_empty() {
				for index in 0..self.concurrency {
					let shared = self.queue.clone();
					let mut builder = thread::Builder::new()
						.name(format!("markview-cpu-{index}"));
					if let Some(stack) = self.stack {
						builder = builder.stack_size(stack);
					}
					workers.push(
						builder
							.spawn(move || {
								loop {
									let (task, wakes) = {
										let mut queue =
											shared.0.lock().unwrap();
										while queue.tasks.is_empty()
											&& !queue.closed
										{
											queue =
												shared.1.wait(queue).unwrap();
										}
										if queue.closed {
											break;
										}
										let task =
											queue.tasks.pop_front().unwrap();
										queue.bytes -= task.input_bytes;
										(
											task,
											queue
												.wakes
												.iter()
												.filter_map(
													std::sync::Weak::upgrade,
												)
												.collect::<Vec<_>>(),
										)
									};
									for wake in wakes {
										wake();
									}
									if std::panic::catch_unwind(
										std::panic::AssertUnwindSafe(|| {
											task.run()
										}),
									)
									.is_err()
									{
										log::warn!("CPU computation panicked");
									}
								}
							})
							.expect("start CPU worker"),
					);
				}
			}
			queue.bytes += task.input_bytes;
			queue.tasks.push_back(task);
			drop(queue);
			self.queue.1.notify_one();
			Ok(())
		}
		fn on_available(&self, wake: Wake) {
			let mut queue = self.queue.0.lock().unwrap();
			queue.wakes.retain(|wake| wake.strong_count() > 0);
			queue.wakes.push(Arc::downgrade(&wake));
		}
	}
	impl Drop for ThreadExecutor {
		fn drop(&mut self) {
			self.shutdown();
		}
	}

	#[cfg(test)]
	mod tests {
		use super::*;
		use std::sync::{Barrier, mpsc};
		#[test]
		fn queue_count_and_running_computations_share_fixed_capacity() {
			let mut executor = ThreadExecutor::new(None);
			executor.concurrency = 1;
			let entered = Arc::new(Barrier::new(2));
			let release = Arc::new(Barrier::new(2));
			let a = entered.clone();
			let b = release.clone();
			assert!(
				executor
					.try_submit(Task::new(0, move || {
						a.wait();
						b.wait();
					}))
					.is_ok()
			);
			entered.wait();
			for _ in 0..QUEUE_TASKS {
				assert!(executor.try_submit(Task::new(0, || {})).is_ok());
			}
			assert!(
				executor
					.try_submit(Task::new(0, || panic!("rejected task ran")))
					.is_err()
			);
			assert_eq!(executor.workers.lock().unwrap().len(), 1);
			release.wait();
			executor.shutdown();
		}

		#[test]
		fn queue_returns_owned_inputs_and_shutdown_joins_running_work() {
			let mut executor = ThreadExecutor::new(None);
			executor.concurrency = 1;
			let entered = Arc::new(Barrier::new(2));
			let release = Arc::new(Barrier::new(2));
			let (done, recv) = mpsc::channel();
			let a = entered.clone();
			let b = release.clone();
			assert!(
				executor
					.try_submit(Task::new(0, move || {
						a.wait();
						b.wait();
						done.send(()).unwrap();
					}))
					.is_ok()
			);
			entered.wait();
			assert!(executor.try_submit(Task::new(QUEUE_BYTES, || {})).is_ok());
			let rejected = executor
				.try_submit(Task::new(1, || panic!("rejected task ran")))
				.err()
				.unwrap();
			assert_eq!(rejected.input_bytes, 1);
			release.wait();
			executor.shutdown();
			recv.recv().unwrap();
			assert!(executor.try_submit(rejected).is_err());
			assert!(executor.workers.lock().unwrap().is_empty());
		}
	}
}
