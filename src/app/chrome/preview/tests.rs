use super::*;
use crate::app::chrome::preview::Card;

fn card(title: &str, body: bool) -> Card {
	Card {
		title: title_lines(&mut crate::test_support::shaper(), title),
		detail: "/home/reader/notes.md".into(),
		body,
	}
}

fn anchor(x: f32, w: f32) -> Rect {
	Rect {
		x,
		y: 4.0,
		w,
		h: 36.0,
	}
}

#[test]
fn the_card_hangs_below_its_anchor_and_stays_inside_the_window() {
	let card = card("notes.md", true);
	for (x, expected_x) in [
		(0.0, 8.0),
		(200.0, 200.0 + 50.0 - WIDTH / 2.0),
		(700.0, 800.0 - WIDTH - 8.0),
	] {
		let layout = super::layout(&card, anchor(x, 100.0), (800.0, 600.0));
		assert_eq!(layout.rect.y, 4.0 + 36.0 + TOP_GAP);
		assert_eq!(layout.rect.w, WIDTH);
		assert!((layout.rect.x - expected_x).abs() < 0.001, "{x}");
		assert!(layout.rect.x >= 8.0);
		assert!(layout.rect.x + layout.rect.w <= 800.0 - 8.0);
	}
	// A window too short for the whole card keeps its top on screen.
	let short = super::layout(&card, anchor(200.0, 100.0), (800.0, 120.0));
	assert!(short.rect.y >= 0.0);
}

#[test]
fn the_title_wraps_to_at_most_two_lines_and_ends_ellipsized() {
	let mut ui = crate::test_support::shaper();
	assert_eq!(super::title_lines(&mut ui, "notes.md"), ["notes.md"]);

	let long =
		"a-very-long-document-name-that-cannot-fit-on-a-single-tab-label.md"
			.repeat(2);
	let lines = super::title_lines(&mut ui, &long);
	assert_eq!(lines.len(), 2);
	assert!(lines[1].ends_with('…'), "{lines:?}");
	assert!(lines[0].len() + lines[1].len() < long.len());
	assert!(ui.text_width(&lines[1], TITLE_SIZE) <= CONTENT_WIDTH + 0.001);

	let chinese =
		"这是一个名字特别长的中文文档用来检查标题最多显示两行".repeat(2);
	let lines = super::title_lines(&mut ui, &chinese);
	assert_eq!(lines.len(), 2);
	for line in &lines {
		assert!(ui.text_width(line, TITLE_SIZE) <= CONTENT_WIDTH + 0.001);
	}
}

#[test]
fn a_card_without_a_body_reserves_no_preview_rectangle() {
	let with = super::layout(
		&card("notes.md", true),
		anchor(200.0, 100.0),
		(800.0, 600.0),
	);
	let without = super::layout(
		&card("notes.md", false),
		anchor(200.0, 100.0),
		(800.0, 600.0),
	);
	assert!(with.body.is_some());
	assert!(without.body.is_none());
	assert_eq!(
		with.rect.h - without.rect.h,
		CONTENT_WIDTH * BODY_RATIO + BODY_GAP
	);
}

#[test]
fn preview_cards_use_the_theme_radius_without_a_body_frame() {
	let mut ui = crate::test_support::shaper();
	let card = card("notes.md", true);
	let layout = super::layout(&card, anchor(200.0, 100.0), (800.0, 600.0));
	for theme in markview_layout::style::Stylesheet::READER_THEMES {
		let mut stylesheet =
			(*markview_layout::style::Stylesheet::builtin()).clone();
		stylesheet.merge(
			&markview_layout::style::Stylesheet::named_rules(theme).unwrap(),
		);
		for radius in [0.0, 12.0] {
			if radius > 0.0 {
				stylesheet.merge(
					&markview_layout::style::Stylesheet::parse(
						"format_version=2\nversion=1\n[[rule]]\nwhen=['ui','preview']\nradius=12",
					)
					.unwrap(),
				);
			}
			ui.set_stylesheet(std::sync::Arc::new(stylesheet.clone()));
			let draws = draw(&mut ui, &card, &layout);
			assert!(
				matches!(
					draws.first(),
					Some(Draw::Box {
						rect,
						condition: Condition::Preview,
						radius: actual,
						border: 1.0,
						..
					}) if *rect == layout.rect && *actual == radius
				),
				"{theme}: {radius}"
			);
			assert!(
				draws[1..].iter().all(|draw| matches!(draw, Draw::Glyph(_)))
			);
			assert!(draws.len() > 1);
		}
	}
}
