use markview_core::{
	image::{ImageInfo, Pixels},
	scene::{ColorField, Draw, LayoutSnapshot, Paint, Rect},
	style::Color,
};
use markview_render::{Renderer, Theme, View};
use std::{collections::HashMap, sync::Arc};

fn view(horizontal: &HashMap<(usize, usize), f32>, scale: f32) -> View<'_> {
	View {
		width: (12. * scale) as u32,
		height: (12. * scale) as u32,
		scale,
		scroll: 0.,
		left: 0.,
		top: 0.,
		bottom: 0.,
		theme: Theme::Light,
		horizontal,
		selection: None,
		revision: 0,
		hovered_link: None,
		hovered_overflow: None,
		held_overflow: None,
	}
}

fn render(
	renderer: &mut Renderer,
	snapshot: &LayoutSnapshot,
	view: &View<'_>,
	draws: &[Draw],
) -> Vec<u8> {
	let target = renderer.offscreen(view.width, view.height);
	let submission = renderer
		.render(
			snapshot,
			view,
			draws,
			&target.create_view(&Default::default()),
		)
		.unwrap();
	renderer.wait(Some(submission)).unwrap();
	renderer.read_pixels(&target).unwrap().rgba.to_vec()
}

fn pixel(pixels: &[u8], width: usize, x: usize, y: usize) -> [u8; 4] {
	pixels[(y * width + x) * 4..(y * width + x + 1) * 4]
		.try_into()
		.unwrap()
}

fn srgb(linear: f32) -> u8 {
	let encoded = if linear <= 0.0031308 {
		linear * 12.92
	} else {
		1.055 * linear.powf(1. / 2.4) - 0.055
	};
	(encoded * 255.).round() as u8
}

#[test]
#[ignore = "requires a GPU; verifies fragment coordinates, clipping and pipeline reuse"]
fn color_fields_normalize_clip_and_cache_programs() {
	let mut renderer = pollster::block_on(Renderer::new(None)).unwrap();
	let snapshot = LayoutSnapshot::default();
	let horizontal = HashMap::new();
	let field = ColorField::builder()
		.paint("return vec4<f32>(x, y, 0.0, 1.0);")
		.finish();
	let rect = Rect {
		x: 2.,
		y: 2.,
		w: 4.,
		h: 4.,
	};
	let independent = ColorField::builder()
		.paint("return vec4<f32>(x, y, 0.0, 1.0);")
		.finish();
	for scale in [1., 1.25, 2.] {
		let view = view(&horizontal, scale);
		let draws = [Draw::Clipped {
			rect: Rect { x: 3., ..rect },
			draws: vec![field.draw(rect)],
		}];
		let pixels = render(&mut renderer, &snapshot, &view, &draws);
		let x = (4. * scale) as usize;
		let y = (3. * scale) as usize;
		let expected = [
			srgb(((x as f32 + 0.5) / scale - rect.x) / rect.w),
			srgb(((y as f32 + 0.5) / scale - rect.y) / rect.h),
			0,
			255,
		];
		assert!(
			pixel(&pixels, view.width as usize, x, y)
				.into_iter()
				.zip(expected)
				.all(|(a, b)| a.abs_diff(b) <= 1)
		);
		assert_eq!(
			pixel(
				&pixels,
				view.width as usize,
				(2. * scale) as usize,
				(2. * scale) as usize
			),
			pixel(&pixels, view.width as usize, 0, 0)
		);
		let bytes = renderer.gpu_bytes();
		for _ in 0..10 {
			render(&mut renderer, &snapshot, &view, &draws);
		}
		assert_eq!(renderer.gpu_bytes(), bytes);
		assert_eq!(renderer.color_field_stats(), (1, 1));
	}
	let view = view(&horizontal, 1.);
	let moved = Rect {
		x: 4.,
		y: 1.,
		w: 6.,
		h: 8.,
	};
	let pixels = render(&mut renderer, &snapshot, &view, &[field.draw(moved)]);
	assert_eq!(pixel(&pixels, 12, 5, 2)[0], srgb(0.25));
	render(&mut renderer, &snapshot, &view, &[]);
	assert_eq!(renderer.color_field_stats(), (1, 1));
	let hidden = ColorField::builder()
		.paint("return rgba(0x00FF00FFu);")
		.finish();
	render(
		&mut renderer,
		&snapshot,
		&view,
		&[hidden.draw(Rect { x: 100., ..rect })],
	);
	assert_eq!(renderer.color_field_stats(), (1, 1));
	render(&mut renderer, &snapshot, &view, &[hidden.draw(rect)]);
	assert_eq!(renderer.color_field_stats(), (2, 2));
	render(&mut renderer, &snapshot, &view, &[independent.draw(rect)]);
	assert_eq!(renderer.color_field_stats(), (2, 2));
	drop((field, hidden));
	render(&mut renderer, &snapshot, &view, &[]);
	assert_eq!(renderer.color_field_stats(), (1, 2));
	render(&mut renderer, &snapshot, &view, &[independent.draw(rect)]);
	assert_eq!(renderer.color_field_stats(), (1, 2));
	drop(independent);
	render(&mut renderer, &snapshot, &view, &[]);
	assert_eq!(renderer.color_field_stats(), (0, 2));
}

#[test]
#[ignore = "requires a GPU; verifies layer composition and order among images and solids"]
fn color_fields_compose_and_preserve_paint_order() {
	let mut renderer = pollster::block_on(Renderer::new(None)).unwrap();
	let mut snapshot = LayoutSnapshot::default();
	let horizontal = HashMap::new();
	let view = view(&horizontal, 1.);
	let rect = Rect {
		x: 2.,
		y: 2.,
		w: 8.,
		h: 8.,
	};
	let field = ColorField::builder()
		.paint("return rgba(0x0000FF80u);")
		.paint("return rgba(0xFF000080u);")
		.map("return vec4<f32>(color.rgb, select(color.a, 0.0, x < 0.25));")
		.finish();
	let composed = render(&mut renderer, &snapshot, &view, &[field.draw(rect)]);
	let native = render(
		&mut renderer,
		&snapshot,
		&view,
		&[Draw::Clipped {
			rect: Rect {
				x: 4.,
				w: 6.,
				..rect
			},
			draws: [0x0000FF80, 0xFF000080]
				.into_iter()
				.map(|c| Draw::Rect(rect, Paint::Color(Color(c))))
				.collect(),
		}],
	);
	assert!(composed.iter().zip(native).all(|(a, b)| a.abs_diff(b) <= 1));
	let bytes = renderer.gpu_bytes();
	snapshot.images.entries.insert(
		"test".into(),
		ImageInfo {
			version: 1,
			size: Some((1, 1)),
			error: None,
		},
	);
	snapshot.images.pixels.insert(
		"test".into(),
		1,
		Arc::new(Pixels {
			width: 1,
			height: 1,
			rgba: Arc::from([0, 255, 0, 255]),
		}),
	);
	let red = ColorField::builder()
		.paint("return rgba(0xFF0000FFu);")
		.finish();
	let blue = ColorField::builder()
		.paint("return rgba(0x0000FFFFu);")
		.finish();
	let image = Draw::Image {
		rect: Rect {
			x: 4.,
			w: 6.,
			..rect
		},
		src: "test".into(),
		version: 1,
		title: String::new(),
	};
	let draws = [
		red.draw(rect),
		image,
		blue.draw(Rect {
			x: 6.,
			w: 4.,
			..rect
		}),
		Draw::Rect(
			Rect {
				x: 8.,
				w: 2.,
				..rect
			},
			Paint::Color(Color(0xEFE0CDFF)),
		),
	];
	let pixels = render(&mut renderer, &snapshot, &view, &draws);
	for (x, expected) in [
		(3, [255, 0, 0, 255]),
		(5, [0, 255, 0, 255]),
		(7, [0, 0, 255, 255]),
		(9, [239, 224, 205, 255]),
	] {
		assert_eq!(pixel(&pixels, 12, x, 3), expected);
	}
	assert_eq!(renderer.gpu_bytes() - bytes, 4);
	assert_eq!(renderer.color_field_stats(), (3, 3));
}
