//! `winit` owns window styles and client sizing; this hook adds native hit tests.
#![allow(unsafe_code)]
use crate::{
	app::frame::{Caption, Layout},
	settings::WindowLayout,
};
use anyhow::{Result, anyhow};
use std::{cell::Cell, ptr};
use windows_sys::Win32::{
	Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
	Graphics::Gdi::ScreenToClient,
	UI::{
		HiDpi::GetDpiForWindow,
		Input::KeyboardAndMouse::{
			ReleaseCapture, SetCapture, TME_LEAVE, TME_NONCLIENT,
			TRACKMOUSEEVENT, TrackMouseEvent,
		},
		Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
		WindowsAndMessaging::*,
	},
};
use winit::{
	raw_window_handle::{HasWindowHandle, RawWindowHandle},
	window::{ResizeDirection, Window},
};

const SUBCLASS: usize = 0x4d56;

struct State {
	style: WindowLayout,
	tab_end: Cell<Option<f32>>,
	fullscreen: Cell<bool>,
	hover: Cell<bool>,
	pressed: Cell<bool>,
	destroyed: Cell<bool>,
	redraw: Box<dyn Fn()>,
}

/// The stable allocation outlives the hook and drops before the `Window`.
pub(crate) struct NativeFrame {
	hwnd: HWND,
	state: Box<State>,
}
impl NativeFrame {
	pub fn new(
		window: &Window,
		style: WindowLayout,
		redraw: impl Fn() + 'static,
	) -> Result<Self> {
		let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw()
		else {
			unreachable!();
		};
		let hwnd = handle.hwnd.get() as HWND;
		let state = Box::new(State {
			style,
			tab_end: Cell::new(None),
			fullscreen: Cell::new(false),
			hover: Cell::new(false),
			pressed: Cell::new(false),
			destroyed: Cell::new(false),
			redraw: Box::new(redraw),
		});
		let data = ptr::from_ref(state.as_ref()) as usize;
		// SAFETY: This is the window's event-loop thread. `state` stays at this
		// address until `Drop` removes the hook, before the window is dropped.
		if unsafe { SetWindowSubclass(hwnd, Some(subclass), SUBCLASS, data) }
			== 0
		{
			return Err(anyhow!("Cannot install window caption hit testing"));
		}
		Ok(Self { hwnd, state })
	}
	pub fn set_fullscreen(&self, fullscreen: bool) {
		self.state.fullscreen.set(fullscreen);
	}
	pub fn set_tab_end(&self, end: f32) {
		self.state.tab_end.set(Some(end));
	}
	pub fn feedback(&self) -> (Option<Caption>, Option<Caption>) {
		(
			self.state.hover.get().then_some(Caption::Expand),
			self.state.pressed.get().then_some(Caption::Expand),
		)
	}
	pub fn cancel(&self) {
		if self.state.pressed.get() {
			self.state.feedback(false, false);
			// SAFETY: Only our outstanding maximize press owns capture.
			unsafe {
				ReleaseCapture();
			}
		}
	}
}
impl Drop for NativeFrame {
	fn drop(&mut self) {
		if !self.state.destroyed.get() {
			// SAFETY: The guard and window live on the same thread. Removing
			// the subclass prevents further access to its allocation.
			unsafe {
				RemoveWindowSubclass(self.hwnd, Some(subclass), SUBCLASS);
			}
		}
	}
}
impl State {
	fn layout(&self, hwnd: HWND) -> Layout {
		let mut rect = RECT::default();
		// SAFETY: `hwnd` belongs to the callback; `rect` is writable. DPI
		// and maximization queries do not retain pointers or send callbacks.
		let (scale, maximized) = unsafe {
			GetClientRect(hwnd, &mut rect);
			(
				GetDpiForWindow(hwnd).max(96) as f32 / 96.0,
				IsZoomed(hwnd) != 0,
			)
		};
		let layout = Layout::new(
			self.style,
			false,
			(rect.right - rect.left) as f32 / scale,
			(rect.bottom - rect.top) as f32 / scale,
			self.fullscreen.get(),
			maximized,
		);
		self.tab_end
			.get()
			.map_or(layout, |end| layout.with_tab_end(end))
	}
	fn point(&self, hwnd: HWND, mut point: POINT) -> (f32, f32) {
		// SAFETY: `point` is writable and `hwnd` is alive during its callback.
		let scale = unsafe {
			ScreenToClient(hwnd, &mut point);
			GetDpiForWindow(hwnd).max(96) as f32 / 96.0
		};
		(point.x as f32 / scale, point.y as f32 / scale)
	}
	fn over_expand(&self, hwnd: HWND) -> bool {
		let mut point = POINT::default();
		// SAFETY: `point` is writable; the query retains no pointer.
		unsafe {
			GetCursorPos(&mut point);
		}
		let (x, y) = self.point(hwnd, point);
		self.layout(hwnd).caption_at(x, y) == Some(Caption::Expand)
	}
	fn feedback(&self, hover: bool, pressed: bool) {
		if self.hover.replace(hover) != hover || self.pressed.get() != pressed {
			self.pressed.set(pressed);
			(self.redraw)();
		}
	}
}

// SAFETY: Windows calls this only on the installing thread. The guard keeps
// `data` alive until this hook is removed; no reference to `App` crosses it.
unsafe extern "system" fn subclass(
	hwnd: HWND,
	message: u32,
	wparam: WPARAM,
	lparam: LPARAM,
	_: usize,
	data: usize,
) -> LRESULT {
	// SAFETY: `data` is the stable `State` registered by `NativeFrame::new`.
	let state = unsafe { &*(data as *const State) };
	match message {
		WM_NCHITTEST => {
			let point = POINT {
				x: (lparam as u16 as i16) as i32,
				y: ((lparam >> 16) as u16 as i16) as i32,
			};
			let (x, y) = state.point(hwnd, point);
			let layout = state.layout(hwnd);
			if let Some(edge) = layout.resize_at(x, y) {
				return match edge {
					ResizeDirection::North => HTTOP,
					ResizeDirection::South => HTBOTTOM,
					ResizeDirection::West => HTLEFT,
					ResizeDirection::East => HTRIGHT,
					ResizeDirection::NorthWest => HTTOPLEFT,
					ResizeDirection::NorthEast => HTTOPRIGHT,
					ResizeDirection::SouthWest => HTBOTTOMLEFT,
					ResizeDirection::SouthEast => HTBOTTOMRIGHT,
				} as LRESULT;
			}
			return if layout.caption_at(x, y) == Some(Caption::Expand) {
				HTMAXBUTTON
			} else if layout.draggable(x, y) {
				HTCAPTION
			} else {
				HTCLIENT
			} as LRESULT;
		}
		WM_NCMOUSEMOVE => {
			state.feedback(wparam == HTMAXBUTTON as usize, state.pressed.get());
			let mut tracking = TRACKMOUSEEVENT {
				cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
				dwFlags: TME_LEAVE | TME_NONCLIENT,
				hwndTrack: hwnd,
				dwHoverTime: 0,
			};
			// SAFETY: Windows reads this initialized structure during the call.
			unsafe {
				TrackMouseEvent(&mut tracking);
			}
			// Forwarding preserves the Windows 11 maximize-hover Snap menu.
		}
		WM_NCLBUTTONDOWN if wparam == HTMAXBUTTON as usize => {
			state.feedback(true, true);
			// SAFETY: The callback owns an alive window on this thread.
			unsafe {
				SetCapture(hwnd);
			}
			return 0;
		}
		WM_MOUSEMOVE if state.pressed.get() => {
			state.feedback(state.over_expand(hwnd), true);
			return 0;
		}
		WM_LBUTTONUP if state.pressed.get() => {
			let over = state.over_expand(hwnd);
			state.feedback(over, false);
			// SAFETY: Capture is ours, and posting a scalar system command
			// retains no Rust pointers. Native code owns the maximize action.
			unsafe {
				ReleaseCapture();
				if over {
					PostMessageW(
						hwnd,
						WM_SYSCOMMAND,
						if IsZoomed(hwnd) != 0 {
							SC_RESTORE
						} else {
							SC_MAXIMIZE
						} as usize,
						0,
					);
				}
			}
			return 0;
		}
		WM_NCMOUSELEAVE => state.feedback(false, state.pressed.get()),
		WM_CANCELMODE => {
			let pressed = state.pressed.get();
			state.feedback(false, false);
			if pressed {
				// SAFETY: Only our outstanding caption press owns capture.
				unsafe {
					ReleaseCapture();
				}
			}
		}
		WM_CAPTURECHANGED if lparam != hwnd as LPARAM => {
			state.feedback(false, false)
		}
		WM_NCDESTROY => state.destroyed.set(true),
		_ => {}
	}
	// SAFETY: Forward the original callback arguments to the next hook;
	// `winit` continues to own sizing, activation and all unrelated input.
	unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

#[cfg(test)]
mod tests {
	use super::*;
	use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
	use winit::{
		dpi::LogicalSize, event_loop::EventLoop,
		platform::windows::EventLoopBuilderExtWindows,
	};

	#[test]
	#[ignore = "requires a Windows desktop"]
	#[allow(deprecated)]
	fn native_caption_hit_testing_tracks_empty_space_and_tab_changes() {
		let event_loop =
			EventLoop::builder().with_any_thread(true).build().unwrap();
		let window = event_loop
			.create_window(
				Window::default_attributes()
					.with_visible(false)
					.with_decorations(false)
					.with_inner_size(LogicalSize::new(800, 500)),
			)
			.unwrap();
		let native =
			NativeFrame::new(&window, WindowLayout::Windows, || {}).unwrap();
		let hit = |x: f32, y: f32| {
			// SAFETY: The hidden window and its hook are alive on this thread;
			// coordinate conversion and hit testing retain no Rust pointers.
			unsafe {
				let scale = GetDpiForWindow(native.hwnd).max(96) as f32 / 96.0;
				let mut point = POINT {
					x: (x * scale) as i32,
					y: (y * scale) as i32,
				};
				ClientToScreen(native.hwnd, &mut point);
				let coordinates = ((point.y as u16 as usize) << 16
					| point.x as u16 as usize) as LPARAM;
				SendMessageW(native.hwnd, WM_NCHITTEST, 0, coordinates) as u32
			}
		};
		let layout = native.state.layout(native.hwnd);
		native.set_tab_end(layout.tabs.x);
		assert_eq!(hit(200.0, 20.0), HTCAPTION);
		native.set_tab_end(180.0);
		assert_eq!(hit(80.0, 20.0), HTCLIENT);
		assert_eq!(hit(200.0, 20.0), HTCAPTION);
		assert_eq!(hit(layout.toolbar_x + 16.0, 20.0), HTCLIENT);
		let (_, expand) = layout.captions()[1];
		assert_eq!(hit(expand.x + expand.w / 2.0, 20.0), HTMAXBUTTON);
		assert_eq!(hit(1.0, 1.0), HTTOPLEFT);
		native.set_tab_end(layout.toolbar_x + 100.0);
		assert_eq!(hit(200.0, 20.0), HTCLIENT);
		assert_eq!(hit(layout.drag.x + 10.0, 20.0), HTCAPTION);
		native.set_fullscreen(true);
		assert_eq!(hit(layout.drag.x + 10.0, 20.0), HTCLIENT);
		assert_eq!(hit(1.0, 1.0), HTCLIENT);
	}
}
