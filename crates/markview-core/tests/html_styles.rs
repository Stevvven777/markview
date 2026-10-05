//! A raw HTML block must not grow its per-run style state with tag depth, or
//! copy a long `href` into the state each open tag keeps.
//!
//! Every `<b>` in `<b>x<b>x...` used to add one more patch to every run after
//! it, so the block retained work quadratic in the nesting depth. The guard
//! measures the peak allocation of one parse, because a wall-clock bound would
//! depend on the machine.

// The counting allocator is test-only instrumentation for the guard above.
#![allow(unsafe_code)]

use markview_core::document::{BlockKind, InlineKind, parse};
use std::{
	alloc::{GlobalAlloc, Layout, System},
	sync::atomic::{AtomicUsize, Ordering},
};

/// Peak live bytes, tracked so the test can bound a single parse.
static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn accounted(bytes: usize) {
	let live = LIVE.fetch_add(bytes, Ordering::Relaxed) + bytes;
	PEAK.fetch_max(live, Ordering::Relaxed);
}

struct Counting;

// SAFETY: every method forwards its arguments unchanged to `System` and only
// records `layout.size()`; all pointers stay owned by `System`.
unsafe impl GlobalAlloc for Counting {
	unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
		// SAFETY: `layout` comes from the caller of this allocator, which is
		// exactly what `System::alloc` requires.
		let ptr = unsafe { System.alloc(layout) };
		if !ptr.is_null() {
			accounted(layout.size());
		}
		ptr
	}

	unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
		LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
		// SAFETY: `ptr` and `layout` are the pair `System` returned or last
		// accepted, and no other allocator touched the pointer.
		unsafe { System.dealloc(ptr, layout) };
	}

	unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
		// SAFETY: `layout` comes from the caller of this allocator, which is
		// exactly what `System::alloc_zeroed` requires.
		let ptr = unsafe { System.alloc_zeroed(layout) };
		if !ptr.is_null() {
			accounted(layout.size());
		}
		ptr
	}

	unsafe fn realloc(
		&self,
		ptr: *mut u8,
		layout: Layout,
		new_size: usize,
	) -> *mut u8 {
		// SAFETY: `ptr` and `layout` are the pair `System` returned, and
		// `new_size` is the caller's requested size, so the new block replaces
		// the old one.
		let out = unsafe { System.realloc(ptr, layout, new_size) };
		if !out.is_null() {
			LIVE.fetch_sub(layout.size(), Ordering::Relaxed);
			accounted(new_size);
		}
		out
	}
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// One parse's reading text and the peak live bytes above the live bytes
/// before it.
fn parse_measured(source: String) -> (String, usize) {
	let baseline = LIVE.load(Ordering::Relaxed);
	PEAK.store(baseline, Ordering::Relaxed);
	let document = parse(source);
	let peak = PEAK.load(Ordering::Relaxed).saturating_sub(baseline);
	let text = document
		.blocks
		.iter()
		.flat_map(|block| match &block.kind {
			BlockKind::Paragraph(rich) => rich.as_slice(),
			_ => &[],
		})
		.filter_map(|inline| match &inline.kind {
			InlineKind::Text(text) => Some(text.as_str()),
			_ => None,
		})
		.collect();
	(text, peak)
}

#[test]
fn deeply_nested_raw_html_keeps_its_allocation_bounded() {
	const DEPTH: usize = 4_000;
	const BUDGET: usize = 4 << 20;
	let (text, peak) =
		parse_measured(format!("<p>{}</p>\n", "<b>x".repeat(DEPTH)));
	assert!(peak < BUDGET, "nested tags: peak allocation {peak} bytes");
	assert_eq!(text, "x".repeat(DEPTH));
	// A long `href` must not be copied into every open tag's style.
	let (_, peak) = parse_measured(format!(
		"<p><a href=\"{}\">{}</a></p>\n",
		"u".repeat(64 << 10),
		"<a>".repeat(DEPTH)
	));
	assert!(peak < BUDGET, "long href: peak allocation {peak} bytes");
}
