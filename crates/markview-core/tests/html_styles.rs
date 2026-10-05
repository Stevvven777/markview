//! A raw HTML block must not grow its per-run style state with tag depth, or
//! copy a long `href` into the state each open tag keeps.
//!
//! Every `<b>` in `<b>x<b>x...` used to add one more patch to every run after
//! it, so the block retained work quadratic in the nesting depth. The guard
//! measures the peak allocation of one parse, because a wall-clock bound would
//! depend on the machine.
//!
//! Inline HTML has the same shape but no nesting at all: Comrak passes raw
//! tags through as siblings, so a run of unclosed tags grows the scope stack
//! without consuming the inline depth budget, and one long address used to be
//! copied into every open scope and every emitted run.

// The counting allocator is test-only instrumentation for the guard above.
#![allow(unsafe_code)]

use markview_core::document::{BlockKind, InlineKind, parse};
use std::{
	alloc::{GlobalAlloc, Layout, System},
	sync::{
		Arc, Mutex,
		atomic::{AtomicUsize, Ordering},
	},
	time::{Duration, Instant},
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

/// The counters and the clock are process-wide, so one measured parse runs at
/// a time; concurrent tests would otherwise see each other's allocations.
static MEASURING: Mutex<()> = Mutex::new(());

/// One parse's reading text, the peak live bytes above the live bytes before
/// it, and how long the parse took.
fn parse_measured(source: String) -> (String, usize, Duration) {
	let _measuring = MEASURING
		.lock()
		.unwrap_or_else(|poisoned| poisoned.into_inner());
	let baseline = LIVE.load(Ordering::Relaxed);
	PEAK.store(baseline, Ordering::Relaxed);
	let start = Instant::now();
	let document = parse(source);
	let elapsed = start.elapsed();
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
	(text, peak, elapsed)
}

#[test]
fn deeply_nested_raw_html_keeps_its_allocation_bounded() {
	const DEPTH: usize = 4_000;
	const BUDGET: usize = 4 << 20;
	let (text, peak, _) =
		parse_measured(format!("<p>{}</p>\n", "<b>x".repeat(DEPTH)));
	assert!(peak < BUDGET, "nested tags: peak allocation {peak} bytes");
	assert_eq!(text, "x".repeat(DEPTH));
	// A long `href` must not be copied into every open tag's style.
	let (_, peak, _) = parse_measured(format!(
		"<p><a href=\"{}\">{}</a></p>\n",
		"u".repeat(64 << 10),
		"<a>".repeat(DEPTH)
	));
	assert!(peak < BUDGET, "long href: peak allocation {peak} bytes");
}

/// Inline raw HTML is a sibling list, so one long address must not be copied
/// per open scope or per emitted run. The guard compares the same shape with a
/// short and a long address: neither the peak allocation nor the parse time may
/// follow the address, however many tags carry it.
#[test]
fn an_inline_address_is_not_copied_per_tag_or_per_run() {
	const TAGS: usize = 4_000;
	const SHORT: usize = 64;
	const LONG: usize = 64 << 10;
	// Unclosed tags grow the scope stack; alternating tags keep every emitted
	// run unmergeable, so both retention paths are exercised.
	/// One attack shape: the address length and the tag count become source.
	type Shape = fn(usize, usize) -> String;
	let shapes: [(&str, Shape); 2] = [
		("open scopes", |href, tags| {
			format!("<a href=\"{}\">{}\n", "u".repeat(href), "<b>".repeat(tags))
		}),
		("emitted runs", |href, tags| {
			let mut source = format!("<a href=\"{}\">", "u".repeat(href));
			source.push_str(&"<b>x</b><i>x</i>".repeat(tags));
			source.push('\n');
			source
		}),
	];
	for (name, shape) in shapes {
		let (_, short_peak, short_time) = parse_measured(shape(SHORT, TAGS));
		let (_, long_peak, long_time) = parse_measured(shape(LONG, TAGS));
		assert!(
			long_peak < short_peak + (1 << 20),
			"{name}: a longer address grew the peak from {short_peak} to \
			 {long_peak} bytes"
		);
		assert!(
			long_time < short_time * 4 + Duration::from_millis(20),
			"{name}: a longer address took {long_time:?} against {short_time:?}"
		);
	}
}

/// Every run under one link shares one address buffer.
#[test]
fn one_link_holds_one_address() {
	let source = format!(
		"<a href=\"{}\">{}</a>\n",
		"u".repeat(64 << 10),
		"<b>x</b><i>x</i>".repeat(500)
	);
	let document = parse(source);
	let mut addresses: Vec<usize> = Vec::new();
	for block in &document.blocks {
		let BlockKind::Paragraph(rich) = &block.kind else {
			continue;
		};
		for inline in rich {
			if let Some(link) = &inline.style.link {
				addresses
					.push(Arc::as_ptr(link.shared()) as *const u8 as usize);
			}
		}
	}
	assert!(!addresses.is_empty(), "the document kept no link");
	addresses.dedup();
	assert_eq!(addresses.len(), 1, "the address was copied per run");
}

/// An unclosed run of inline tags past the scope budget keeps the tags as
/// source, which is what bounds the stack itself rather than only the address
/// each entry holds.
#[test]
fn inline_scopes_are_budgeted() {
	let source = format!("<a href=\"/a\">{}\n", "<b>".repeat(9_000));
	let (text, peak, _) = parse_measured(source);
	assert!(text.contains("<b>"), "tags past the budget stayed styling");
	assert!(
		peak < 16 << 20,
		"budgeted tags: peak allocation {peak} bytes"
	);
}
