use super::*;
use crate::app::tab_metrics::TabMetrics;

#[test]
fn cached_tab_end_follows_scroll_theme_and_document_changes() {
	let mut ui = crate::test_support::shaper();
	let mut metrics = TabMetrics::default();
	let mut tabs: Vec<_> = (0..20)
		.map(|i| ReaderTab::new(format!("中文文档{i}.md").into()))
		.collect();
	for count in [20, 2, 1, 0] {
		tabs.truncate(count);
		for size in [1.0, 1.5] {
			let sheet = markview_core::style::Stylesheet::parse(&format!(
				"format_version=2\nversion=1\n[[rule]]\nwhen=['ui','toolbar']\nsize={size}"
			))
			.unwrap();
			ui.set_stylesheet(std::sync::Arc::new(sheet));
			metrics.sync(&mut ui, &tabs);
			for width in [150.0, 500.5, 1200.0] {
				for scroll in [-10.0, 0.0, 25.5, 10000.0] {
					let viewport = Rect {
						x: 10.0,
						y: 4.0,
						w: width,
						h: 36.0,
					};
					let layout =
						TabLayout::new(viewport, &metrics.widths, scroll);
					let end = layout
						.rects
						.last()
						.map_or(viewport.x, |rect| rect.x + rect.w);
					assert!(
						(metrics.end(viewport, scroll) - end).abs() < 0.001
					);
				}
			}
		}
	}
}

#[test]
fn live_tab_style_switch_preserves_widths_scroll_and_hit_targets() {
	let mut ui = crate::test_support::shaper();
	let tabs: Vec<_> = (0..30)
		.map(|i| ReaderTab::new(format!("document{i}.md").into()))
		.collect();
	let mut metrics = TabMetrics::default();
	metrics.sync(&mut ui, &tabs);
	let widths = metrics.widths.clone();
	let strip = TabStrip {
		scroll: 25.0,
		..Default::default()
	};
	let mut bar = TabBar {
		style: TabStyle::default(),
		ui: &mut ui,
		strip: &strip,
		widths: &widths,
		tabs: &tabs,
		active_tab: 1,
		cursor: (0.0, 0.0),
		viewport: crate::app::frame::Layout::new(
			crate::settings::WindowLayout::Windows,
			false,
			500.0,
			300.0,
			false,
			false,
		)
		.tabs,
	};
	let original = bar.layout();
	for (style, height) in
		[(TabStyle::Underline, 32.0), (TabStyle::Connected, 36.0)]
	{
		bar.style = style;
		let layout = bar.layout();
		assert_eq!(layout.scroll, original.scroll);
		for (rect, before) in layout.rects.iter().zip(&original.rects) {
			assert_eq!((rect.x, rect.w, rect.h), (before.x, before.w, height));
		}
		let active = layout.rects[1];
		assert_eq!(layout.hit(active.x + active.w / 2.0, 20.0), Some(1));
		assert_eq!(layout.hit(active.x + active.w - 16.0, 20.0), Some(1));
		let draws = bar.draw_tabs();
		let Draw::Clipped { draws, .. } = &draws[0] else {
			panic!("tab clip missing")
		};
		let underline = draws.iter().any(|draw| {
			matches!(
				draw,
				Draw::Rect(_, Paint::Styled(Condition::Toolbar, C::Accent))
			)
		});
		assert_eq!(underline, style == TabStyle::Underline);
	}
	metrics.sync(bar.ui, &tabs);
	assert_eq!(metrics.widths, widths);
}

#[test]
fn compact_labels_keep_two_whole_graphemes_and_fit_the_measured_space() {
	let mut ui = crate::test_support::shaper();
	ui.appearance = crate::app::tab_metrics::tab_appearance(&ui);
	for name in ["中文文档.md", "e\u{301}日笔记.md", "👩‍💻🙂notes.md", "a.md"]
	{
		let prefix: String = name.graphemes(true).take(2).collect();
		let width = ui.text_width(&prefix, 12.0);
		let fitted = fit_label(&mut ui, name, width);
		assert_eq!(fitted, prefix);
		assert!(ui.text_width(&fitted, 12.0) <= width + 0.001);
	}
}

#[test]
fn measured_tabs_fit_minimum_window_and_styles_invalidate_widths() {
	let mut ui = crate::test_support::shaper();
	let mut metrics = TabMetrics::default();
	let tabs: Vec<_> = (0..30)
		.map(|i| ReaderTab::new(format!("中文文档{i}.md").into()))
		.collect();
	metrics.sync(&mut ui, &tabs);
	let old = metrics.widths.clone();
	let strip = TabStrip::default();
	let mut bar = TabBar {
		style: TabStyle::default(),
		ui: &mut ui,
		strip: &strip,
		widths: &metrics.widths,
		tabs: &tabs,
		active_tab: 0,
		cursor: (0.0, 0.0),
		viewport: crate::app::frame::Layout::new(
			crate::settings::WindowLayout::Windows,
			false,
			500.0,
			300.0,
			false,
			false,
		)
		.tabs,
	};
	let layout = bar.layout();
	assert!(layout.max_scroll > 0.0);
	for (rect, (_, minimum)) in layout.rects.iter().zip(&metrics.widths) {
		assert!((rect.w - minimum).abs() < 0.001);
	}
	let sheet = markview_core::style::Stylesheet::parse(
		"format_version=2\nversion=1\n[[rule]]\nwhen=['ui','toolbar']\nsize = 1.5\n",
	)
	.unwrap();
	ui.set_stylesheet(std::sync::Arc::new(sheet));
	metrics.sync(&mut ui, &tabs);
	assert!(metrics.widths[0].1 > old[0].1);
	metrics.sync(&mut ui, &[]);
	assert!(metrics.widths.is_empty());
}
