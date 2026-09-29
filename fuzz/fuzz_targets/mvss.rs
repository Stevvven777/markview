//! G2, Tier 1: MVSS stylesheet parsing — `format_version`, the
//! targets / fontdef / font-family / meta / svg / page / mermaid / rule
//! tables, and the numeric, URL and digest validation. Oracle (O1, O2):
//! invalid sheets must return `Err`, never panic, and within budgets.
#![no_main]

use libfuzzer_sys::{fuzz_mutator, fuzz_target};
use markview_core::style::{Condition, Stylesheet};
use mvfuzz::{budget, mutators};

fuzz_mutator! { |data: &mut [u8], size: usize, max_size: usize, seed: u32| {
	mutators::mvss(data, size, max_size, seed)
}}

fuzz_target!(|data: &[u8]| {
	let budget = budget::Budget::parse().from_env();
	let text = String::from_utf8_lossy(data);
	if text.len() > 128 * 1024 {
		return;
	}
	let guard = budget::InputGuard::new();
	match Stylesheet::parse(&text) {
		Err(_) => {}
		Ok(sheet) => {
			// Query every condition and the worst-case element chain, so a
			// mis-indexed rule table faults here rather than in the app.
			for (condition, _) in Condition::ALL {
				let _ = sheet.rule(*condition);
				let _ = sheet.element_rule(u128::MAX, *condition);
			}
			let _ = sheet.paint(markview_core::scene::Paint::Background);
			let _ = sheet.color(
				Condition::Body,
				markview_core::style::ColorField::Color,
			);
			let _ = sheet.scrollbar_metrics();
			let _ = sheet.scrollbar_gutter();
			let _ = sheet.list_indent(false);
			let _ = sheet.marker_shapes();
			let _ = sheet.enum_numbering();
			// Cascading must not panic or lose the print furniture.
			let mut merged = sheet;
			merged.merge(&Stylesheet::bundled(false));
			let _ = merged.layout_key();
			let _ = merged.diagram_key();
		}
	}
	guard.finish(&budget, text.len());
});
