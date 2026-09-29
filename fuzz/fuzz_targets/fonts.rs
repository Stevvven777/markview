//! G5, Tier 1 + Tier 2: font validation. The unit carries the bytes; the
//! target feeds them to the byte-level checks and to the directory scan the
//! shaper performs, with up to four candidate files. Oracle (O1, O2): a
//! malformed font degrades to "not a font", never panics.
#![no_main]

use libfuzzer_sys::fuzz_target;
use markview_core::fonts::{self, FontConfig};
use mvfuzz::{budget, oracle};

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::parse().from_env();
	if data.len() > 1_000_000 {
		return;
	}
	let guard = budget::InputGuard::new();
	let _ = fonts::is_font(data);
	let _ = fonts::is_postscript_outline(data);
	// The directory scan: derived file names and contents, so a hostile
	// header and a hostile name meet in the same scan.
	let dir = tempfile::tempdir().expect("tempdir");
	let seed = oracle::derive(data);
	let n = 1 + (seed as usize % 4);
	for i in 0..n {
		let name = sanitize_name(data, i, n);
		let start = (i * data.len()) / n;
		let end = if i + 1 == n {
			data.len()
		} else {
			(i + 1) * data.len() / n
		};
		std::fs::write(dir.path().join(name), &data[start..end]).ok();
	}
	// `families` drives the shaper's own scan-and-register path over the
	// directory, which is where a malformed face is dropped; the names it
	// reports must never be empty.
	let config = FontConfig {
		ignore_system_fonts: true,
		directories: vec![dir.path().to_owned()],
		..Default::default()
	};
	for family in fonts::families(&config, false).iter() {
		assert!(!family.is_empty());
	}
	guard.finish(&budget, data.len());
});

/// A file name the scan will accept, derived from the input at slot `i`.
fn sanitize_name(data: &[u8], i: usize, n: usize) -> String {
	let base = match data.get(i % data.len().max(1)) {
		Some(b'0') => "a.ttf",
		Some(b'1') => "b.otf",
		Some(b'2') => "c.ttf",
		Some(b'3') => "d.woff2",
		Some(_) => "e.ttf",
		None => "f.ttf",
	};
	if n > 1 {
		format!("{i}-{base}")
	} else {
		base.to_string()
	}
}
