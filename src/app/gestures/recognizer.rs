use super::Point;
use std::collections::HashMap;
use winit::event::{DeviceId, TouchPhase};
pub(super) type Contact = (DeviceId, u64);
const SLOP: f32 = 8.0;

pub(super) struct Drag<T> {
	id: Contact,
	origin: Point,
	last: Point,
	axis: Option<bool>,
	target: T,
}

#[derive(Debug, PartialEq)]
pub(super) enum Gesture<T> {
	Tap(T),
	Pan(T, Point),
}

pub(super) struct Recognizer<T> {
	pub(super) contacts: HashMap<Contact, Point>,
	pub(super) drag: Option<Drag<T>>,
}

impl<T> Default for Recognizer<T> {
	fn default() -> Self {
		Self {
			contacts: HashMap::new(),
			drag: None,
		}
	}
}

impl<T: Clone> Recognizer<T> {
	pub(super) fn begin(&mut self, id: Contact, point: Point, target: T) {
		let first = self.contacts.is_empty();
		self.contacts.insert(id, point);
		if first {
			self.drag = Some(Drag {
				id,
				origin: point,
				last: point,
				axis: None,
				target,
			});
		} else {
			self.drag = None;
			// TODO: implement viewport zoom without changing the document layout.
		}
	}

	pub(super) fn update(
		&mut self,
		id: Contact,
		phase: TouchPhase,
		point: Point,
	) -> Option<Gesture<T>> {
		if !self.contacts.contains_key(&id) {
			return None;
		}
		let ending = matches!(phase, TouchPhase::Ended | TouchPhase::Cancelled);
		if ending {
			self.contacts.remove(&id);
		} else {
			let position = self.contacts.get_mut(&id)?;
			*position = point;
		}
		let drag = self.drag.as_mut().filter(|drag| drag.id == id)?;
		if phase == TouchPhase::Cancelled {
			self.drag = None;
			return None;
		}
		let total = (point.0 - drag.origin.0, point.1 - drag.origin.1);
		let mut delta = (point.0 - drag.last.0, point.1 - drag.last.1);
		if drag.axis.is_none() && total.0.hypot(total.1) > SLOP {
			drag.axis = Some(total.0.abs() > total.1.abs());
			delta = total;
		}
		drag.last = point;
		let result = match drag.axis {
			Some(horizontal) => Some(Gesture::Pan(
				drag.target.clone(),
				if horizontal {
					(delta.0, 0.0)
				} else {
					(0.0, delta.1)
				},
			)),
			None if ending => Some(Gesture::Tap(drag.target.clone())),
			None => None,
		};
		if ending {
			self.drag = None;
		}
		result
	}
}

pub(super) use markview_selection::Motion;
