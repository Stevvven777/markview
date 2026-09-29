//! G1, Tier 3 differential (O5.3): an edit re-parsed incrementally must
//! equal a full parse of the same source, field by field. When the fast path
//! declines (`None`), `reparse` falls back to the full parse, which is equal
//! by construction; the interesting inputs are the ones where it succeeds.
#![no_main]

use std::sync::Arc;

use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use mvfuzz::{budget, mutators, oracle};

fuzz_mutator! { |data: &mut [u8], size: usize, max_size: usize, seed: u32| {
	mutators::markdown(data, size, max_size, seed)
}}

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::parse().from_env();
	let md0 = String::from_utf8_lossy(data).into_owned();
	if md0.len() > 256 * 1024 {
		return;
	}
	// One deterministic edit: insert a line, so the changed window is small
	// and the rest of the document exercises the reuse path.
	let seed = oracle::derive(data);
	let line = [
		"an inserted line",
		"## heading now",
		"- a new item",
		"more **emphasis**",
		"$x^2$",
		"| a | b |",
		"```rust",
	][seed as usize % 7];
	let lines: Vec<&str> = md0.split_inclusive('\n').collect();
	let at = (seed >> 8) as usize % (lines.len() + 1);
	let before: String = lines.iter().take(at).copied().collect();
	let after: String = lines.iter().skip(at).copied().collect();
	let md1 = format!("{before}{line}\n{after}");

	let guard = budget::InputGuard::new();
	let doc0 = markview_core::document::parse(md0);
	let len = md1.len();
	let full = markview_core::document::parse(md1.clone());
	let incr = markview_core::document::reparse(&doc0, Arc::from(md1));
	assert_eq!(
		oracle::document(&full),
		oracle::document(&incr),
		"incremental reparse diverged from the full parse"
	);
	guard.finish(&budget, len);
});
