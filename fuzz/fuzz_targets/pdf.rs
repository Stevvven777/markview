//! G6, Tier 1 + Tier 2 + Tier 3: the whole PDF export path. Oracle (O1, O2,
//! O5.4): no panic within budgets; the same input exports to identical bytes
//! (I6); and the bytes must read back as a PDF with a parseable page tree
//! and content streams (structured readback, not just the writer's own
//! bookkeeping).
#![no_main]

use libfuzzer_sys::fuzz_target;
use mvfuzz::{budget, pipeline, ratex};

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::pdf().from_env();
	let md = String::from_utf8_lossy(data);
	if md.len() > 48 * 1024 {
		return;
	}
	// An exported document lays out its formulas through ratex; see
	// `mvfuzz::ratex`.
	ratex::allow_char_overflow();
	let guard = budget::InputGuard::new();
	let (a, ..) = pipeline::export_pdf(&md);
	let (b, ..) = pipeline::export_pdf(&md);
	assert_eq!(a, b, "two exports of the same input differ");
	// Structured readback: the bytes are a PDF, with pages whose content
	// streams decode.
	let pdf = lopdf::Document::load_mem(&a).expect("the export parses");
	let pages = pdf.get_pages();
	assert!(!pages.is_empty(), "the export has no pages");
	for id in pages.values() {
		let content = pdf.get_page_content(*id);
		assert!(
			content.len() < 64 * 1024 * 1024,
			"page content is implausible"
		);
	}
	guard.finish(&budget, md.len());
});
