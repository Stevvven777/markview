//! A counting global allocator backing the peak-allocation oracle.
//!
//! Peak here means the high-water mark of live bytes (`allocated - freed`),
//! measured by the harness itself rather than reading system memory (O3). The
//! fuzzer runs single-threaded; the highlight worker may still allocate in the
//! background, which inflates the peak slightly. That conservatism is fine:
//! the budget is a guard, not a measurement.

use std::{
	alloc::{GlobalAlloc, Layout, System},
	sync::atomic::{AtomicUsize, Ordering},
};

#[derive(Default)]
pub struct CountingAllocator {
	current: AtomicUsize,
	peak: AtomicUsize,
}

impl CountingAllocator {
	pub const fn new() -> Self {
		Self {
			current: AtomicUsize::new(0),
			peak: AtomicUsize::new(0),
		}
	}
	/// Live bytes right now.
	pub fn current(&self) -> u64 {
		self.current.load(Ordering::Relaxed) as u64
	}
	/// The highest live-byte count since process start.
	pub fn peak(&self) -> u64 {
		self.peak.load(Ordering::Relaxed) as u64
	}
}
unsafe impl GlobalAlloc for CountingAllocator {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		let ptr = unsafe { System.alloc(layout) };
		if !ptr.is_null() {
			let live = self.current.fetch_add(layout.size(), Ordering::Relaxed);
			self.peak.fetch_max(live + layout.size(), Ordering::Relaxed);
		}
		ptr
	}
	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		self.current.fetch_sub(layout.size(), Ordering::Relaxed);
		unsafe { System.dealloc(ptr, layout) }
	}
}

/// The process-wide allocator. Declared here so every target and tool in
/// this crate counts into the same instance the [`crate::budget::InputGuard`]
/// reads; a per-binary `#[global_allocator]` would be a *different* counter
/// that never gets observed.
#[global_allocator]
pub static GLOBAL: CountingAllocator = CountingAllocator::new();
