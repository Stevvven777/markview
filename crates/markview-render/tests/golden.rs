#![cfg(target_os = "linux")]

use anyhow::{Context, Result, bail};
use image::{Rgba, RgbaImage};
use markview_core::{
	background::Direct,
	document,
	fonts::FontConfig,
	image::{ImageInfo, ImageSnapshot, Pixels},
	layout::{LayoutEngine, LayoutOptions},
	style::Stylesheet,
};
use markview_render::{Renderer, Theme, View};
use std::{collections::HashMap, fs, path::Path, sync::Arc};

#[test]
fn markdown_matches_rendering_baselines() -> Result<()> {
	let root = Path::new(env!("CARGO_MANIFEST_DIR"));
	let artifacts = root.join("../../artifacts/render-goldens");
	if artifacts.exists() {
		fs::remove_dir_all(&artifacts)?;
	}
	fs::create_dir_all(&artifacts)?;
	let update =
		std::env::var("MARKVIEW_UPDATE_RENDER_GOLDENS").as_deref() == Ok("1");
	let mut files = fs::read_dir(root.join("../markview-core/tests/fonts"))?
		.collect::<std::io::Result<Vec<_>>>()?;
	files.sort_by_key(|file| file.file_name());
	let faces = files
		.into_iter()
		.map(|file| {
			fs::read(file.path())
				.map(|data| parley::fontique::Blob::new(Arc::new(data)))
		})
		.collect::<std::io::Result<_>>()?;
	let fonts = FontConfig::from_faces(0x676f6c64656e, faces);
	let tiles = image::load_from_memory(include_bytes!("fixtures/tiles.png"))?
		.into_rgba8();
	let mut images = ImageSnapshot::default();
	images.entries.insert(
		"tiles.png".into(),
		ImageInfo {
			version: 1,
			size: Some(tiles.dimensions()),
			error: None,
		},
	);
	images.pixels.insert(
		"tiles.png".into(),
		1,
		Arc::new(Pixels {
			width: tiles.width(),
			height: tiles.height(),
			rgba: tiles.into_raw().into(),
		}),
	);
	let mut renderer = pollster::block_on(Renderer::new(None))?;
	assert!(
		renderer.adapter_name.starts_with("llvmpipe")
			&& renderer.adapter_name.ends_with("(Vulkan, Cpu)"),
		"Golden tests require Lavapipe; select its ICD with VK_DRIVER_FILES. Got {}",
		renderer.adapter_name
	);
	eprintln!("Golden adapter: {}", renderer.adapter_name);
	let mut failures = Vec::new();
	for (name, fixture, width, dark, scale) in [
		("prose", "prose", 560., false, 1.),
		("lists", "lists", 560., false, 1.),
		("table", "table", 560., false, 1.),
		("code", "code", 560., false, 1.),
		("math", "math", 560., false, 1.),
		("images", "images", 560., false, 1.),
		("prose-narrow", "prose", 320., false, 1.),
		("prose-dark-125", "prose", 560., true, 1.25),
	] {
		let height = if width < 400. { 960. } else { 640. };
		let doc = document::parse(fs::read_to_string(
			root.join(format!("tests/fixtures/{fixture}.md")),
		)?);
		let sheet = Stylesheet::bundled(dark);
		renderer.set_stylesheet(sheet.clone());
		let options = LayoutOptions {
			width: width - 40.,
			fonts: fonts.clone(),
			stylesheet: sheet,
			..Default::default()
		};
		let mut engine =
			LayoutEngine::with_executor(Arc::new(Direct), Arc::new(|| {}));
		let snapshot = engine.layout_with_images(&doc, &options, &images);
		let snapshot = if engine.wait_highlights() {
			engine.layout_with_images(&doc, &options, &images)
		} else {
			snapshot
		};
		let horizontal = HashMap::new();
		let view = View {
			width: (width * scale) as u32,
			height: (height * scale) as u32,
			scale,
			left: 20.,
			top: 20.,
			bottom: 20.,
			scroll: 0.,
			theme: if dark { Theme::Dark } else { Theme::Light },
			horizontal: &horizontal,
			selection: None,
			revision: 1,
			hovered_link: None,
			hovered_overflow: None,
			held_overflow: None,
		};
		assert!(
			snapshot.height <= height - 40.,
			"{name}: fixture is clipped"
		);
		let target = renderer.offscreen(view.width, view.height);
		let submission = renderer.render(
			&snapshot,
			&view,
			&[],
			&target.create_view(&Default::default()),
		)?;
		renderer.wait(Some(submission))?;
		let pixels = renderer.read_pixels(&target)?;
		let actual =
			RgbaImage::from_raw(pixels.width, pixels.height, pixels.rgba)
				.unwrap();
		let background = actual.get_pixel(0, 0);
		assert!(
			actual.pixels().filter(|pixel| *pixel != background).count() > 1000,
			"{name}: frame is empty"
		);
		let baseline = root.join(format!("tests/goldens/{name}.png"));
		actual.save(artifacts.join(format!("{name}-actual.png")))?;
		if update {
			fs::create_dir_all(baseline.parent().unwrap())?;
			actual.save(&baseline)?;
		} else if let Err(error) = compare(&baseline, &actual, &artifacts, name)
		{
			failures.push(format!("{name}: {error:#}"));
		}
	}
	if !failures.is_empty() {
		bail!("{}\nInspect {}", failures.join("\n"), artifacts.display());
	}
	Ok(())
}

fn compare(
	baseline: &Path,
	actual: &RgbaImage,
	artifacts: &Path,
	name: &str,
) -> Result<()> {
	let expected = image::open(baseline)
		.with_context(|| {
			format!("Missing or invalid baseline {}", baseline.display())
		})?
		.into_rgba8();
	if expected == *actual {
		return Ok(());
	}
	let (w, h) = (
		expected.width().max(actual.width()),
		expected.height().max(actual.height()),
	);
	let mut different = 0;
	let mut bounds = (w, h, 0, 0);
	let diff = RgbaImage::from_fn(w, h, |x, y| {
		let before = expected.get_pixel_checked(x, y);
		let after = actual.get_pixel_checked(x, y);
		// Allow one RGBA8 level for cross-driver rounding.
		let matches = match (before, after) {
			(Some(before), Some(after)) => before
				.0
				.iter()
				.zip(after.0)
				.all(|(a, b)| a.abs_diff(b) <= 1),
			_ => before == after,
		};
		if matches {
			Rgba([0, 0, 0, 255])
		} else {
			different += 1;
			bounds = (
				bounds.0.min(x),
				bounds.1.min(y),
				bounds.2.max(x),
				bounds.3.max(y),
			);
			Rgba([255, 0, 255, 255])
		}
	});
	if different == 0 {
		return Ok(());
	}
	expected.save(artifacts.join(format!("{name}-expected.png")))?;
	diff.save(artifacts.join(format!("{name}-diff.png")))?;
	bail!(
		"{different} differing pixels, bounds {bounds:?}; expected {:?}, actual {:?}",
		expected.dimensions(),
		actual.dimensions()
	);
}

#[test]
fn comparisons_reject_pixel_changes_dimensions_and_missing_baselines()
-> Result<()> {
	let dir = tempfile::tempdir()?;
	let baseline = dir.path().join("baseline.png");
	let expected = RgbaImage::from_pixel(2, 2, Rgba([40, 50, 60, 255]));
	expected.save(&baseline)?;
	let original = fs::read(&baseline)?;
	compare(&baseline, &expected, dir.path(), "identical")?;
	let mut changed = expected.clone();
	changed.get_pixel_mut(1, 0).0[0] += 1;
	compare(&baseline, &changed, dir.path(), "rounding")?;
	changed.get_pixel_mut(1, 0).0[0] += 1;
	let error = compare(&baseline, &changed, dir.path(), "pixel").unwrap_err();
	assert!(
		error
			.to_string()
			.contains("1 differing pixels, bounds (1, 0, 1, 0)")
	);
	let diff = image::open(dir.path().join("pixel-diff.png"))?.into_rgba8();
	assert_eq!(diff.get_pixel(1, 0).0, [255, 0, 255, 255]);
	assert_eq!(diff.get_pixel(0, 0).0, [0, 0, 0, 255]);
	assert_eq!(
		image::open(dir.path().join("pixel-expected.png"))?.into_rgba8(),
		expected
	);
	let smaller = RgbaImage::from_pixel(1, 2, Rgba([40, 50, 60, 255]));
	assert!(compare(&baseline, &smaller, dir.path(), "dimensions").is_err());
	assert_eq!(fs::read(&baseline)?, original);
	let missing = dir.path().join("missing.png");
	assert!(compare(&missing, &expected, dir.path(), "missing").is_err());
	assert!(!missing.exists());
	Ok(())
}
