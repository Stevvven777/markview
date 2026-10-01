//! A replaceable request slot with independent owner control state.
use std::sync::{
	LockResult, Mutex, MutexGuard,
	atomic::{AtomicU32, AtomicU64},
};

pub(crate) struct Inbox<R, C = ()> {
	pub pending: Option<R>,
	pub stopped: bool,
	pub release: bool,
	pub background_ready: bool,
	pub control: C,
}
impl<R, C: Default> Default for Inbox<R, C> {
	fn default() -> Self {
		Self {
			pending: None,
			stopped: false,
			release: false,
			background_ready: false,
			control: C::default(),
		}
	}
}
impl<R, C: Default> Inbox<R, C> {
	pub fn recover<'a>(
		lock: &'a Mutex<Self>,
		result: LockResult<MutexGuard<'a, Self>>,
	) -> MutexGuard<'a, Self> {
		markview_core::sync::recover(lock, result, "Worker inbox", |state| {
			state.pending = None;
			state.control = C::default();
		})
	}
	pub fn lock(lock: &Mutex<Self>) -> MutexGuard<'_, Self> {
		Self::recover(lock, lock.lock())
	}
}
#[derive(Default)]
pub(crate) struct Control {
	pub sequence: AtomicU64,
	pub generation: AtomicU64,
	pub coverage: AtomicU32,
}

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn replacing_a_request_preserves_release_stop_and_completion() {
		let mut inbox = Inbox::<usize> {
			release: true,
			background_ready: true,
			stopped: true,
			pending: Some(1),
			..Default::default()
		};
		inbox.pending = Some(2);
		assert_eq!(inbox.pending.take(), Some(2));
		assert!(inbox.release && inbox.background_ready && inbox.stopped);
	}
}
