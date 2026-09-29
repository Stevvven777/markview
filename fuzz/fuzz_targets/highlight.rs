//! G4, Tier 1 + Tier 2: syntax highlighting through syntect, reached the way
//! the reader reaches it — a fenced code block in a layout pass. The unit is
//! `language \0 code`: the first NUL separates the two. Oracle (O1, O2, I7):
//! no panic within budgets; the uncolored fallback is a legal outcome.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mvfuzz::{budget, oracle, pipeline};

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::layout().from_env();
	let (lang, code) = oracle::split_nul(data);
	if lang.len() + code.len() > 128 * 1024 {
		return;
	}
	let md = format!("```{lang}\n{code}\n```\n");
	pipeline::warmup();
	let guard = budget::InputGuard::new();
	let (_doc, _options, snapshot) = pipeline::parse_layout(&md);
	oracle::assert_snapshot_finite(&snapshot);
	guard.finish(&budget, md.len());
});
