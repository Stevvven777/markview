//! Bounded teardown for application-owned threads.
use std::{
	thread,
	time::{Duration, Instant},
};

/// How long a thread gets to finish before shutdown abandons it.
pub(crate) const GRACE: Duration = Duration::from_secs(5);

/// Waits for one application thread, at most [`GRACE`].
///
/// A worker blocked inside a call no token can interrupt never sees its stop
/// signal. An unbounded join would hold the whole process behind it, so the
/// deadline is what keeps shutdown bounded: past it the handle is dropped,
/// which detaches the thread, and exit proceeds without it. Whatever the
/// thread was writing goes through an exclusive temporary file, so abandoning
/// it mid-write leaves the destination untouched rather than half replaced.
pub(crate) fn join_within(thread: thread::JoinHandle<()>, what: &str) {
	join_until(thread, what, GRACE);
}

fn join_until(thread: thread::JoinHandle<()>, what: &str, grace: Duration) {
	let deadline = Instant::now() + grace;
	while !thread.is_finished() {
		if Instant::now() >= deadline {
			log::warn!("{what} did not stop; abandoning it");
			return;
		}
		thread::sleep(Duration::from_millis(10));
	}
	if thread.join().is_err() {
		log::warn!("{what} panicked");
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn a_finished_thread_is_joined() {
		let thread =
			thread::Builder::new().spawn(|| {}).expect("spawn a worker");
		join_within(thread, "Test worker");
	}

	/// A thread that never observes its stop signal must not hold shutdown.
	#[test]
	fn a_thread_that_never_stops_is_abandoned() {
		let (_release, gate) = std::sync::mpsc::channel::<()>();
		let thread = thread::Builder::new()
			.spawn(move || {
				// Stands in for a blocking call no token can interrupt.
				let _ = gate.recv();
			})
			.expect("spawn a worker");
		let started = std::time::Instant::now();
		join_until(thread, "Test worker", Duration::from_millis(100));
		assert!(
			started.elapsed() < Duration::from_secs(5),
			"shutdown waited for a thread that never stopped"
		);
	}

	#[test]
	fn a_panicking_thread_is_reported_not_propagated() {
		let thread = thread::Builder::new()
			.spawn(|| panic!("injected failure"))
			.expect("spawn a worker");
		join_within(thread, "Test worker");
	}
}
