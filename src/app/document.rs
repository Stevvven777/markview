use super::tabs;
use crate::watch::FileWatch;
use std::{path::PathBuf, time::Instant};

use super::{App, Event};
use crate::state::Selection;
impl<P: super::SendEvent> App<P> {
	pub(super) fn request(&mut self, follow: bool) {
		self.update_media();
		if let Some(mut request) = self.readers.request(self.options(), follow)
		{
			request.coverage = if follow
				&& self.readers.session.scrolling.offset
					>= (self.readers.session.snapshot.height
						- self.viewport() - 3.)
						.max(0.)
			{
				self.readers.session.scrolling.target = Some(f32::INFINITY);
				f32::INFINITY
			} else {
				self.readers.session.coverage(self.viewport())
			};
			self.error = false;
			self.status_until = None;
			self.status =
				self.preferences.values.lang().status_updating().into();
			self.worker.submit(request);
			self.redraw();
		}
	}
	pub(super) fn observe_document(&mut self) {
		self.watch = self
			.readers
			.session
			.path
			.clone()
			.filter(|_| !self.readers.session.web_loading)
			.map(|path| {
				let proxy = self.proxy.clone();
				let observed = path.clone();
				FileWatch::new(path, move || {
					proxy.send(Event::Changed(observed.clone()));
				})
			});
	}
	pub(super) fn open(&mut self, path: PathBuf) {
		self.open_document(
			path,
			crate::security::Security::local(crate::security::Trust::Trusted),
		);
	}
	pub(super) fn open_document(
		&mut self,
		path: PathBuf,
		security: crate::security::Security,
	) {
		self.open_path(path, false, security);
	}
	pub(super) fn open_pending(
		&mut self,
		path: PathBuf,
		security: crate::security::Security,
	) {
		self.open_path(path, true, security);
	}
	fn open_path(
		&mut self,
		path: PathBuf,
		web_loading: bool,
		security: crate::security::Security,
	) {
		self.restore_session();
		self.interaction.modal = None;
		self.clear_input_focus();
		self.cancel_gestures();
		self.abandon_dm();
		self.tab_strip.cancel_drag();
		self.tab_strip.reveal_active = true;
		let path = if path.is_absolute() {
			path
		} else {
			std::env::current_dir().unwrap_or_default().join(path)
		};
		let path = std::fs::canonicalize(&path).unwrap_or(path);
		if let Some(index) = self.readers.find_origin(&path, &security.origin) {
			self.select_tab(index);
			if self.readers.session.load_error.take().is_some() {
				self.request(false);
			}
			return;
		}
		self.close_search();
		self.worker.cancel();
		self.readers.open(path, Instant::now());
		self.readers.session.security = security;
		self.readers.session.web_loading = web_loading;
		self.readers.session.layout_pending = true;
		self.error = false;
		self.status.clear();
		self.interaction.clear_selection();
		if web_loading {
			self.watch = None;
			self.redraw();
		} else {
			self.observe_document();
			self.request(false);
		}
	}
	pub(super) fn new_page(&mut self) {
		self.restore_session();
		self.interaction.modal = None;
		self.clear_input_focus();
		self.cancel_gestures();
		self.abandon_dm();
		self.tab_strip.cancel_drag();
		self.tab_strip.reveal_active = false;
		self.close_search();
		self.readers.new_page(Instant::now());
		self.interaction.viewer = None;
		self.worker.cancel();
		self.search_worker.release();
		self.watch = None;
		self.interaction.clear_selection();
		self.interaction.outline_open = false;
		self.interaction.show_panel(crate::state::PanelPage::Closed);
		self.error = false;
		self.status.clear();
		self.status_until = None;
		if let Some(window) = &self.window {
			window.set_title("Markview");
		}
		self.refresh_hover();
		self.redraw();
	}
	pub(super) fn select_tab(&mut self, index: usize) {
		self.interaction.modal = None;
		self.clear_input_focus();
		self.cancel_gestures();
		self.abandon_dm();
		self.tab_strip.cancel_drag();
		self.tab_strip.reveal_active = true;
		if !self.readers.select(index, Instant::now()) {
			self.apply_anchor();
			self.redraw();
			return;
		}
		self.observe_document();
		self.interaction.clear_selection();
		self.worker.cancel();
		self.close_search();
		self.search_changed();
		self.error = false;
		self.status.clear();
		self.status_until = None;
		// The worker releases inactive pixels, so image tabs must resume loading.
		if self.readers.session.document.is_none()
			|| self.readers.session.layout_pending
			|| !self.readers.session.snapshot.images.entries.is_empty()
			|| self.readers.session.requested_options.as_ref()
				!= Some(&self.options())
		{
			self.request(false);
		}
		self.apply_anchor();
		self.redraw();
	}
	pub(super) fn close_tab(&mut self, index: usize) {
		self.interaction.modal = None;
		self.clear_input_focus();
		self.cancel_gestures();
		self.abandon_dm();
		self.tab_strip.cancel_drag();
		self.tab_strip.reveal_active = true;
		let Some(closed) = self
			.readers
			.entries()
			.get(index)
			.map(|tab| tab.path.clone())
		else {
			return;
		};
		let origin = self.readers.session_mut(index).security.origin.clone();
		let result = self.readers.close(index, Instant::now());
		// A watched export belongs to the document and origin that chose it.
		if self.watch_export.as_ref().is_some_and(|watch| {
			watch.source == closed && watch.origin == origin
		}) {
			self.watch_export = None;
			self.watch_at = None;
		}
		match result {
			tabs::Closed::Missing => return,
			tabs::Closed::Inactive if !self.readers.entries().is_empty() => {
				self.redraw();
				return;
			}
			tabs::Closed::Active | tabs::Closed::Inactive => {}
		}
		self.watch = None;
		if self.readers.entries().is_empty() {
			// No tab is left, so the worker can drop the document it kept for
			// the closed one instead of holding it until the next open. The
			// export panel has nothing to export either.
			self.worker.release();
			self.search_worker.release();
			self.interaction.show_panel(crate::state::PanelPage::Closed);
			if let Some(window) = &self.window {
				window.set_title("Markview");
			}
		} else {
			self.worker.cancel();
		}
		self.interaction.clear_selection();
		self.close_search();
		self.search_changed();
		self.error = false;
		self.status.clear();
		self.status_until = None;
		if !self.readers.entries().is_empty()
			&& self.readers.session.path.is_some()
		{
			self.observe_document();
			if self.readers.session.document.is_none()
				|| self.readers.session.layout_pending
				|| !self.readers.session.snapshot.images.entries.is_empty()
				|| self.readers.session.requested_options.as_ref()
					!= Some(&self.options())
			{
				self.request(false);
			}
		}
		self.redraw();
	}
}

#[cfg(test)]
mod tests;
