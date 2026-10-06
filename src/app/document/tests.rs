use super::*;
use crate::{
	app::SendEvent,
	cli::{LaunchOptions, Mode},
};
use std::{fs, sync::mpsc, time::Duration};

#[derive(Clone)]
struct Proxy(mpsc::Sender<Event>);
impl SendEvent for Proxy {
	fn try_send(&self, event: Event) -> bool {
		self.0.send(event).is_ok()
	}
}

struct Harness {
	app: App<Proxy>,
	events: mpsc::Receiver<Event>,
	dir: tempfile::TempDir,
}
impl Harness {
	fn new() -> Self {
		let dir = tempfile::tempdir().unwrap();
		for (name, color) in [
			("first.png", [255, 0, 255, 255]),
			("second.png", [0, 255, 0, 255]),
		] {
			image::RgbaImage::from_pixel(512, 512, image::Rgba(color))
				.save(dir.path().join(name))
				.unwrap();
		}
		fs::write(
			dir.path().join("diagram.svg"),
			br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="40" height="30" fill="#00ffff"/></svg>"##,
		)
		.unwrap();
		fs::write(
			dir.path().join("README.md"),
			format!(
				"![first](first.png)\n\n<img src=\"diagram.svg\" width=\"80\">{}",
				"\n\nReading paragraph.".repeat(50)
			),
		)
		.unwrap();
		fs::write(dir.path().join("README.zh-cn.md"), "![second](second.png)")
			.unwrap();
		let (tx, events) = mpsc::channel();
		let mut app = App::new(
			LaunchOptions {
				mode: Mode::Smoke,
				offline: true,
				width: 400,
				height: 300,
				options: crate::test_support::options(),
				..Default::default()
			},
			Proxy(tx),
		);
		app.preferences.values = Default::default();
		Self { app, events, dir }
	}

	fn open(&mut self, name: &str, count: usize) {
		self.app.open(self.dir.path().join(name));
		self.wait_images(count);
	}

	fn wait_images(&mut self, count: usize) {
		let deadline = Instant::now() + Duration::from_secs(10);
		while self.app.readers.session.layout_pending
			|| self.app.readers.session.snapshot.images.decoded().len() != count
		{
			let event = self
				.events
				.recv_timeout(
					deadline.saturating_duration_since(Instant::now()),
				)
				.expect("active tab images did not load");
			if let Event::Ready(update) = event
				&& update.version == self.app.readers.session.version
				&& Some(&update.path) == self.app.readers.session.path.as_ref()
				&& let Some(result) = update.result
			{
				let reader = result.unwrap();
				let viewport = self.app.viewport();
				if !self.app.readers.session.can_display(&reader, viewport) {
					continue;
				}
				self.app.readers.session.accept(
					reader,
					viewport,
					update.counts,
				);
				self.app.readers.session.displayed_version = update.version;
			}
		}
	}
}

struct StubLoop;
impl crate::app::window::Loop for StubLoop {
	fn exit(&self) {}
}

#[test]
fn web_tabs_open_immediately_and_complete_without_stealing_focus() {
	let mut h = Harness::new();
	h.app.args.offline = false;
	let permit = h
		.app
		.services
		.handle
		.transfers
		.clone()
		.try_acquire_many_owned(4)
		.unwrap();
	let url = "https://example.org/".to_string();
	h.app.open_web_page(url.clone());
	assert_eq!(h.app.readers.entries().len(), 1);
	assert!(h.app.readers.session.web_loading);
	assert!(h.app.readers.session.layout_pending);
	assert!(h.app.readers.session.document.is_none());
	let pending_path = h.app.readers.session.path.clone().unwrap();
	h.app.open_web_page(url.clone());
	assert_eq!(h.app.readers.entries().len(), 1);
	h.open("README.md", 2);
	let active_path = h.app.readers.session.path.clone();
	let markdown = "# Web article\n\nNative article text.\n";
	h.app
		.web_page_loaded(url.clone(), pending_path, Ok(markdown.into()));
	assert_eq!(h.app.readers.session.path, active_path);
	assert_eq!(h.app.readers.active(), 1);
	h.app.open_web_page(url);
	assert_eq!(h.app.readers.active(), 0);
	h.wait_images(0);
	let loaded_path = h.app.readers.session.path.clone().unwrap();
	assert!(loaded_path.starts_with(h.app.paste_dir.path()));
	assert!(
		loaded_path
			.file_name()
			.unwrap()
			.to_string_lossy()
			.contains("Web article")
	);
	assert_eq!(fs::read_to_string(loaded_path).unwrap(), markdown);
	assert!(!h.app.readers.session.web_loading);
	assert!(!h.app.readers.session.snapshot.blocks.is_empty());
	drop(h);
	drop(permit);
}

#[test]
fn failed_tabs_keep_their_errors_and_closed_web_tabs_ignore_completions() {
	let mut h = Harness::new();
	h.app.preferences.values.lang = Some(crate::lang::Lang::ZhHans);
	let url = "https://example.org/".to_string();
	h.app.open_web_page(url.clone());
	assert_eq!(h.app.readers.entries().len(), 1);
	let first_path = h.app.readers.session.path.clone().unwrap();
	let error = h.app.readers.session.load_error.clone().unwrap();
	assert!(error.contains("离线"));
	assert!(!h.app.readers.session.layout_pending);
	let version = h.app.readers.session.version;
	h.app.request(false);
	assert_eq!(h.app.readers.session.version, version);
	assert_eq!(h.app.readers.session.load_error.as_ref(), Some(&error));
	h.open("README.md", 2);
	h.app.select_tab(0);
	assert_eq!(h.app.readers.session.load_error.as_ref(), Some(&error));
	assert!(!h.app.readers.session.layout_pending);
	h.app.close_tab(0);
	h.app.web_page_loaded(
		url.clone(),
		first_path.clone(),
		Ok("# Closed article".into()),
	);
	assert_eq!(h.app.readers.entries().len(), 1);
	h.app.open_web_page(url.clone());
	let second_path = h.app.readers.session.path.clone().unwrap();
	assert_ne!(first_path, second_path);
	h.app.web_page_loaded(
		url.clone(),
		first_path,
		Ok("# Stale article".into()),
	);
	assert_eq!(h.app.readers.session.path.as_ref(), Some(&second_path));
	assert!(h.app.readers.session.load_error.is_some());

	h.app.args.offline = false;
	let permit = h
		.app
		.services
		.handle
		.transfers
		.clone()
		.try_acquire_many_owned(4)
		.unwrap();
	h.app.open_web_page(url.clone());
	assert_eq!(h.app.readers.entries().len(), 2);
	assert!(h.app.readers.session.web_loading);
	assert!(h.app.readers.session.load_error.is_none());
	h.app.web_page_loaded(
		url,
		second_path,
		Err(anyhow::anyhow!("download failed")),
	);
	assert!(
		h.app
			.readers
			.session
			.load_error
			.as_deref()
			.unwrap()
			.contains("download failed")
	);

	h.app.open(h.dir.path().join("missing.md"));
	assert_eq!(h.app.readers.entries().len(), 3);
	assert!(h.app.readers.session.layout_pending);
	let deadline = Instant::now() + Duration::from_secs(10);
	while h.app.readers.session.load_error.is_none() {
		let event = h
			.events
			.recv_timeout(deadline.saturating_duration_since(Instant::now()))
			.unwrap();
		h.app.handle_user_event(&StubLoop, event);
	}
	let failed_path = h.app.readers.session.path.clone();
	let error = h.app.readers.session.load_error.clone();
	assert!(!h.app.readers.session.layout_pending);
	h.app.select_tab(0);
	h.app.select_tab(2);
	assert_eq!(h.app.readers.session.path, failed_path);
	assert_eq!(h.app.readers.session.load_error, error);
	assert!(!h.app.readers.session.layout_pending);
	drop(h);
	drop(permit);
}

#[test]
fn returning_to_cached_tabs_reloads_images_after_switching_or_closing() {
	for close in [false, true] {
		let mut h = Harness::new();
		h.open("README.md", 2);
		let old_pixels = h.app.readers.session.snapshot.images.clone();
		let revision = h.app.readers.session.content_version;
		h.app.readers.session.scrolling.offset = 300.;
		h.app.readers.session.load_all_images = true;
		h.open("README.zh-cn.md", 1);
		assert!(
			old_pixels.decoded().is_empty(),
			"inactive pixels were retained"
		);
		if close {
			h.app.close_tab(1);
			h.wait_images(2);
		} else {
			h.open("README.md", 2);
		}
		let session = &h.app.readers.session;
		assert_eq!(session.content_version, revision);
		assert_eq!(session.scrolling.offset, 300.);
		assert!(session.load_all_images);
		let pixels = session.snapshot.images.decoded();
		assert_eq!(&pixels["first.png"].rgba[..4], &[255, 0, 255, 255]);
		assert_eq!(&pixels["diagram.svg"].rgba[..4], &[0, 255, 255, 255]);
	}
}

#[test]
#[ignore = "requires a GPU"]
fn gpu_restored_tabs_draw_images_after_switching_or_closing()
-> anyhow::Result<()> {
	use crate::render::{Renderer, Theme, View};
	let mut renderer = pollster::block_on(Renderer::new(None))?;
	let target = renderer.offscreen(400, 300);
	let horizontal = Default::default();
	let view = View {
		width: 400,
		height: 300,
		scale: 1.,
		left: 0.,
		top: 0.,
		bottom: 0.,
		scroll: 0.,
		theme: Theme::Light,
		horizontal: &horizontal,
		selection: None,
		revision: 1,
		hovered_link: None,
		hovered_overflow: None,
		held_overflow: None,
	};
	for close in [false, true] {
		let mut h = Harness::new();
		for step in 0..3 {
			match step {
				0 => h.open("README.md", 2),
				1 => h.open("README.zh-cn.md", 1),
				_ if close => {
					h.app.close_tab(1);
					h.wait_images(2);
				}
				_ => h.open("README.md", 2),
			}
			let submission = renderer.render(
				&h.app.readers.session.snapshot,
				&view,
				&[],
				&target.create_view(&Default::default()),
			)?;
			renderer.wait(Some(submission))?;
			let output = h.dir.path().join("frame.png");
			renderer.save_png(&target, &output)?;
			let want = if step == 1 {
				[0, 255, 0]
			} else {
				[255, 0, 255]
			};
			let count = image::open(output)?
				.to_rgb8()
				.pixels()
				.filter(|pixel| {
					(0..3).all(|i| pixel.0[i].abs_diff(want[i]) <= 6)
				})
				.count();
			assert!(
				count > 800,
				"image missing after step {step}, close={close}"
			);
		}
	}
	Ok(())
}
