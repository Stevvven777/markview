use markview_core::{
	image::ColorField,
	scene::{Draw, LayoutSnapshot, Paint, Rect},
	style::Color,
};
use markview_render::{Renderer, Theme, View};
use std::num::NonZeroU32;

#[test]
#[ignore = "requires a GPU; verifies generated texture reuse, alpha and clipping"]
fn color_fields_reuse_gpu_pixels_and_invalidate_by_version() {
	let mut renderer = pollster::block_on(Renderer::new(None)).unwrap();
	let base_bytes = renderer.gpu_bytes();
	let size = [NonZeroU32::new(4).unwrap(); 2];
	let field = ColorField::rasterize(size, |x, y| {
		Color(if y > 0.5 {
			0x864C2400
		} else if x > 0.5 {
			0x0000FFFF
		} else {
			0xFF000080
		})
	});
	let mut snapshot = LayoutSnapshot::default();
	let rect = Rect {
		x: 2.,
		y: 2.,
		w: 4.,
		h: 4.,
	};
	let clipped = |draw| Draw::Clipped {
		rect: Rect { x: 3., ..rect },
		draws: vec![draw],
	};
	let draw = clipped(field.draw(rect, "ui:test", 1, &mut snapshot.images));
	let horizontal = Default::default();
	let view = View {
		width: 10,
		height: 10,
		scale: 1.,
		scroll: 0.,
		left: 0.,
		top: 0.,
		bottom: 0.,
		theme: Theme::Light,
		horizontal: &horizontal,
		selection: None,
		revision: 0,
		hovered_link: None,
		hovered_overflow: None,
		held_overflow: None,
	};
	let target = renderer.offscreen(view.width, view.height);
	let mut paint = |snapshot: &LayoutSnapshot, draw: Draw| {
		let submission = renderer
			.render(
				snapshot,
				&view,
				&[draw],
				&target.create_view(&Default::default()),
			)
			.unwrap();
		renderer.wait(Some(submission)).unwrap();
		let pixels = renderer.read_pixels(&target).unwrap().rgba;
		(pixels, renderer.gpu_bytes())
	};
	let (first, bytes) = paint(&snapshot, draw.clone());
	assert_eq!(bytes - base_bytes, 4 * 4 * 4);
	let pixel = |pixels: &[u8], x: usize, y: usize| -> [u8; 4] {
		pixels[(y * 10 + x) * 4..(y * 10 + x + 1) * 4]
			.try_into()
			.unwrap()
	};
	assert_eq!(pixel(&first, 2, 2), pixel(&first, 0, 0));
	assert_eq!(pixel(&first, 5, 2), [0, 0, 255, 255]);
	assert_eq!(pixel(&first, 4, 5), pixel(&first, 0, 0));
	let red = pixel(&first, 3, 2);
	assert!(red[0] > red[1] && red[1] > 0 && red[1] < 240);
	snapshot.images.pixels.replace(Default::default());
	let (cached, bytes) = paint(&snapshot, draw);
	assert_eq!(cached, first);
	assert_eq!(bytes - base_bytes, 64);
	assert!(
		!snapshot.images.pixels.demand(snapshot.images.generation)["ui:test"]
			.needs_pixels
	);
	let changed = ColorField::rasterize(size, |x, _| {
		Color(if x > 0.5 { 0x00FF00FF } else { 0x864C24FF })
	});
	let draw = clipped(changed.draw(rect, "ui:test", 2, &mut snapshot.images));
	let (updated, bytes) = paint(&snapshot, draw);
	assert_eq!(pixel(&updated, 5, 2), [0, 255, 0, 255]);
	assert_eq!(bytes - base_bytes, 64);
	let solid = ColorField::rasterize(size, |_, _| Color(0xEFE0CDFF));
	let draw = clipped(solid.draw(rect, "ui:test", 3, &mut snapshot.images));
	let (updated, bytes) = paint(&snapshot, draw);
	assert_eq!(pixel(&updated, 5, 2), [239, 224, 205, 255]);
	assert_eq!(bytes, base_bytes);
	let layers = [Color(0x0000FF80), Color(0xFF000080)];
	let flattened = ColorField::builder(size)
		.paint(|_, _| layers[0])
		.paint(|_, _| layers[1])
		.finish();
	let draw =
		clipped(flattened.draw(rect, "ui:test", 4, &mut snapshot.images));
	let (flattened_pixels, bytes) = paint(&snapshot, draw);
	assert_eq!(bytes, base_bytes);
	let (native_layers, _) = paint(
		&snapshot,
		Draw::Clipped {
			rect: Rect { x: 3., ..rect },
			draws: layers
				.into_iter()
				.map(|color| Draw::Rect(rect, Paint::Color(color)))
				.collect(),
		},
	);
	assert!(
		flattened_pixels
			.iter()
			.zip(native_layers)
			.all(|(a, b)| a.abs_diff(b) <= 1)
	);
}
