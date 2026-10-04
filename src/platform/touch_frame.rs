//! Supply the touch serials that `winit`'s Wayland window drags omit.
#![allow(unsafe_code)]
use anyhow::Result;
use std::{collections::VecDeque, sync::Arc};
use wayland_client::{
	Connection, Dispatch, EventQueue, Proxy, QueueHandle, WEnum,
	backend::{Backend, ObjectId},
	protocol::{wl_registry, wl_seat, wl_surface, wl_touch},
};
use wayland_protocols::xdg::shell::client::xdg_toplevel::{self, XdgToplevel};
use winit::{
	event::{Touch, TouchPhase},
	platform::wayland::WindowExtWayland,
	raw_window_handle::{
		HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle,
	},
	window::{ResizeDirection, Window},
};

pub(crate) struct TouchFrame {
	queue: EventQueue<State>,
	state: State,
	connection: Connection,
	// Keep the foreign display and surface alive until our queue is dropped.
	_window: Arc<Window>,
}

#[derive(Default)]
struct State {
	seats: Vec<(u32, wl_seat::WlSeat, Option<wl_touch::WlTouch>)>,
	down: VecDeque<(u64, u32, wl_seat::WlSeat, ObjectId)>,
}

impl TouchFrame {
	pub fn new(window: Arc<Window>) -> Result<Option<Self>> {
		let RawDisplayHandle::Wayland(display) =
			window.display_handle()?.as_raw()
		else {
			return Ok(None);
		};
		// SAFETY: `window` retains the live Wayland connection. The guest
		// backend owns only its queue and never disconnects this display.
		let backend = unsafe {
			Backend::from_foreign_display(display.display.as_ptr().cast())
		};
		let connection = Connection::from_backend(backend);
		let mut queue = connection.new_event_queue();
		connection.display().get_registry(&queue.handle(), ());
		let mut state = State::default();
		queue.roundtrip(&mut state)?;
		queue.roundtrip(&mut state)?;
		connection.flush()?;
		Ok(Some(Self {
			queue,
			state,
			connection,
			_window: window,
		}))
	}

	pub fn touch(
		&mut self,
		touch: Touch,
		drag: Option<Option<ResizeDirection>>,
	) {
		self.dispatch();
		if touch.phase != TouchPhase::Started {
			return;
		}
		let RawWindowHandle::Wayland(handle) =
			self._window.window_handle().unwrap().as_raw()
		else {
			unreachable!();
		};
		let toplevel = self._window.xdg_toplevel().unwrap();
		// SAFETY: `winit` supplies its live `xdg_toplevel`, kept alive by
		// `_window`. Importing the proxy preserves its original event handler.
		let id = unsafe {
			ObjectId::from_ptr(
				XdgToplevel::interface(),
				toplevel.as_ptr().cast(),
			)
		}
		.unwrap();
		let toplevel = XdgToplevel::from_id(&self.connection, id).unwrap();
		self.state
			.begin(touch.id, handle.surface.as_ptr(), &toplevel, drag);
		if let Err(error) = self.connection.flush() {
			log::debug!("Cannot flush window touch drag: {error}");
		}
	}

	pub fn dispatch(&mut self) {
		if let Err(error) = self.queue.dispatch_pending(&mut self.state) {
			log::debug!("Cannot dispatch window touch input: {error}");
		}
		if let Err(error) = self.connection.flush() {
			log::debug!("Cannot flush window touch input: {error}");
		}
	}
}

impl State {
	fn begin(
		&mut self,
		id: u64,
		surface: *mut std::ffi::c_void,
		toplevel: &XdgToplevel,
		drag: Option<Option<ResizeDirection>>,
	) {
		let Some(index) =
			self.down.iter().position(|(contact, ..)| *contact == id)
		else {
			return;
		};
		let (_, serial, seat, origin) = self.down.remove(index).unwrap();
		if origin.as_ptr().cast::<std::ffi::c_void>() != surface {
			return;
		}
		let Some(direction) = drag else { return };
		if let Some(direction) = direction {
			toplevel.resize(&seat, serial, resize_edge(direction));
		} else {
			toplevel._move(&seat, serial);
		}
	}
}

fn resize_edge(direction: ResizeDirection) -> xdg_toplevel::ResizeEdge {
	use xdg_toplevel::ResizeEdge;
	match direction {
		ResizeDirection::North => ResizeEdge::Top,
		ResizeDirection::South => ResizeEdge::Bottom,
		ResizeDirection::West => ResizeEdge::Left,
		ResizeDirection::East => ResizeEdge::Right,
		ResizeDirection::NorthWest => ResizeEdge::TopLeft,
		ResizeDirection::NorthEast => ResizeEdge::TopRight,
		ResizeDirection::SouthWest => ResizeEdge::BottomLeft,
		ResizeDirection::SouthEast => ResizeEdge::BottomRight,
	}
}

impl Dispatch<wl_registry::WlRegistry, ()> for State {
	fn event(
		state: &mut Self,
		registry: &wl_registry::WlRegistry,
		event: wl_registry::Event,
		_: &(),
		_: &Connection,
		qh: &QueueHandle<Self>,
	) {
		match event {
			wl_registry::Event::Global {
				name,
				interface,
				version,
			} if interface == "wl_seat" => {
				let seat = registry.bind(name, version.min(7), qh, ());
				state.seats.push((name, seat, None));
			}
			wl_registry::Event::GlobalRemove { name } => {
				if let Some(index) =
					state.seats.iter().position(|(global, ..)| *global == name)
				{
					let (_, seat, touch) = state.seats.remove(index);
					state.down.retain(|(_, _, owner, _)| *owner != seat);
					release_touch(touch);
					if seat.version() >= 5 {
						seat.release();
					}
				}
			}
			_ => {}
		}
	}
}

impl Dispatch<wl_seat::WlSeat, ()> for State {
	fn event(
		state: &mut Self,
		seat: &wl_seat::WlSeat,
		event: wl_seat::Event,
		_: &(),
		_: &Connection,
		qh: &QueueHandle<Self>,
	) {
		if let wl_seat::Event::Capabilities {
			capabilities: WEnum::Value(capabilities),
		} = event
		{
			let Some((_, _, touch)) =
				state.seats.iter_mut().find(|(_, owner, _)| owner == seat)
			else {
				return;
			};
			if capabilities.contains(wl_seat::Capability::Touch) {
				if touch.is_none() {
					*touch = Some(seat.get_touch(qh, seat.clone()));
				}
			} else {
				state.down.retain(|(_, _, owner, _)| owner != seat);
				release_touch(touch.take());
			}
		}
	}
}

impl Dispatch<wl_touch::WlTouch, wl_seat::WlSeat> for State {
	fn event(
		state: &mut Self,
		_: &wl_touch::WlTouch,
		event: wl_touch::Event,
		seat: &wl_seat::WlSeat,
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
		if let wl_touch::Event::Down {
			serial,
			surface,
			id,
			..
		} = event
		{
			// Keep batched contacts until their matching `winit` starts arrive.
			state.down.push_back((
				id as u64,
				serial,
				seat.clone(),
				surface.id(),
			));
		}
	}
}

fn release_touch(touch: Option<wl_touch::WlTouch>) {
	if let Some(touch) = touch
		&& touch.version() >= 3
	{
		touch.release();
	}
}

impl Drop for TouchFrame {
	fn drop(&mut self) {
		for (_, seat, touch) in self.state.seats.drain(..) {
			release_touch(touch);
			if seat.version() >= 5 {
				seat.release();
			}
		}
	}
}

wayland_client::delegate_noop!(State: ignore wl_surface::WlSurface);

#[cfg(test)]
mod tests;
