use super::Images;
use crate::test_support::render_goldens::{Baselines, capture};
use crate::{
	document,
	layout::{LayoutEngine, LayoutOptions},
	render::{Renderer, Theme, View},
};
use anyhow::{Result, ensure};
use markview_core::{
	background::Direct,
	style::{CjkType, Stylesheet},
};
use std::{collections::HashMap, fs, path::Path, sync::Arc};

#[test]
fn diagrams_match_rendering_baselines() -> Result<()> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let mut baselines = Baselines::new(
		root.join("tests/goldens/diagrams"),
		root.join("artifacts/diagram-goldens"),
	)?;
	let mut renderer = pollster::block_on(Renderer::new(None))?;
	ensure!(
		renderer.adapter_name.starts_with("llvmpipe")
			&& renderer.adapter_name.ends_with("(Vulkan, Cpu)"),
		"Diagram baselines require Lavapipe"
	);
	let fonts = crate::test_support::fonts();
	let mut images = Images::with_cache(true, None);
	for preset in ["default", "dark", "forest", "neutral", "modern", "custom"] {
		let mut sheet = (*Stylesheet::bundled(preset == "dark")).clone();
		sheet.set_cjk_type(CjkType::Sc);
		let rules = if preset == "custom" {
			fs::read_to_string(
				root.join("tests/fixtures/diagrams/custom.mvss.toml"),
			)?
		} else {
			format!(
				"format_version=2\nversion=1\n[mermaid]\ntheme='{preset}'\nfont_family=['serif']"
			)
		};
		sheet.merge(&Stylesheet::parse(&rules)?);
		let sheet = Arc::new(sheet);
		renderer.set_stylesheet(sheet.clone());
		for kind in ["flowchart", "sequence", "git", "pie", "svg"] {
			let name = format!("{kind}-{preset}");
			let path = root.join(format!("tests/fixtures/diagrams/{kind}.md"));
			let doc = document::parse(fs::read_to_string(&path)?);
			images.prepare(&doc, &path, 1, false, &sheet, &fonts);
			images.wait();
			ensure!(
				!images.snapshot.entries.is_empty(),
				"{name}: no images parsed"
			);
			for info in images.snapshot.entries.values() {
				ensure!(
					info.error.is_none() && info.size.is_some(),
					"{name}: image failed: {info:?}"
				);
			}
			ensure!(
				images.snapshot.decoded().len()
					== images.snapshot.entries.len(),
				"{name}: pixels did not settle"
			);
			let options = LayoutOptions {
				width: 720.,
				stylesheet: sheet.clone(),
				fonts: fonts.clone(),
				..Default::default()
			};
			let mut engine =
				LayoutEngine::with_executor(Arc::new(Direct), Arc::new(|| {}));
			let snapshot =
				engine.layout_with_images(&doc, &options, &images.snapshot);
			ensure!(snapshot.height < 960., "{name}: clipped diagram");
			let horizontal = HashMap::new();
			let view = View {
				width: 760,
				height: 1000,
				scale: 1.,
				left: 20.,
				top: 20.,
				bottom: 20.,
				scroll: 0.,
				theme: Theme::Light,
				horizontal: &horizontal,
				selection: None,
				revision: 1,
				hovered_link: None,
				hovered_overflow: None,
				held_overflow: None,
			};
			let actual = capture(&mut renderer, &snapshot, &view, &[])?;
			baselines.record(&name, &actual)?;
		}
	}
	baselines.finish()
}
