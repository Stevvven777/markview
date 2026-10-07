use markview_core::{
	image::{ColorField, ImageSnapshot},
	scene::{Draw, Paint, Rect},
	style::Color,
};
use std::{num::NonZeroU32, sync::Arc};

#[test]
fn normalized_fields_share_pixels_and_elide_solid_resources() {
	let size = [NonZeroU32::new(2).unwrap(); 2];
	let mut samples = Vec::new();
	let field = ColorField::rasterize(size, |x, y| {
		samples.push((x, y));
		Color(if x < y { 0xC37C5480 } else { 0xF0EAD6FF })
	});
	assert_eq!(
		samples,
		[(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
	);
	let ColorField::Raster(pixels) = &field else {
		panic!("expected a raster field")
	};
	assert_eq!((pixels.width, pixels.height), (2, 2));
	assert_eq!(&pixels.rgba[8..12], &[195, 124, 84, 128]);
	let mut images = ImageSnapshot::default();
	let rect = Rect {
		x: 10.,
		y: 20.,
		w: 180.,
		h: 32.,
	};
	let mut draw = field.draw(rect, "ui:tab", 1, &mut images);
	draw.translate(7., 9.);
	assert!(matches!(
		draw,
		Draw::Image {
			rect: Rect { x: 17., y: 29., .. },
			..
		}
	));
	field.clone().draw(rect, "ui:tab-copy", 1, &mut images);
	assert!(Arc::ptr_eq(
		&images.pixels.get("ui:tab", 1).unwrap(),
		&images.pixels.get("ui:tab-copy", 1).unwrap(),
	));
	assert_eq!(samples.len(), 4);
	let old = images.clone();
	images.pixels.insert("ui:tab".into(), 99, pixels.clone());
	field.draw(rect, "ui:tab", 2, &mut images);
	assert!(images.pixels.get("ui:tab", 1).is_none());
	assert!(images.pixels.get("ui:tab", 2).is_some());
	assert!(!old.decoded().contains_key("ui:tab"));
	for color in [Color(0xEFE0CDFF), Color(0x864C2480)] {
		let field = ColorField::rasterize(size, |_, _| color);
		assert!(
			matches!(field.draw(rect, "ui:tab", 2, &mut images), Draw::Rect(_, Paint::Color(c)) if c == color)
		);
		assert!(!images.entries.contains_key("ui:tab"));
		assert!(images.pixels.get("ui:tab", 1).is_none());
		assert!(images.pixels.get("ui:tab", 2).is_none());
		assert!(images.pixels.get("ui:tab", 99).is_some());
		assert!(images.pixels.get("ui:tab-copy", 1).is_some());
	}
	let transparent = ColorField::rasterize(size, |x, _| {
		Color(if x < 0.5 { 0xFF000000 } else { 0x00FF0000 })
	});
	assert!(matches!(transparent, ColorField::Solid(Color(0))));
}

#[test]
fn layers_compose_in_linear_light_and_masks_transform_the_result() {
	let size = [NonZeroU32::new(2).unwrap(); 2];
	let mut samples = Vec::new();
	let field = ColorField::builder(size)
		.paint(|_, _| Color(0x000000FF))
		.paint(|x, y| {
			samples.push((x, y));
			Color(0xFFFFFF80)
		})
		.paint(|_, _| Color(0xFF000000))
		.map(|x, _, color| {
			assert_eq!(color, Color(0xBCBCBCFF));
			Color((color.0 & 0xFFFFFF00) | if x < 0.5 { 128 } else { 255 })
		})
		.finish();
	assert_eq!(
		samples,
		[(0.25, 0.25), (0.75, 0.25), (0.25, 0.75), (0.75, 0.75)]
	);
	let ColorField::Raster(pixels) = field else {
		panic!("expected masked raster")
	};
	assert_eq!(&pixels.rgba[..8], &[188, 188, 188, 128, 188, 188, 188, 255]);

	let translucent = ColorField::builder(size)
		.paint(|_, _| Color(0x0000FF80))
		.paint(|_, _| Color(0xFF000080))
		.finish();
	assert!(matches!(translucent, ColorField::Solid(Color(0xD5009CC0))));
	let replaced = ColorField::builder(size)
		.paint(|x, _| Color(if x < 0.5 { 0xFF000080 } else { 0x00FF0080 }))
		.paint(|_, _| Color(0xEFE0CDFF))
		.finish();
	assert!(matches!(replaced, ColorField::Solid(Color(0xEFE0CDFF))));
	let hidden = ColorField::builder(size)
		.paint(|_, _| Color(0xEFE0CDFF))
		.map(|_, _, color| Color(color.0 & 0xFFFFFF00))
		.finish();
	assert!(matches!(hidden, ColorField::Solid(Color(0))));
}
