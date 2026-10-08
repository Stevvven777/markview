use markview_core::scene::{ColorField, Draw, Rect};
use std::sync::Arc;

#[test]
fn field_draws_share_programs_and_translate_without_rebuilding() {
	let field = ColorField::builder()
		.paint("return rgba(0xEFE0CDFFu);")
		.finish();
	let rect = Rect {
		x: 10.,
		y: 20.,
		w: 180.,
		h: 32.,
	};
	let first = field.draw(rect);
	let mut second = field.clone().draw(rect);
	second.translate(7., 9.);
	let Draw::ColorField { shader: a, .. } = first else {
		panic!("expected field")
	};
	let Draw::ColorField { rect, shader: b } = second else {
		panic!("expected field")
	};
	assert_eq!((rect.x, rect.y, rect.w, rect.h), (17., 29., 180., 32.));
	assert!(Arc::ptr_eq(&a, &b));
}
