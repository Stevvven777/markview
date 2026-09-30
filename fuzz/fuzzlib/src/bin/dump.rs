//! Dump the blocks, source ranges, and content id of a fuzz input through
//! the full parse, the prefix parse, and the incremental reparse the
//! `reparse` target applies. For crash triage; the output points at which
//! entry point diverges.
//!
//! Run: `cargo run -p mvfuzz --bin dump -- <file>`

use std::sync::Arc;

fn main() {
	let path = std::env::args().nth(1).expect("usage: dump <file>");
	let data = std::fs::read(&path).unwrap();
	let md0 = String::from_utf8_lossy(&data).into_owned();

	let doc0 = markview_core::document::parse(md0.clone());
	eprintln!(
		"full parse ({} bytes): {} blocks",
		md0.len(),
		doc0.blocks.len()
	);
	for b in &doc0.blocks {
		eprintln!("  src={:?} id={:016x}", b.source, b.id);
	}

	// Same source through reparse (no edit): must be identical.
	let same = markview_core::document::reparse(&doc0, Arc::from(md0.clone()));
	eprintln!(
		"reparse(same): {} blocks, same content id: {}",
		same.blocks.len(),
		same.content_id == doc0.content_id
	);
	for b in &same.blocks {
		eprintln!("  src={:?} id={:016x}", b.source, b.id);
	}

	// The deterministic edit the `reparse` target applies.
	let edit = mvfuzz::edit::apply_deterministic_edit(&data);
	let md1 = edit.md1;
	let full = markview_core::document::parse(md1.clone());
	let incr = markview_core::document::reparse(&doc0, Arc::from(md1.clone()));
	eprintln!(
		"edit ({}): full={} incremental={} equal={}",
		edit.description,
		full.blocks.len(),
		incr.blocks.len(),
		full.content_id == incr.content_id
	);
	for b in &incr.blocks {
		eprintln!("  src={:?} id={:016x}", b.source, b.id);
	}
}
