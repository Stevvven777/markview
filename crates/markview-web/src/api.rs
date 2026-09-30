//! The `Markview` handle: state, update loop and frames.
//!
//! Everything here is driven from JavaScript, one call at a time, on the
//! browser's own thread: there is nowhere to block on the GPU and nowhere to
//! lay a document out but here.

use crate::{
	fonts,
	selection::{Pointer, Reading},
};
use markview_core::{
	document::{Document, parse},
	image::ImageSnapshot,
	layout::{
		LayoutEngine, LayoutOptions, LayoutSnapshot, ProgressiveLayout,
		Viewport,
	},
	style::Stylesheet,
};
use markview_render::{FrameStatus, Renderer, SurfaceSource, Theme, View};
use markview_selection::{Host, Modifiers};
use serde::Deserialize;
use std::{
	cell::Cell,
	collections::HashMap,
	sync::{Arc, OnceLock},
	time::Duration,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;
use web_time::Instant;

/// Logical pixels kept clear above and below the reading column.
const INSET: f32 = 10.0;
/// Logical pixels kept clear beside the reading column. The column narrows so
/// that this margin survives, because nothing here can pan sideways to text a
/// too-wide column would clip.
const MARGIN: f32 = 20.0;
/// The largest canvas edge the first surface configure may report.
///
/// The device limit is only knowable after the device exists, so the
/// configure inside `Renderer::new` keeps to the smallest limit Markview ever
/// asks a device for, which is the GL backend's. `create` resizes to the real
/// limit as soon as it can read it.
const INITIAL_LIMIT: u32 =
	wgpu::Limits::downlevel_webgl2_defaults().max_texture_dimension_2d;
/// The largest logical canvas edge the layout viewport accepts.
const MAX_LOGICAL: f32 = 8_192.0;
/// The largest device pixel ratio the viewport accepts.
const MAX_DPR: f32 = 8.0;
/// The longest layout budget one `stepUpdate` call may be asked for.
const MAX_STEP_MS: f64 = 60_000.0;

/// Builds a handle that draws into `canvas`, importing `config_json` when the
/// page passes one.
#[wasm_bindgen]
pub async fn create(
	canvas: HtmlCanvasElement,
	config_json: Option<String>,
) -> Result<Markview, JsValue> {
	console_error_panic_hook::set_once();
	let config = Config::parse(config_json.as_deref())?;
	let dpr = ratio(web_sys::window().map_or(1.0, |w| w.device_pixel_ratio()));
	// The page may not have sized the canvas yet, so start from its CSS box
	// when there is one and let `resize` keep it current afterwards.
	let rect = canvas.get_bounding_client_rect();
	let logical = if rect.width() > 0.0 && rect.height() > 0.0 {
		(logical_edge(rect.width()), logical_edge(rect.height()))
	} else {
		(
			logical_edge(f64::from(canvas.width().max(1))),
			logical_edge(f64::from(canvas.height().max(1))),
		)
	};
	// The device limit is only knowable after the device exists, so the
	// renderer is created from the backing store the page already has — the
	// HTML default for an untouched canvas — and the canvas is sized under
	// the limit only once it can be read.
	let mut renderer = Renderer::new(Some(Box::new(Present {
		canvas: canvas.clone(),
	})))
	.await
	.map_err(|error| fail(format!("{error:#}")))?;
	let limit = renderer.max_texture_dimension_2d();
	let scale = effective_scale(logical, dpr, limit);
	let (width, height) = size_canvas(&canvas, logical, scale, limit);
	renderer.resize(width, height);
	let mut options = config.options();
	options.width = column_width(config.width, logical.0);
	Ok(Markview {
		canvas,
		renderer,
		engine: LayoutEngine::new(),
		options,
		config,
		published: Published::default(),
		pointer: Pointer::default(),
		document: parse(""),
		pending: None,
		cursor: None,
		logical,
		dpr,
		scale,
		scroll: 0.0,
		parse_ms: 0.0,
		layout_ms: 0.0,
		frame_ms: 0.0,
		frames: 0,
		glyphs: 0,
		selection_chars: Cell::new(None),
	})
}

/// One rendering of a Markdown document, driven by the page.
#[wasm_bindgen]
pub struct Markview {
	canvas: HtmlCanvasElement,
	renderer: Renderer,
	engine: LayoutEngine,
	config: Config,
	options: LayoutOptions,
	published: Published,
	pointer: Pointer,
	document: Document,
	/// The document `begin_update` parsed, until its layout completes.
	pending: Option<Pending>,
	/// The last canvas-local point the page reported, in CSS pixels.
	cursor: Option<(f32, f32)>,
	/// The canvas box in CSS pixels, which hit testing and scrolling use.
	logical: (f32, f32),
	/// The device pixel ratio the page asked for.
	dpr: f32,
	/// The scale the canvas is actually drawn at: the requested ratio,
	/// lowered until both logical edges fit the device limit. Sharing it with
	/// the drawn view keeps rendering and hit testing on one viewport.
	scale: f32,
	scroll: f32,
	parse_ms: f64,
	layout_ms: f64,
	frame_ms: f64,
	frames: u64,
	glyphs: u64,
	/// The UTF-16 length of the selected text, or `None` after a change.
	/// `stats_json` runs at least once per frame, while extracting the
	/// selection scans every block and allocates the whole text.
	selection_chars: Cell<Option<usize>>,
}

#[wasm_bindgen]
impl Markview {
	/// Full replace: parses, lays out and publishes.
	#[wasm_bindgen(js_name = setMarkdown)]
	pub fn set_markdown(&mut self, text: String) -> String {
		let started = Instant::now();
		self.document = parse(text);
		self.pending = None;
		self.parse_ms = started.elapsed().as_secs_f64() * 1000.0;
		let laid = Instant::now();
		self.relayout();
		self.layout_ms = laid.elapsed().as_secs_f64() * 1000.0;
		self.clamp_scroll();
		self.stats_json()
	}

	/// Parses `text` and starts a resumable layout.
	#[wasm_bindgen(js_name = beginUpdate)]
	pub fn begin_update(&mut self, text: String) -> String {
		let started = Instant::now();
		let document = parse(text);
		self.parse_ms = started.elapsed().as_secs_f64() * 1000.0;
		self.layout_ms = 0.0;
		let images = ImageSnapshot::default();
		let layout =
			self.engine.begin_layout(&document, &self.options, &images);
		self.pending = Some(Pending {
			parsed: Some(document),
			layout,
		});
		self.stats_json()
	}

	/// Advances the pending layout by at most `budget_ms` of work, publishing
	/// the prefix it reached.
	#[wasm_bindgen(js_name = stepUpdate)]
	pub fn step_update(&mut self, budget_ms: f64) -> String {
		let budget = Duration::from_secs_f64(step_budget(budget_ms));
		self.step(budget);
		self.clamp_scroll();
		self.stats_json()
	}

	/// True while `begin_update` has not finished.
	#[wasm_bindgen(js_name = updatePending)]
	pub fn update_pending(&self) -> bool {
		self.pending.is_some()
	}

	/// Forces the remaining layout to complete synchronously.
	#[wasm_bindgen(js_name = finishUpdate)]
	pub fn finish_update(&mut self) {
		let Some(mut pending) = self.pending.take() else {
			return;
		};
		let laid = Instant::now();
		// The pass is over the parsed text when there is one, and over the
		// accepted document otherwise, which is the case for a reflow.
		let accepted = &self.document;
		let document = pending.parsed.as_ref().unwrap_or(accepted);
		self.engine
			.advance(&mut pending.layout, document, Duration::MAX);
		self.layout_ms += laid.elapsed().as_secs_f64() * 1000.0;
		let source = document.source.clone();
		let pass = pending.layout.pass_id();
		self.published.accept(
			pending.layout.into_snapshot(),
			source,
			Some(pass),
			&mut self.pointer,
		);
		if let Some(parsed) = pending.parsed {
			self.document = parsed;
		}
		self.forget_selection_length();
		self.clamp_scroll();
	}

	/// Renders and presents one frame from the latest published snapshot.
	pub fn frame(&mut self) -> Result<String, JsValue> {
		let (width, height) = self.physical();
		if width == 0 || height == 0 {
			return Ok(self.stats_json());
		}
		let started = Instant::now();
		let view = View {
			selection: self.pointer.selection(),
			revision: self.published.revision,
			width,
			height,
			scale: self.scale,
			scroll: self.visible_scroll(),
			left: self.left(),
			// A small inset keeps the first and last line off the canvas edge.
			top: INSET,
			bottom: INSET,
			theme: self.theme(),
			horizontal: no_horizontal(),
			hovered_link: None,
			hovered_overflow: None,
			held_overflow: None,
		};
		self.renderer.set_pointer(self.cursor);
		let status = self
			.renderer
			.acquire()
			.map_err(|error| fail(format!("{error:#}")))?;
		let FrameStatus::Ready(frame, suboptimal) = status else {
			self.frame_ms = started.elapsed().as_secs_f64() * 1000.0;
			return Ok(self.stats_json());
		};
		let target = frame.texture.create_view(&Default::default());
		let before = self.renderer.raster_stats().rasterized;
		self.renderer
			.render(&self.published.snapshot, &view, &[], &target)
			.map_err(|error| fail(format!("{error:#}")))?;
		self.glyphs += self.renderer.raster_stats().rasterized - before;
		frame.present();
		self.frames += 1;
		if suboptimal {
			self.renderer.resize(width, height);
		}
		self.frame_ms = started.elapsed().as_secs_f64() * 1000.0;
		Ok(self.stats_json())
	}

	/// Sizes the drawing surface from the canvas box and the device ratio, and
	/// reports whether the reading column narrowed, which starts a reflow.
	pub fn resize(
		&mut self,
		css_width: f64,
		css_height: f64,
		dpr: f64,
	) -> bool {
		self.logical = (logical_edge(css_width), logical_edge(css_height));
		self.dpr = ratio(dpr);
		// The page passes its requested ratio on every call, so the scale is
		// derived from it and the box rather than from the last scale, which
		// would shrink the canvas again on every repeat.
		let limit = self.renderer.max_texture_dimension_2d();
		self.scale = effective_scale(self.logical, self.dpr, limit);
		let (width, height) =
			size_canvas(&self.canvas, self.logical, self.scale, limit);
		self.renderer.resize(width, height);
		// A narrower canvas narrows the column, and the lines that were laid
		// out for the wider one no longer fit: lay the document out again.
		let reflowed = self.apply_column();
		if reflowed {
			self.reflow();
		}
		self.clamp_scroll();
		reflowed
	}

	/// Lays the current text out again at the column the canvas now allows.
	///
	/// It runs as a resumable pass, so dragging a window edge never blocks a
	/// frame: each animation frame advances it and presents what is ready.
	fn reflow(&mut self) {
		if let Some(pending) = self.pending.take()
			&& let Some(parsed) = pending.parsed
		{
			// Keep the newest text rather than the last fully accepted one.
			self.document = parsed;
		}
		let images = ImageSnapshot::default();
		let layout =
			self.engine
				.begin_layout(&self.document, &self.options, &images);
		self.pending = Some(Pending {
			parsed: None,
			layout,
		});
	}

	/// Sets the document scroll in logical pixels, clamped to the range. A
	/// non-finite request leaves the scroll where it was.
	#[wasm_bindgen(js_name = setScroll)]
	pub fn set_scroll(&mut self, y: f64) {
		if y.is_finite() {
			let before = self.visible_scroll();
			self.hold_scroll(y as f32);
			// A request the clamp absorbs, or one that a pending pass holds
			// above its temporary height, leaves the offset on screen alone,
			// and with it every reading position under the pointer.
			if self.visible_scroll() == before {
				return;
			}
			// Only a press in flight has a focus to move, and only a known
			// cursor says where to move it.
			if self.pointer.drag().is_none() {
				return;
			}
			let Some((x, y)) = self.cursor else {
				return;
			};
			let reading = Reading {
				snapshot: &self.published.snapshot,
				revision: self.published.revision,
				viewport: self.viewport(),
			};
			// Re-hit-testing can land on the position already selected, and
			// then the cached count still describes the text on screen.
			if self.pointer.drag_to(&reading, x, y) {
				self.forget_selection_length();
			}
		}
	}

	#[wasm_bindgen(js_name = scrollBy)]
	pub fn scroll_by(&mut self, dy: f64) {
		// The request accumulates, not what a still-growing prefix could show.
		self.set_scroll(f64::from(self.scroll) + dy);
	}

	/// The offset on screen, logical px.
	pub fn scroll(&self) -> f64 {
		f64::from(self.visible_scroll())
	}

	#[wasm_bindgen(js_name = maxScroll)]
	pub fn max_scroll(&self) -> f64 {
		f64::from(self.scroll_range())
	}

	/// The laid-out document height in logical pixels.
	#[wasm_bindgen(js_name = contentHeight)]
	pub fn content_height(&self) -> f64 {
		f64::from(self.published.snapshot.height)
	}

	/// Starts a press: a word or block selection on a repeated click.
	#[wasm_bindgen(js_name = pointerDown)]
	pub fn pointer_down(
		&mut self,
		x: f64,
		y: f64,
		shift: bool,
		control: bool,
		alt: bool,
		meta: bool,
	) {
		self.cursor = Some((x as f32, y as f32));
		self.pointer.set_modifiers(Modifiers {
			shift,
			control,
			alt,
			meta,
		});
		let reading = Reading {
			snapshot: &self.published.snapshot,
			revision: self.published.revision,
			viewport: self.viewport(),
		};
		self.pointer.press(&reading, x as f32, y as f32);
		self.forget_selection_length();
	}

	/// Extends the press in flight. A hover that is not dragging, or one that
	/// lands on the same reading position, leaves the selection alone, so its
	/// cached length survives.
	#[wasm_bindgen(js_name = pointerMove)]
	pub fn pointer_move(&mut self, x: f64, y: f64) {
		self.cursor = Some((x as f32, y as f32));
		let reading = Reading {
			snapshot: &self.published.snapshot,
			revision: self.published.revision,
			viewport: self.viewport(),
		};
		if self.pointer.drag_to(&reading, x as f32, y as f32) {
			self.forget_selection_length();
		}
	}

	/// Ends the press in flight.
	#[wasm_bindgen(js_name = pointerUp)]
	pub fn pointer_up(&mut self, x: f64, y: f64) {
		self.pointer.release(x as f32, y as f32);
		self.forget_selection_length();
	}

	#[wasm_bindgen(js_name = selectAll)]
	pub fn select_all(&mut self) {
		self.pointer
			.select_all(&self.published.snapshot, self.published.revision);
		self.forget_selection_length();
	}

	#[wasm_bindgen(js_name = clearSelection)]
	pub fn clear_selection(&mut self) {
		self.pointer.clear();
		self.forget_selection_length();
	}

	/// The reading text of the selection, `""` when empty or stale.
	#[wasm_bindgen(js_name = selectedText)]
	pub fn selected_text(&self) -> String {
		self.pointer
			.selected_text(&self.published.snapshot, self.published.revision)
	}

	/// A `Stats` JSON string for the current state.
	pub fn stats(&self) -> String {
		self.stats_json()
	}

	/// The GPU adapter and backend this handle draws through.
	pub fn adapter(&self) -> String {
		self.renderer.adapter_name.clone()
	}

	/// Applies a new `Config` and lays the document out again.
	#[wasm_bindgen(js_name = setConfig)]
	pub fn set_config(
		&mut self,
		config_json: Option<String>,
	) -> Result<(), JsValue> {
		let config = Config::parse(config_json.as_deref())?;
		self.options = config.options();
		self.config = config;
		self.apply_column();
		if let Some(pending) = self.pending.take()
			&& let Some(parsed) = pending.parsed
		{
			self.document = parsed;
		}
		let laid = Instant::now();
		self.relayout();
		self.layout_ms = laid.elapsed().as_secs_f64() * 1000.0;
		self.clamp_scroll();
		Ok(())
	}
}

impl Markview {
	/// The canvas backing store in physical pixels.
	fn physical(&self) -> (u32, u32) {
		(self.canvas.width(), self.canvas.height())
	}

	/// The layout viewport in logical pixels, mirroring the drawn `View`.
	fn viewport(&self) -> Viewport {
		Viewport {
			width: self.logical.0,
			height: self.logical.1,
			left: self.left(),
			top: INSET,
			bottom: INSET,
			scroll: self.visible_scroll(),
		}
	}

	/// The x of the layout column's left edge inside the canvas.
	fn left(&self) -> f32 {
		((self.logical.0 - self.published.snapshot.width) / 2.0).max(MARGIN)
	}

	/// Narrows the layout column to what the canvas can show, and reports
	/// whether it moved. A column left wider than the canvas clips its lines
	/// off the edge, and nothing here can pan sideways to reach them.
	fn apply_column(&mut self) -> bool {
		let width = column_width(self.config.width, self.logical.0);
		if (self.options.width - width).abs() < f32::EPSILON {
			return false;
		}
		self.options.width = width;
		true
	}

	/// The offset actually on screen. A pass that is still publishing has only
	/// a temporary height, so the requested offset is held above it rather than
	/// clamped away: later prefixes put the reader back where they were.
	fn visible_scroll(&self) -> f32 {
		self.scroll.min(self.scroll_range())
	}

	fn theme(&self) -> Theme {
		match self.config.theme {
			ThemeConfig::Light => Theme::Light,
			ThemeConfig::Dark => Theme::Dark,
		}
	}

	/// How far the document scrolls: its height past the page that is actually
	/// visible, which the two viewport insets shrink. Subtracting the whole
	/// canvas would leave the last `2 * INSET` pixels unreachable.
	fn scroll_range(&self) -> f32 {
		let page = (self.logical.1 - 2.0 * INSET).max(0.0);
		(self.published.snapshot.height - page).max(0.0)
	}

	fn clamp_scroll(&mut self) {
		// A pass that is still publishing reports a height that is about to
		// grow, so clamping against it would throw away where the reader was.
		if self.pending.is_some() {
			return;
		}
		self.scroll = self.scroll.clamp(0.0, self.scroll_range());
	}

	/// Records a scroll request without discarding it, and clamps it right
	/// away unless a pass is pending: a pending pass reports a temporary
	/// height, so the request is only held above zero there, and
	/// `clamp_scroll` clamps it once the pass completes. `visible_scroll`
	/// still reports it clamped to what exists.
	fn hold_scroll(&mut self, y: f32) {
		self.scroll = if self.pending.is_some() {
			y.max(0.0)
		} else {
			y.clamp(0.0, self.scroll_range())
		};
	}

	/// Lays the accepted document out in one uninterrupted pass.
	fn relayout(&mut self) {
		// `accept` rebases the selection onto the new snapshot, so its text
		// may differ even where the reading positions survive.
		self.forget_selection_length();
		let Markview {
			engine,
			options,
			published,
			pointer,
			document,
			..
		} = self;
		let snapshot = engine.layout(document, options);
		// A full layout is not a prefix of anything, so no pass may extend it.
		published.accept(snapshot, document.source.clone(), None, pointer);
	}

	/// Runs the pending layout until the budget runs out, publishing the prefix
	/// it reached. Returns whether the whole document is laid out.
	fn step(&mut self, budget: Duration) -> bool {
		let Some(mut pending) = self.pending.take() else {
			return true;
		};
		let started = Instant::now();
		// The pass is resumed, never restarted, so a step pays only for the
		// blocks it has not reached yet, however long the published prefix has
		// grown. It is over the parsed text when there is one, and over the
		// accepted document otherwise, which is the case for a reflow.
		let accepted = &self.document;
		let document = pending.parsed.as_ref().unwrap_or(accepted);
		let completed =
			self.engine.advance(&mut pending.layout, document, budget);
		self.layout_ms += started.elapsed().as_secs_f64() * 1000.0;
		if completed {
			let source = document.source.clone();
			let pass = pending.layout.pass_id();
			self.published.accept(
				pending.layout.into_snapshot(),
				source,
				Some(pass),
				&mut self.pointer,
			);
			if let Some(parsed) = pending.parsed {
				self.document = parsed;
			}
			self.forget_selection_length();
		} else {
			let prefix = pending.layout.snapshot();
			let pass = pending.layout.pass_id();
			if self.published.continues(&document.source, pass) {
				// The prefix of a pass already on screen only adds blocks, so
				// it is appended rather than copied whole.
				self.published.extend(prefix, &mut self.pointer);
				// `extend` only re-stamps the selection, which reads the same
				// text, so the cached character count still holds here.
			} else if prefix.height >= self.scroll {
				// A different document, or the same one laid out for another
				// column, replaces what is on screen. Until its prefix has
				// grown back past the top of the current view, replacing would
				// show the reader a much shorter document, so the snapshot
				// they are reading stays until it does.
				self.published.accept(
					prefix.clone(),
					document.source.clone(),
					Some(pass),
					&mut self.pointer,
				);
				self.forget_selection_length();
			}
			self.pending = Some(pending);
		}
		completed
	}

	/// The length of the selection as JavaScript counts it:
	/// [`Markview::selected_text`] scans the whole snapshot and allocates the
	/// selected text, so it is extracted at most once between changes.
	fn selection_length(&self) -> usize {
		if let Some(length) = self.selection_chars.get() {
			return length;
		}
		// `selectedText().length` counts UTF-16 code units, so a character
		// outside the basic plane counts as two here as well.
		let length = self.selected_text().encode_utf16().count();
		self.selection_chars.set(Some(length));
		length
	}

	/// Drops the cached count after the selection or the snapshot it was
	/// extracted from changes.
	fn forget_selection_length(&self) {
		self.selection_chars.set(None);
	}

	fn stats_json(&self) -> String {
		serde_json::json!({
			"revision": self.published.revision,
			"blocks": self.published.snapshot.blocks.len(),
			"contentHeight": self.published.snapshot.height,
			"width": self.published.snapshot.width,
			"reused": self.published.snapshot.reused,
			"parseMs": self.parse_ms,
			"layoutMs": self.layout_ms,
			"frameMs": self.frame_ms,
			"frames": self.frames,
			"glyphs": self.glyphs,
			"backend": format!("{:?}", self.renderer.backend),
			"adapter": self.renderer.adapter_name,
			"selectionLength": self.selection_length(),
			"pending": self.pending.is_some(),
		})
		.to_string()
	}
}

/// A layout pass in flight, and the text it is over.
struct Pending {
	/// The document `begin_update` parsed, when the pass is over text newer
	/// than [`Markview::document`] holds. A reflow re-uses the accepted
	/// document instead, which is why this is optional.
	parsed: Option<Document>,
	/// The suspended pass. It holds everything the layout needs between calls,
	/// so a step never re-measures a block an earlier step already laid out.
	layout: ProgressiveLayout,
}

/// The snapshot the canvas last drew, and the revision it was accepted at.
#[derive(Default)]
struct Published {
	snapshot: LayoutSnapshot,
	revision: u64,
	/// The source of the document `snapshot` describes, and the pass that laid
	/// it out. Together they say whether a prefix merely adds blocks to what is
	/// on screen: the same document laid out for a new column width does not.
	source: Option<Arc<str>>,
	pass: Option<u64>,
}
impl Published {
	/// Accepts `next` as a whole new snapshot, moving the selection onto it
	/// while it still reads the same text and clearing it otherwise. `pass` is
	/// the resumable pass that produced it, or `None` for a snapshot no pass
	/// can extend.
	fn accept(
		&mut self,
		next: LayoutSnapshot,
		source: Arc<str>,
		pass: Option<u64>,
		pointer: &mut Pointer,
	) {
		let revision = self.revision.wrapping_add(1);
		let previous = std::mem::replace(&mut self.snapshot, next);
		let selection = pointer.selection().and_then(|selection| {
			previous.rebase_selection(
				&self.snapshot,
				selection,
				self.revision,
				revision,
			)
		});
		pointer.set_selection(selection);
		pointer.rebase_drag(&previous, &self.snapshot, self.revision, revision);
		self.revision = revision;
		self.source = Some(source);
		self.pass = pass;
	}

	/// Appends the blocks `prefix` added since the last publication, so a
	/// budgeted step pays for the blocks it reached rather than for the whole
	/// prefix again. Returns false when `prefix` does not continue what is
	/// published, which leaves the caller to accept it whole.
	fn extend(
		&mut self,
		prefix: &LayoutSnapshot,
		pointer: &mut Pointer,
	) -> bool {
		let published = self.snapshot.blocks.len();
		if prefix.blocks.len() < published {
			return false;
		}
		self.snapshot
			.blocks
			.extend_from_slice(&prefix.blocks[published..]);
		self.snapshot.height = prefix.height;
		self.snapshot.document_box = prefix.document_box.clone();
		self.snapshot.width = prefix.width;
		self.snapshot.reused = prefix.reused;
		self.snapshot.degraded = prefix.degraded;
		self.snapshot.math_errors = prefix.math_errors;
		self.snapshot.images = prefix.images.clone();
		let revision = self.revision.wrapping_add(1);
		// Only blocks were added, so every reading position still means what it
		// meant and the gesture in flight keeps extending from the same base.
		pointer.retag(self.revision, revision);
		self.revision = revision;
		true
	}

	/// Whether `pass` over `source` merely adds blocks to what is published.
	fn continues(&self, source: &Arc<str>, pass: u64) -> bool {
		self.pass == Some(pass)
			&& self
				.source
				.as_ref()
				.is_some_and(|published| Arc::ptr_eq(published, source))
	}
}

/// The display handle wgpu wants before it accepts a canvas surface.
///
/// A browser borrows nothing for it, but the shared handle enum is neither
/// `Send` nor `Sync`, which wgpu requires, so this names the web variant alone.
#[derive(Debug)]
struct WebDisplay;
impl wgpu::rwh::HasDisplayHandle for WebDisplay {
	fn display_handle(
		&self,
	) -> Result<wgpu::rwh::DisplayHandle<'_>, wgpu::rwh::HandleError> {
		Ok(wgpu::rwh::DisplayHandle::web())
	}
}

/// The canvas the renderer draws into, and its only surface source.
struct Present {
	canvas: HtmlCanvasElement,
}
impl SurfaceSource for Present {
	fn instance_descriptor(&self) -> wgpu::InstanceDescriptor {
		// wgpu insists the instance carries a display handle before it accepts
		// a canvas surface, and asking for WebGL2 alone keeps the reported
		// backend the one the demo asserts.
		wgpu::InstanceDescriptor {
			backends: wgpu::Backends::GL,
			..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(
				WebDisplay,
			))
		}
	}
	fn create_surface(
		&self,
		instance: &wgpu::Instance,
	) -> anyhow::Result<wgpu::Surface<'static>> {
		let target: wgpu::SurfaceTarget<'static> =
			wgpu::SurfaceTarget::Canvas(self.canvas.clone());
		Ok(instance.create_surface(target)?)
	}
	fn size(&self) -> (u32, u32) {
		// The first configure happens before a device exists to ask, so a
		// backing store the page already left oversized is reported at
		// `INITIAL_LIMIT` until `create` reads the real limit.
		(
			self.canvas.width().clamp(1, INITIAL_LIMIT),
			self.canvas.height().clamp(1, INITIAL_LIMIT),
		)
	}
}

/// The JavaScript-injected configuration. Every field is optional and unknown
/// keys are ignored.
#[derive(Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Config {
	width: f32,
	font_size: f32,
	theme: ThemeConfig,
	justify: bool,
	hyphenate: bool,
	paragraph_indent: f32,
	greedy: bool,
	hide_front_matter: bool,
	front_matter_label: String,
}
impl Default for Config {
	fn default() -> Self {
		Self {
			width: 760.0,
			font_size: 18.0,
			theme: ThemeConfig::Light,
			justify: true,
			hyphenate: true,
			paragraph_indent: 0.0,
			greedy: false,
			hide_front_matter: false,
			front_matter_label: "Metadata".into(),
		}
	}
}
impl Config {
	fn parse(json: Option<&str>) -> Result<Self, JsValue> {
		match json.map(str::trim) {
			None | Some("") => Ok(Self::default()),
			Some(text) => serde_json::from_str(text)
				.map_err(|error| fail(format!("invalid config: {error}"))),
		}
	}

	fn options(&self) -> LayoutOptions {
		LayoutOptions {
			width: finite(self.width, 760.0).clamp(1.0, MAX_LOGICAL),
			font_size: finite(self.font_size, 18.0).clamp(1.0, 200.0),
			justify: self.justify,
			hyphenate: self.hyphenate,
			paragraph_indent: self.paragraph_indent,
			greedy: self.greedy,
			hide_front_matter: self.hide_front_matter,
			front_matter_label: self.front_matter_label.clone(),
			stylesheet: Stylesheet::bundled(self.theme == ThemeConfig::Dark),
			fonts: fonts::config(),
			..LayoutOptions::default()
		}
	}
}

#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum ThemeConfig {
	Light,
	Dark,
}

/// The reading column `desired` logical pixels wide, narrowed until [`MARGIN`]
/// survives on both sides of a canvas `canvas` logical pixels wide. Text that
/// overflows sideways cannot be reached, so the column always fits.
fn column_width(desired: f32, canvas: f32) -> f32 {
	desired.max(1.0).min((canvas - 2.0 * MARGIN).max(1.0))
}

/// A finite layout budget in seconds, bounded so `Duration` always accepts
/// it. A non-finite request becomes a zero budget, which still completes one
/// prefix per call rather than laying the document out in one go.
fn step_budget(millis: f64) -> f64 {
	if millis.is_finite() {
		millis.clamp(0.0, MAX_STEP_MS) / 1000.0
	} else {
		0.0
	}
}

/// A finite, non-negative logical edge, capped where layout stops making
/// sense. A non-finite request becomes zero, which draws nothing.
fn logical_edge(value: f64) -> f32 {
	if value.is_finite() {
		(value as f32).clamp(0.0, MAX_LOGICAL)
	} else {
		0.0
	}
}

/// A finite device pixel ratio. Zero, negative and non-finite all become one,
/// because a ratio scales the canvas and is never a way to ask for none.
fn ratio(value: f64) -> f32 {
	if value.is_finite() {
		(value as f32).clamp(0.1, MAX_DPR)
	} else {
		1.0
	}
}

/// A finite value, or `fallback` when it is not a number.
fn finite(value: f32, fallback: f32) -> f32 {
	if value.is_finite() { value } else { fallback }
}

/// The scale the canvas and the drawn view share: the ratio the page asked
/// for, lowered until neither logical edge asks for more device pixels than
/// the device accepts. A zero edge asks for none, so it caps nothing.
fn effective_scale(logical: (f32, f32), dpr: f32, limit: u32) -> f32 {
	let limit = limit as f32;
	let fit = |edge: f32| if edge > 0.0 { limit / edge } else { dpr };
	dpr.min(fit(logical.0)).min(fit(logical.1))
}

/// Sizes the canvas backing store from its logical box and the scale the
/// device accepts.
fn size_canvas(
	canvas: &HtmlCanvasElement,
	logical: (f32, f32),
	scale: f32,
	limit: u32,
) -> (u32, u32) {
	let width = device_pixels(logical.0, scale, limit);
	let height = device_pixels(logical.1, scale, limit);
	canvas.set_width(width);
	canvas.set_height(height);
	(width, height)
}

/// One edge in device pixels, capped at what the device accepts.
fn device_pixels(logical: f32, scale: f32, limit: u32) -> u32 {
	let edge = (logical * scale).round();
	if edge.is_finite() {
		(edge.max(0.0) as u32).min(limit)
	} else {
		0
	}
}

/// The demo never pans a wide block sideways, so every lookup misses.
fn no_horizontal() -> &'static HashMap<(usize, usize), f32> {
	static EMPTY: OnceLock<HashMap<(usize, usize), f32>> = OnceLock::new();
	EMPTY.get_or_init(HashMap::new)
}

/// Reports an error to JavaScript as a string.
fn fail(error: impl std::fmt::Display) -> JsValue {
	JsValue::from_str(&error.to_string())
}
