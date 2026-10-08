//! An isolated native tab specimen with a normalized GPU fragment program.
use markview_core::{
	scene::{ColorField, Draw, LayoutSnapshot, Paint, Rect},
	shaping::TextShaper,
	style::{Color, ColorField as C, Condition as K, Stylesheet},
};
use markview_render::{Renderer, Theme, View};
use std::{collections::HashMap, path::Path, sync::Arc, time::Instant};

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
	let field = ColorField::builder()
        .paint("return rgba(0xEFE0CDFFu);")
        .paint(r#"
            let grain = sin(y * 96.0 + sin(x * 12.0) * 0.7) * 0.5 + 0.5;
            return vec4<f32>(rgba(0xD2C2ACFFu).rgb, 0.04 + 0.08 * grain);
        "#)
        .paint(r#"
            let p = (vec2<f32>(x, y) - vec2<f32>(0.085, 0.45)) / vec2<f32>(0.18, 0.7);
            return vec4<f32>(rgba(0xC37C54FFu).rgb, exp(-dot(p, p)) * 0.16);
        "#)
        .paint(r#"
            let p = (vec2<f32>(x, y) - vec2<f32>(0.085, 0.45)) / vec2<f32>(0.022, 0.13);
            return vec4<f32>(rgba(0xF0EAD6FFu).rgb, exp(-dot(p, p)) * 0.9);
        "#)
        .map(r#"
            let q = abs((vec2<f32>(x, y) - vec2<f32>(0.5)) * size) - (size * 0.5 - vec2<f32>(5.0));
            let distance = length(max(q, vec2<f32>(0.0))) + min(max(q.x, q.y), 0.0) - 5.0;
            return vec4<f32>(color.rgb, color.a * clamp(0.5 - distance * scale, 0.0, 1.0));
        "#)
        .finish();
	for scale in [1., 1.25, 3.] {
		let snapshot = LayoutSnapshot::default();
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
			field.draw(inner),
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
		assert_eq!(renderer.color_field_stats(), (1, 1));
		println!(
			"{scale}x: cached frame {:.3} ms; tracked GPU bytes {bytes}; field pipelines {:?}",
			start.elapsed().as_secs_f64() * 10.,
			renderer.color_field_stats()
		);
	}
	Ok(())
}
