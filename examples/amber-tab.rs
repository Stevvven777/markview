//! An isolated native tab specimen with a cached normalized color function.
use markview_core::{
	image::ColorField,
	scene::{Draw, LayoutSnapshot, Paint, Rect},
	shaping::TextShaper,
	style::{Color, ColorField as C, Condition as K, Stylesheet},
};
use markview_render::{Renderer, Theme, View};
use std::{
	collections::HashMap, num::NonZeroU32, path::Path, sync::Arc, time::Instant,
};

fn with_alpha(color: Color, alpha: f32) -> Color {
	Color((color.0 & 0xFFFFFF00) | (alpha.clamp(0., 1.) * 255.).round() as u32)
}

fn rounded_alpha(x: f32, y: f32, scale: f32) -> f32 {
	// The rounded mask uses logical distances; only antialiasing follows DPI.
	let qx = ((x - 0.5) * 178.).abs() - 84.;
	let qy = ((y - 0.5) * 30.).abs() - 10.;
	let distance = qx.max(0.).hypot(qy.max(0.)) + qx.max(qy).min(0.) - 5.;
	(0.5 - distance * scale).clamp(0., 1.)
}

fn main() -> anyhow::Result<()> {
	let directory =
		Path::new(env!("CARGO_MANIFEST_DIR")).join("artifacts/amber-tabs");
	std::fs::create_dir_all(&directory)?;
	let mut sheet = (*Stylesheet::bundled(false)).clone();
	sheet.merge(&Stylesheet::parse(include_str!("amber-tab.mvss.toml"))?);
	let sheet = Arc::new(sheet);
	let mut ui = TextShaper::new();
	ui.set_stylesheet(sheet.clone());
	let mut renderer = pollster::block_on(Renderer::new(None))?;
	renderer.set_stylesheet(sheet);
	let rect = Rect {
		x: 24.,
		y: 16.,
		w: 180.,
		h: 32.,
	};
	let inner = Rect {
		x: 25.,
		y: 17.,
		w: 178.,
		h: 30.,
	};
	let mut foreground =
		ui.label("Amber.md", 12., 50., 37., Paint::Styled(K::Panel, C::Color));
	foreground.push(Draw::Icon {
		paths: markview_icon::icon!("assets/ui/close.svg"),
		paint: Paint::Styled(K::Panel, C::Muted),
		x: 182.,
		y: 24.,
		size: 16.,
	});
	for (x, w, color) in [(36., 28., 0x864C24FF), (68., 14., 0xD2C2ACFF)] {
		foreground.push(Draw::Rect(
			Rect {
				x,
				y: 44.,
				w,
				h: 1.,
			},
			Paint::Color(Color(color)),
		));
	}
	let horizontal = HashMap::new();
	for (index, scale) in [1., 1.25, 3.].into_iter().enumerate() {
		let start = Instant::now();
		let field = ColorField::builder([
			NonZeroU32::new((inner.w * scale).ceil() as u32).unwrap(),
			NonZeroU32::new((inner.h * scale).ceil() as u32).unwrap(),
		])
		.paint(|_, _| Color(0xEFE0CDFF))
		.paint(|x, y| {
			let grain = (y * 96. + (x * 12.).sin() * 0.7).sin() * 0.5 + 0.5;
			with_alpha(Color(0xD2C2ACFF), 0.04 + 0.08 * grain)
		})
		.paint(|x, y| {
			let glow = (-((x - 0.085) / 0.18).powi(2)
				- ((y - 0.45) / 0.7).powi(2))
			.exp();
			with_alpha(Color(0xC37C54FF), glow * 0.16)
		})
		.paint(|x, y| {
			let light = (-((x - 0.085) / 0.022).powi(2)
				- ((y - 0.45) / 0.13).powi(2))
			.exp();
			with_alpha(Color(0xF0EAD6FF), light * 0.9)
		})
		.map(|x, y, color| with_alpha(color, rounded_alpha(x, y, scale)))
		.finish();
		let raster_ms = start.elapsed().as_secs_f64() * 1000.;
		let mut snapshot = LayoutSnapshot::default();
		let mut draws = vec![
			Draw::Box {
				rect,
				chain: K::Panel.chain(),
				condition: K::Panel,
				radius: 6.,
				border: 1.,
				left_only: false,
				decoration: None,
			},
			field.draw(
				inner,
				"ui:amber-tab",
				index as u64 + 1,
				&mut snapshot.images,
			),
		];
		draws.extend(foreground.clone());
		let view = View {
			width: (228. * scale) as u32,
			height: (64. * scale) as u32,
			scale,
			left: 0.,
			top: 0.,
			bottom: 0.,
			scroll: 0.,
			theme: Theme::Light,
			horizontal: &horizontal,
			selection: None,
			revision: 0,
			hovered_link: None,
			hovered_overflow: None,
			held_overflow: None,
		};
		let target = renderer.offscreen(view.width, view.height);
		let submission = renderer.render(
			&snapshot,
			&view,
			&draws,
			&target.create_view(&Default::default()),
		)?;
		renderer.wait(Some(submission))?;
		let pixels = renderer.read_pixels(&target)?;
		let pixel = |x: u32, y: u32| -> &[u8] {
			let offset = ((y * pixels.width + x) * 4) as usize;
			&pixels.rgba[offset..offset + 4]
		};
		assert_eq!(
			pixel((24. * scale) as u32, (16. * scale) as u32),
			pixel(0, 0)
		);
		renderer.save_png(
			&target,
			&directory.join(format!("amber-field-{scale}x.png")),
		)?;
		let bytes = renderer.gpu_bytes();
		let start = Instant::now();
		for _ in 0..100 {
			let submission = renderer.render(
				&snapshot,
				&view,
				&draws,
				&target.create_view(&Default::default()),
			)?;
			renderer.wait(Some(submission))?;
		}
		assert_eq!(renderer.gpu_bytes(), bytes);
		println!(
			"{scale}x: raster {raster_ms:.3} ms; cached frame {:.3} ms; GPU bytes {bytes}",
			start.elapsed().as_secs_f64() * 10.
		);
	}
	Ok(())
}
