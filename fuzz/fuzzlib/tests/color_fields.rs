use markview_core::scene::{
	BlockLayout, ColorField, Draw, LayoutSnapshot, PlacedBlock, Rect,
};
use mvfuzz::oracle;
use std::sync::Arc;

fn snapshot(draw: Draw) -> LayoutSnapshot {
	LayoutSnapshot {
		blocks: vec![PlacedBlock {
			id: 0,
			source: 0..0,
			y: 0.,
			layout: Arc::new(BlockLayout {
				draws: vec![draw],
				..Default::default()
			}),
		}],
		..Default::default()
	}
}

#[test]
fn color_field_oracles_track_geometry_and_shader_contents() {
	let field = || {
		ColorField::builder()
			.paint("return rgba(0xEFE0CDFFu);")
			.finish()
	};
	let rect = Rect {
		x: 10.,
		y: 20.,
		w: 180.,
		h: 32.,
	};
	let original = snapshot(field().draw(rect));
	oracle::assert_snapshot_finite(&original);
	assert_eq!(
		oracle::layout(&original),
		oracle::layout(&snapshot(field().draw(rect)))
	);
	let changed = ColorField::builder()
		.paint("return rgba(0xC37C54FFu);")
		.finish();
	assert_ne!(
		oracle::layout(&original),
		oracle::layout(&snapshot(changed.draw(rect)))
	);
	let mut moved = field().draw(rect);
	moved.translate(1., 2.);
	assert_ne!(oracle::layout(&original), oracle::layout(&snapshot(moved)));
}

#[test]
#[should_panic(expected = "color field.w is not finite")]
fn color_field_oracle_rejects_nonfinite_geometry() {
	let field = ColorField::builder().finish();
	let invalid = snapshot(field.draw(Rect {
		x: 0.,
		y: 0.,
		w: f32::NAN,
		h: 1.,
	}));
	oracle::assert_snapshot_finite(&invalid);
}
