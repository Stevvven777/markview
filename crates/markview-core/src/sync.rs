//! Explicit recovery policies for shared runtime state.
use std::sync::{LockResult, Mutex, MutexGuard};

/// Repairs poisoned state before making it available to other callers.
/// `repair` must restore the invariants of `state` while its guard is held.
pub fn recover<'a, T>(
	mutex: &'a Mutex<T>,
	result: LockResult<MutexGuard<'a, T>>,
	name: &str,
	repair: impl FnOnce(&mut T),
) -> MutexGuard<'a, T> {
	match result {
		Ok(state) => state,
		Err(error) => {
			log::warn!("{name}: repairing poisoned shared state");
			let mut state = error.into_inner();
			repair(&mut state);
			mutex.clear_poison();
			state
		}
	}
}

/// Discards a rebuildable cache after an interrupted update.
pub fn cache<'a, T: Default>(
	mutex: &'a Mutex<T>,
	name: &str,
) -> MutexGuard<'a, T> {
	recover(mutex, mutex.lock(), name, |state| *state = T::default())
}

/// Rejects state whose invariants cannot be repaired locally.
pub fn available<'a, T>(
	mutex: &'a Mutex<T>,
	name: &str,
) -> Option<MutexGuard<'a, T>> {
	mutex
		.lock()
		.map_err(|_| log::warn!("{name}: poisoned shared state is unavailable"))
		.ok()
}
