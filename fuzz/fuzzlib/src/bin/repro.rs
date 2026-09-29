//! Reproduce a reparse divergence: applies the same deterministic edit as
//! the `reparse` fuzz target and prints field-by-field differences.
//!
//! Run: `cargo run -p mvfuzz --bin repro -- <file>`

use std::sync::Arc;

fn main() {
	let args: Vec<String> = std::env::args().collect();
	let data = std::fs::read(&args[1]).unwrap();
	let md0 = String::from_utf8_lossy(&data).into_owned();
	let seed = mvfuzz::oracle::derive(&data);
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
	eprintln!("edit: line {at:?} <- {line:?}");

	let doc0 = markview_core::document::parse(md0);
	let full = markview_core::document::parse(md1.clone());
	let incr = markview_core::document::reparse(&doc0, Arc::from(md1));

	println!(
		"content_id full={:#x} incr={:#x}",
		full.content_id, incr.content_id
	);
	println!(
		"blocks: full={} incr={}",
		full.blocks.len(),
		incr.blocks.len()
	);
	for (i, (f, x)) in full.blocks.iter().zip(&incr.blocks).enumerate() {
		let same = f.id == x.id
			&& f.source == x.source
			&& f.content_key == x.content_key;
		if !same {
			println!("block {i} differs:");
			println!(
				"  full: id={:#x} source={:?} key={:#x}",
				f.id, f.source, f.content_key
			);
			println!(
				"  incr: id={:#x} source={:?} key={:#x}",
				x.id, x.source, x.content_key
			);
			println!("  full kind: {:?}", f.kind);
			println!("  incr kind: {:?}", x.kind);
		}
	}
}
