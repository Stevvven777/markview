use std::time::{Duration, Instant};

use super::{InteractionState, PanelPage};

const DURATION: Duration = Duration::from_millis(220);

pub(crate) struct DrawerAnimation {
	from: f32,
	to: f32,
	started: Instant,
	duration: Duration,
}

impl DrawerAnimation {
	fn position(&self, now: Instant) -> f32 {
		let t = (now.saturating_duration_since(self.started).as_secs_f32()
			/ self.duration.as_secs_f32())
		.clamp(0.0, 1.0);
		let eased = 1.0 - (1.0 - t).powi(3);
		self.from + (self.to - self.from) * eased
	}

	pub(super) fn retarget(
		animation: &mut Option<Self>,
		open: bool,
		was_open: bool,
	) {
		Self::retarget_at(animation, open, was_open, Instant::now());
	}

	fn retarget_at(
		animation: &mut Option<Self>,
		open: bool,
		was_open: bool,
		now: Instant,
	) {
		if open == was_open {
			return;
		}
		let from = position(animation, was_open, now);
		let to = u8::from(open) as f32;
		*animation = (from != to).then_some(Self {
			from,
			to,
			started: now,
			duration: DURATION.mul_f32((to - from).abs()),
		});
	}
}

fn position(
	animation: &Option<DrawerAnimation>,
	open: bool,
	now: Instant,
) -> f32 {
	animation
		.as_ref()
		.map_or(u8::from(open) as f32, |a| a.position(now))
}

impl InteractionState {
	pub(crate) fn tabs_reveal(&self) -> f32 {
		position(
			&self.tabs_animation,
			self.panel == PanelPage::Tabs,
			Instant::now(),
		)
	}

	pub(crate) fn outline_reveal(&self) -> f32 {
		position(&self.outline_animation, self.outline_open, Instant::now())
	}

	pub(crate) fn drawer_deadline(&self, now: Instant) -> Option<Instant> {
		(self.tabs_animation.is_some() || self.outline_animation.is_some())
			.then_some(now + Duration::from_millis(16))
	}

	pub(crate) fn advance_drawers(&mut self, now: Instant) -> bool {
		let active = self.drawer_deadline(now).is_some();
		for animation in [&mut self.tabs_animation, &mut self.outline_animation]
		{
			if animation
				.as_ref()
				.is_some_and(|a| now >= a.started + a.duration)
			{
				*animation = None;
			}
		}
		active
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn drawer_motion_eases_reverses_continuously_and_settles() {
		let start = Instant::now();
		let mut animation = None;
		DrawerAnimation::retarget_at(&mut animation, true, false, start);
		assert_eq!(position(&animation, true, start), 0.0);
		let halfway = start + DURATION / 2;
		let revealed = position(&animation, true, halfway);
		assert_eq!(revealed, 0.875);
		assert_eq!(position(&animation, true, start + DURATION), 1.0);
		DrawerAnimation::retarget_at(&mut animation, false, true, halfway);
		assert_eq!(position(&animation, false, halfway), revealed);
		assert!(position(&animation, false, halfway + DURATION / 4) < revealed);
		assert_eq!(position(&animation, false, halfway + DURATION), 0.0);
	}
}
