//! Guidance and link controls for the empty reader.
use super::{
	Button, Command, Draw, Lang, Paint, Rect, TextShaper, components, controls,
	ui_appearance,
};
use crate::state::{InteractionState, TextField};
use markview_core::style::{ColorField as C, Condition};

struct Layout {
	x: f32,
	y: f32,
	detail: Vec<String>,
	hint: Vec<String>,
	buttons: Vec<Button>,
}

fn layout(ui: &mut TextShaper, width: f32, height: f32, lang: Lang) -> Layout {
	ui.appearance = ui_appearance(ui);
	let w = (width - 48.0).clamp(0.0, 440.0);
	let x = (width - w) / 2.0;
	let detail = controls::wrap(
		ui,
		if cfg!(target_os = "android") {
			lang.empty_open_detail_android()
		} else {
			lang.empty_open_detail()
		},
		13.0,
		w,
	);
	let hint = controls::wrap(
		ui,
		&lang.empty_paste_hint(if cfg!(target_os = "macos") {
			"⌘V"
		} else {
			"Ctrl+V"
		}),
		13.0,
		w,
	);
	let total = 32.0
		+ detail.len() as f32 * 20.0
		+ 48.0 + hint.len() as f32 * 20.0
		+ 44.0;
	let y = (height * 0.4)
		.min(height - super::BOTTOM - total - 16.0)
		.max(super::TOP + 40.0);
	let mut open = components::button(
		lang.empty_open_file(),
		Command::Open,
		Rect {
			x,
			y: y + 32.0 + detail.len() as f32 * 20.0,
			w: 128.0_f32.min(w),
			h: 32.0,
		},
	);
	open.kind = components::ButtonKind::Primary;
	let input_y = open.rect.y + 48.0 + hint.len() as f32 * 20.0;
	let button_w = (ui.text_width(lang.empty_open_url(), 13.0) + 24.0).min(w);
	let buttons = vec![
		open,
		components::button(
			lang.empty_url_placeholder(),
			Command::FocusInput(TextField::Url),
			Rect {
				x,
				y: input_y,
				w: (w - button_w - 8.0).max(0.0),
				h: 36.0,
			},
		),
		components::button(
			lang.empty_open_url(),
			Command::OpenUrl,
			Rect {
				x: x + w - button_w,
				y: input_y,
				w: button_w,
				h: 36.0,
			},
		),
	];
	Layout {
		x,
		y,
		detail,
		hint,
		buttons,
	}
}

pub(super) fn buttons(
	ui: &mut TextShaper,
	width: f32,
	height: f32,
	lang: Lang,
) -> Vec<Button> {
	layout(ui, width, height, lang).buttons
}

pub(super) fn draw(
	ui: &mut TextShaper,
	interaction: &InteractionState,
	width: f32,
	height: f32,
	lang: Lang,
) -> Vec<Draw> {
	let l = layout(ui, width, height, lang);
	ui.appearance.weight = 700;
	let title = ui.fit(lang.empty_open_title(), 26.0, width - l.x * 2.0);
	let mut out = ui.label(
		&title,
		26.0,
		l.x,
		l.y,
		Paint::Styled(Condition::Ui, C::Color),
	);
	ui.appearance = ui_appearance(ui);
	for (lines, start) in [
		(&l.detail, l.y + 32.0),
		(&l.hint, l.buttons[0].rect.y + 64.0),
	] {
		for (i, line) in lines.iter().enumerate() {
			out.extend(ui.label(
				line,
				13.0,
				l.x,
				start + i as f32 * 20.0,
				Paint::Styled(Condition::Ui, C::Muted),
			));
		}
	}
	for button in l.buttons {
		if !matches!(button.action, Command::FocusInput(_)) {
			out.extend(components::draw_button(ui, interaction, &button, true));
		}
	}
	vec![Draw::Clipped {
		rect: Rect {
			x: 0.0,
			y: super::TOP,
			w: width,
			h: (height - super::TOP - super::BOTTOM).max(0.0),
		},
		draws: out,
	}]
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::render::{Renderer, Theme, View};

	#[test]
	#[ignore = "requires a GPU; writes artifacts/empty-page/*.png"]
	fn empty_page_frames() -> anyhow::Result<()> {
		let output = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
			.join("artifacts/empty-page");
		std::fs::create_dir_all(&output)?;
		let mut renderer = pollster::block_on(Renderer::new(None))?;
		for dark in [false, true] {
			let sheet = markview_core::style::Stylesheet::bundled(dark);
			renderer.set_stylesheet(sheet.clone());
			let mut ui = TextShaper::with_fonts(Default::default());
			ui.set_stylesheet(sheet);
			for lang in [Lang::En, Lang::ZhHans, Lang::ZhHant, Lang::Ja] {
				for (width, height) in [(800.0, 600.0), (360.0, 480.0)] {
					let interaction = InteractionState::default();
					let mut overlay =
						draw(&mut ui, &interaction, width, height, lang);
					let buttons = buttons(&mut ui, width, height, lang);
					for b in &buttons {
						assert!(
							b.rect.y + b.rect.h
								<= height - super::super::BOTTOM
						);
					}
					let mut input =
						markview_core::text_input::TextInput::default();
					overlay.extend(input.draw(
						&mut ui,
						buttons[1].rect,
						false,
						false,
						lang.empty_url_placeholder(),
					));
					let horizontal = std::collections::HashMap::new();
					let view = View {
						width: width as u32,
						height: height as u32,
						scale: 1.0,
						left: 0.0,
						top: 0.0,
						bottom: 0.0,
						scroll: 0.0,
						theme: if dark { Theme::Dark } else { Theme::Light },
						horizontal: &horizontal,
						selection: None,
						revision: 0,
						hovered_link: None,
						hovered_overflow: None,
						held_overflow: None,
					};
					let target = renderer.offscreen(view.width, view.height);
					let submission = renderer.render(
						&Default::default(),
						&view,
						&overlay,
						&target.create_view(&Default::default()),
					)?;
					renderer.wait(Some(submission))?;
					renderer.save_png(
						&target,
						&output.join(format!("{lang:?}-{dark}-{width}.png")),
					)?;
				}
			}
		}
		Ok(())
	}
}
