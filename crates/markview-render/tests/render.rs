use markview_core::{
	background::Direct,
	document,
	fonts::FontConfig,
	layout::{LayoutEngine, LayoutOptions},
	style::Stylesheet,
};
use markview_render::{Renderer, Theme, View};
use std::{collections::HashMap, sync::Arc};

#[test]
fn markdown_renders_visible_and_identical_cached_frames() -> anyhow::Result<()>
{
	let fonts = FontConfig::from_faces(
		0x72656e646572,
		[
			include_bytes!(
				"../../markview-core/tests/fonts/NotoSerif-Regular-subset.otf"
			)
			.as_slice(),
			include_bytes!(
				"../../markview-core/tests/fonts/NotoSans-Bold-subset.otf"
			)
			.as_slice(),
			include_bytes!(
				"../../markview-core/tests/fonts/NotoSansMono-Regular-subset.otf"
			)
			.as_slice(),
		]
		.into_iter()
		.map(|data| parley::fontique::Blob::new(Arc::new(data)))
		.collect(),
	);
	let doc = document::parse(
		"# Rendering\n\nA paragraph with $x^2$.\n\n- first\n- second\n\n```rust\nlet answer = 42;\n```\n",
	);
	let mut engine =
		LayoutEngine::with_executor(Arc::new(Direct), Arc::new(|| {}));
	let mut renderer = pollster::block_on(Renderer::new(None))?;
	eprintln!("Render test adapter: {}", renderer.adapter_name);
	let horizontal = HashMap::new();
	for (theme, scale) in [(Theme::Light, 1.0), (Theme::Dark, 1.25)] {
		let sheet = Stylesheet::bundled(theme == Theme::Dark);
		renderer.set_stylesheet(sheet.clone());
		let options = LayoutOptions {
			width: 360.0,
			fonts: fonts.clone(),
			stylesheet: sheet,
			..Default::default()
		};
		let snapshot = engine.layout(&doc, &options);
		let snapshot = if engine.wait_highlights() {
			engine.layout(&doc, &options)
		} else {
			snapshot
		};
		let view = View {
			width: (400.0 * scale) as u32,
			height: (500.0 * scale) as u32,
			scale,
			left: 20.0,
			top: 20.0,
			bottom: 20.0,
			scroll: 0.0,
			theme,
			horizontal: &horizontal,
			selection: None,
			revision: 1,
			hovered_link: None,
			hovered_overflow: None,
			held_overflow: None,
		};
		let target = renderer.offscreen(view.width, view.height);
		let submission = renderer.render(
			&snapshot,
			&view,
			&[],
			&target.create_view(&Default::default()),
		)?;
		renderer.wait(Some(submission))?;
		let first = renderer.read_pixels(&target)?;
		assert_eq!((first.width, first.height), (view.width, view.height));
		let background = &first.rgba[..4];
		let visible = first
			.rgba
			.as_chunks::<4>()
			.0
			.iter()
			.filter(|p| p.as_slice() != background)
			.count();
		assert!(visible > 1000, "Markdown frame is empty: {visible} pixels");
		let cached = engine.layout(&doc, &options);
		assert_eq!(cached.reused, cached.blocks.len());
		let submission = renderer.render(
			&cached,
			&view,
			&[],
			&target.create_view(&Default::default()),
		)?;
		renderer.wait(Some(submission))?;
		assert_eq!(first.rgba, renderer.read_pixels(&target)?.rgba);
	}
	Ok(())
}
