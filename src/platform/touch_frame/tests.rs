use super::*;
use std::{os::unix::net::UnixStream, sync::mpsc, thread, time::Duration};
use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_protocols::xdg::shell::server::xdg_toplevel as server_toplevel;
use wayland_server::{
	Client, DataInit, Dispatch as ServerDispatch, Display, DisplayHandle,
	GlobalDispatch, New,
	protocol::{
		wl_seat as server_seat, wl_surface as server_surface,
		wl_touch as server_touch,
	},
};

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for State {
	fn event(
		_: &mut Self,
		_: &wl_registry::WlRegistry,
		_: wl_registry::Event,
		_: &GlobalListContents,
		_: &Connection,
		_: &QueueHandle<Self>,
	) {
	}
}
wayland_client::delegate_noop!(State: ignore XdgToplevel);

struct Server {
	seat: Option<server_seat::WlSeat>,
	touch: Option<server_touch::WlTouch>,
	surface: Option<server_surface::WlSurface>,
	actions: mpsc::Sender<(u32, Option<server_toplevel::ResizeEdge>)>,
}

impl GlobalDispatch<server_seat::WlSeat, ()> for Server {
	fn bind(
		state: &mut Self,
		_: &DisplayHandle,
		_: &Client,
		resource: New<server_seat::WlSeat>,
		_: &(),
		data: &mut DataInit<'_, Self>,
	) {
		let seat = data.init(resource, ());
		seat.capabilities(server_seat::Capability::Touch);
		state.seat = Some(seat);
	}
}

impl ServerDispatch<server_seat::WlSeat, ()> for Server {
	fn request(
		state: &mut Self,
		_: &Client,
		_: &server_seat::WlSeat,
		request: server_seat::Request,
		_: &(),
		_: &DisplayHandle,
		data: &mut DataInit<'_, Self>,
	) {
		if let server_seat::Request::GetTouch { id } = request {
			state.touch = Some(data.init(id, ()));
		}
	}
}

// Expose the target objects directly so the test needs no rendering shell.
impl GlobalDispatch<server_surface::WlSurface, ()> for Server {
	fn bind(
		state: &mut Self,
		_: &DisplayHandle,
		_: &Client,
		resource: New<server_surface::WlSurface>,
		_: &(),
		data: &mut DataInit<'_, Self>,
	) {
		state.surface = Some(data.init(resource, ()));
	}
}

impl GlobalDispatch<server_toplevel::XdgToplevel, ()> for Server {
	fn bind(
		_: &mut Self,
		_: &DisplayHandle,
		_: &Client,
		resource: New<server_toplevel::XdgToplevel>,
		_: &(),
		data: &mut DataInit<'_, Self>,
	) {
		data.init(resource, ());
	}
}

impl ServerDispatch<server_toplevel::XdgToplevel, ()> for Server {
	fn request(
		state: &mut Self,
		_: &Client,
		_: &server_toplevel::XdgToplevel,
		request: server_toplevel::Request,
		_: &(),
		_: &DisplayHandle,
		_: &mut DataInit<'_, Self>,
	) {
		let (seat, serial, direction) = match request {
			server_toplevel::Request::Move { seat, serial } => {
				(seat, serial, None)
			}
			server_toplevel::Request::Resize {
				seat,
				serial,
				edges: WEnum::Value(edges),
			} => (seat, serial, Some(edges)),
			_ => return,
		};
		assert_eq!(seat, *state.seat.as_ref().unwrap());
		state.actions.send((serial, direction)).unwrap();
	}
}

impl ServerDispatch<server_surface::WlSurface, ()> for Server {
	fn request(
		_: &mut Self,
		_: &Client,
		_: &server_surface::WlSurface,
		_: server_surface::Request,
		_: &(),
		_: &DisplayHandle,
		_: &mut DataInit<'_, Self>,
	) {
	}
}

impl ServerDispatch<server_touch::WlTouch, ()> for Server {
	fn request(
		_: &mut Self,
		_: &Client,
		_: &server_touch::WlTouch,
		_: server_touch::Request,
		_: &(),
		_: &DisplayHandle,
		_: &mut DataInit<'_, Self>,
	) {
	}
}

#[test]
fn batched_touch_starts_use_their_own_serial_for_native_move_and_resize() {
	let (client, server) = UnixStream::pair().unwrap();
	let (commands, receive) = mpsc::channel();
	let (sent, actions) = mpsc::channel();
	let (ready, injected) = mpsc::channel();
	let host = thread::spawn(move || {
		let mut display = Display::<Server>::new().unwrap();
		let mut handle = display.handle();
		handle.insert_client(server, Arc::new(())).unwrap();
		handle.create_global::<Server, server_seat::WlSeat, _>(7, ());
		handle.create_global::<Server, server_surface::WlSurface, _>(1, ());
		handle.create_global::<Server, server_toplevel::XdgToplevel, _>(1, ());
		let mut state = Server {
			seat: None,
			touch: None,
			surface: None,
			actions: sent,
		};
		loop {
			display.dispatch_clients(&mut state).unwrap();
			match receive.try_recv() {
				Ok(true) => {
					let touch = state.touch.as_ref().unwrap();
					let surface = state.surface.as_ref().unwrap();
					for (id, serial) in [(1, 101), (2, 202), (3, 303)] {
						touch.down(serial, 0, surface, id, 20.0, 20.0);
					}
					touch.frame();
					display.flush_clients().unwrap();
					ready.send(()).unwrap();
				}
				Ok(false) | Err(mpsc::TryRecvError::Disconnected) => break,
				Err(mpsc::TryRecvError::Empty) => {}
			}
			display.flush_clients().unwrap();
			thread::sleep(Duration::from_millis(1));
		}
	});
	let connection = Connection::from_socket(client).unwrap();
	let (globals, mut queue) =
		registry_queue_init::<State>(&connection).unwrap();
	let qh = queue.handle();
	let surface = globals
		.bind::<wl_surface::WlSurface, _, _>(&qh, 1..=1, ())
		.unwrap();
	let toplevel = globals.bind::<XdgToplevel, _, _>(&qh, 1..=1, ()).unwrap();
	queue.roundtrip(&mut State::default()).unwrap();
	// SAFETY: `connection` owns the display and outlives the guest backend.
	let backend = unsafe {
		Backend::from_foreign_display(connection.backend().display_ptr())
	};
	let guest = Connection::from_backend(backend);
	let mut native_queue = guest.new_event_queue();
	guest.display().get_registry(&native_queue.handle(), ());
	let mut state = State::default();
	native_queue.roundtrip(&mut state).unwrap();
	native_queue.roundtrip(&mut state).unwrap();
	native_queue.roundtrip(&mut state).unwrap();
	// SAFETY: The original proxy belongs to `connection` and remains alive.
	let id = unsafe {
		ObjectId::from_ptr(XdgToplevel::interface(), toplevel.id().as_ptr())
	}
	.unwrap();
	let imported = XdgToplevel::from_id(&guest, id).unwrap();
	commands.send(true).unwrap();
	injected.recv_timeout(Duration::from_secs(2)).unwrap();
	native_queue.roundtrip(&mut state).unwrap();
	assert_eq!(state.down.len(), 3);
	let origin = surface.id().as_ptr().cast();
	state.begin(1, origin, &imported, None);
	state.begin(2, origin, &imported, Some(None));
	state.begin(3, origin, &imported, Some(Some(ResizeDirection::SouthEast)));
	state.begin(2, origin, &imported, Some(None));
	native_queue.roundtrip(&mut state).unwrap();
	assert_eq!(
		actions.recv_timeout(Duration::from_secs(2)).unwrap(),
		(202, None)
	);
	assert_eq!(
		actions.recv_timeout(Duration::from_secs(2)).unwrap(),
		(303, Some(server_toplevel::ResizeEdge::BottomRight))
	);
	assert!(actions.try_recv().is_err());
	assert!(state.down.is_empty());
	drop(imported);
	drop(state);
	drop(native_queue);
	drop(guest);
	toplevel.set_title("still alive".to_owned());
	queue.roundtrip(&mut State::default()).unwrap();
	commands.send(false).unwrap();
	host.join().unwrap();
}
