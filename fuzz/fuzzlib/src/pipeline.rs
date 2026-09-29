//! Pipeline helpers shared by the layout, differential, and PDF targets.
//!
//! Layout options are derived deterministically from the input's content, so
//! every comparison inside one process uses the same options, and every
//! corner of `Limits` gets visited as inputs mutate. Fonts are the committed
//! subsets, never the host's set, so results are reproducible across
//! machines.

use std::{
	path::Path,
	sync::{Arc, Once},
};

use markview_core::{
	document,
	fonts::FontConfig,
	layout::{LayoutEngine, LayoutOptions, LayoutSnapshot},
	limits::Limits,
	paginate::{PageGeometry, Pagination, paginate},
	style::{CjkType, Stylesheet},
};
use markview_pdf::{Export, Metadata};

use crate::oracle::derive;

/// The committed test faces; exports and layouts must not depend on what the
/// machine happens to have installed.
pub fn pinned_fonts() -> FontConfig {
	FontConfig {
		ignore_system_fonts: true,
		directories: vec![
			Path::new(env!("CARGO_MANIFEST_DIR"))
				.join("../../crates/markview-core/tests/fonts")
				.to_path_buf(),
		],
		..Default::default()
	}
}

/// Layout options derived from the input's hash: the reading width and size
/// vary, the `Limits` budgets are shrunk toward their floors so degradation
/// paths are reachable, and the reader toggles exercise the option bits.
pub fn options_for(md: &str) -> LayoutOptions {
	let seed = derive(md.as_bytes());
	let pick = |shift: u32, n: u128| (seed >> shift) % n;
	LayoutOptions {
		width: 120.0 + pick(32, 2600) as f32,
		font_size: 10.0 + pick(40, 40) as f32,
		justify: pick(48, 2) != 0,
		hyphenate: pick(49, 2) != 0,
		paragraph_indent: (pick(50, 8) as f32) / 4.0,
		greedy: pick(51, 4) == 0,
		codeblock_wrap: pick(52, 2) != 0,
		force_open: pick(53, 4) == 0,
		limits: crate::oracle::shrunk_limits(seed),
		// `LayoutOptions::default()` falls back to the host's fonts, which
		// would make the snapshot depend on what this machine happens to have
		// installed; the committed test faces keep comparisons stable.
		fonts: pinned_fonts(),
		..Default::default()
	}
}

/// One layout of a throwaway document, run once per process: the first
/// layout pays for fontconfig's cold caches, which is a warm-up cost, not
/// an input cost, so the layout-family targets call this before their
/// input guard starts.
pub fn warmup() {
	static ONCE: Once = Once::new();
	ONCE.call_once(|| {
		// A fenced block makes this one-time pass pay for fontconfig's cold
		// caches and the syntax-set build, which are warm-up costs, not input
		// costs, so the layout-family targets run it before their guard starts.
		let doc = document::parse(Arc::from("```rust\nlet x = 1;\n```\n"));
		let mut engine = LayoutEngine::new();
		let mut snapshot = engine.layout(&doc, &options_for("warm"));
		if engine.wait_highlights() {
			snapshot = engine.layout(&doc, &options_for("warm"));
		}
		drop(snapshot);
		// The syntax-state build above covers the warm languages; the rest
		// of the set would otherwise cost its first input ~200 ms inside
		// the worker, where the input budget would see it.
		markview_core::prewarm_highlight();
	});
}

/// Parses and lays out `md` with the derived options, waiting for the
/// asynchronous highlight pass so the geometry is the settled one.
pub fn parse_layout(
	md: &str,
) -> (document::Document, LayoutOptions, LayoutSnapshot) {
	let doc = document::parse(md.to_string());
	let options = options_for(md);
	let mut engine = LayoutEngine::new();
	let mut snapshot = engine.layout(&doc, &options);
	if engine.wait_highlights() {
		snapshot = engine.layout(&doc, &options);
	}
	(doc, options, snapshot)
}

/// One export: the print stylesheet, page geometry, pagination, and the PDF
/// bytes. Mirrors the reader's export path, including the highlight wait.
pub fn export_pdf(
	md: &str,
) -> (
	Vec<u8>,
	document::Document,
	LayoutSnapshot,
	Pagination,
	PageGeometry,
) {
	let sheet = {
		let mut sheet = (*Stylesheet::bundled_print()).clone();
		sheet.set_cjk_type(CjkType::Sc);
		Arc::new(sheet)
	};
	let document = document::parse(md.to_string());
	let geometry =
		PageGeometry::from_style(sheet.page()).expect("print page is valid");
	// The derived `Limits` keep the export's degradation paths reachable;
	// the page geometry stays the stylesheet's, like the reader's export.
	let seed = derive(md.as_bytes());
	let pick = |shift: u32, n: u128| (seed >> shift) % n;
	let options = LayoutOptions {
		width: geometry.text_px().0,
		codeblock_wrap: true,
		force_open: true,
		hide_front_matter: true,
		limits: Limits {
			inline_depth: 32 + pick(64, 512) as usize,
			block_depth: 8 + pick(68, 260) as usize,
			linebreak_evaluations: 1000 + pick(72, 3_000_000) as usize,
			highlight_line_bytes: 64 + pick(76, 65_536) as usize,
			highlight_bytes: 4096 + pick(80, 16 * 1024 * 1024) as usize,
			math_formula_bytes: 64 + pick(84, 256 * 1024) as usize,
			math_bytes: 1024 + pick(88, 8 * 1024 * 1024) as usize,
			table_columns: 4 + pick(92, 512) as usize,
			table_rows: 8 + pick(96, 16_384) as usize,
			table_cells: 16 + pick(100, 131_072) as usize,
		},
		stylesheet: sheet.clone(),
		fonts: pinned_fonts(),
		..Default::default()
	};
	let mut engine = LayoutEngine::new();
	let mut snapshot = engine.layout(&document, &options);
	if engine.wait_highlights() {
		snapshot = engine.layout(&document, &options);
	}
	let pagination = paginate(&document, &snapshot, &geometry);
	let bytes = markview_pdf::export(Export {
		snapshot: &snapshot,
		images: &Default::default(),
		prepared_images: None,
		stylesheet: &sheet,
		geometry: &geometry,
		pagination: &pagination,
		metadata: Metadata {
			title: Some("Fuzz".into()),
			..Default::default()
		},
		path: "fuzz.md".into(),
		body_size_px: options.font_size,
		links: true,
		fonts: pinned_fonts(),
	})
	.expect("the export succeeds");
	(bytes, document, snapshot, pagination, geometry)
}
