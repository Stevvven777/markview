//! Time-driven document scrolling; the host supplies bounds and a clock.
use std::time::Duration;
use web_time::Instant;

pub const SCROLL_MIN: Duration = Duration::from_millis(120);
pub const SCROLL_MAX: Duration = Duration::from_millis(400);
const SCROLL_FULL: f32 = 2400.0;
/// How often a running animation asks the event loop for a frame.
const SCROLL_FRAME: Duration = Duration::from_millis(8);

/// Ease-out cubic: fast away from the start and settling into the target.
/// Both ends are exact and the curve is strictly increasing between them.
pub fn ease_out_cubic(t: f32) -> f32 {
	let remaining = 1.0 - t.clamp(0.0, 1.0);
	1.0 - remaining * remaining * remaining
}

/// A time-driven scroll from one offset to another.
///
/// The offset depends only on elapsed time, so the motion is identical at any
/// frame rate; the duration grows with the distance between a floor and a
/// ceiling, which keeps a one-line step responsive and a whole-page jump calm.
#[derive(Clone, Copy, Debug)]
pub struct ScrollAnimation {
	from: f32,
	to: f32,
	started: Instant,
	pub duration: Duration,
}

impl ScrollAnimation {
	/// Starts a move to `to`, scaling the duration from `from`.
	pub fn new(from: f32, to: f32, now: Instant) -> Self {
		let ratio = ((to - from).abs() / SCROLL_FULL).clamp(0.0, 1.0);
		// Integer nanoseconds keep both bounds exact at the ends.
		let span = (SCROLL_MAX - SCROLL_MIN).as_nanos() as f64;
		let nanos = SCROLL_MIN.as_nanos() as f64 + span * f64::from(ratio);
		Self {
			from,
			to,
			started: now,
			duration: Duration::from_nanos(nanos as u64),
		}
	}

	/// The eased offset at `now`, clamped to the two ends.
	pub fn offset_at(&self, now: Instant) -> f32 {
		let elapsed = now.saturating_duration_since(self.started).as_secs_f32();
		let progress = (elapsed / self.duration.as_secs_f32()).clamp(0.0, 1.0);
		self.from + (self.to - self.from) * ease_out_cubic(progress)
	}

	/// When the last frame is due.
	pub fn end(&self) -> Instant {
		self.started + self.duration
	}

	pub fn finished(&self, now: Instant) -> bool {
		now >= self.end()
	}
}

/// Scroll geometry currently available to a host.
#[derive(Clone, Copy, Debug)]
pub struct ScrollBounds {
	pub max: f32,
	pub complete: bool,
}

/// Scroll offset, deferred destination and animation for one document.
#[derive(Clone, Debug, Default)]
pub struct ScrollState {
	/// Absolute requests may exceed incomplete bounds; `visible` clamps them.
	pub offset: f32,
	pub target: Option<f32>,
	pub animation: Option<ScrollAnimation>,
}
impl ScrollState {
	pub fn visible(&self, bounds: ScrollBounds) -> f32 {
		self.offset.clamp(0.0, bounds.max)
	}
	pub fn resolve(&mut self, bounds: ScrollBounds) {
		if self.animation.is_some() {
			return;
		}
		if let Some(target) = self.target
			&& (bounds.complete || target <= bounds.max)
		{
			self.offset = target.clamp(0.0, bounds.max);
			self.target = None;
		}
		if bounds.complete {
			self.offset = self.offset.clamp(0.0, bounds.max);
		}
	}
	pub fn cancel(&mut self) {
		if let Some(animation) = self.animation.take()
			&& self.target == Some(animation.to)
		{
			self.target = None;
		}
	}
	pub fn set(&mut self, offset: f32, bounds: ScrollBounds) {
		self.cancel();
		self.target = Some(offset.max(0.0));
		// Keep the absolute request while `visible` clamps it to each prefix.
		self.offset = offset.max(0.0);
		self.resolve(bounds);
	}
	pub fn by(&mut self, delta: f32, bounds: ScrollBounds) {
		self.cancel();
		if delta == 0.0 {
			self.target.get_or_insert(self.offset);
			self.resolve(bounds);
			return;
		}
		let base = self.target.filter(|v| v.is_finite()).unwrap_or(self.offset);
		self.target = Some((base + delta).max(0.0));
		self.resolve(bounds);
	}
	pub fn animate_to(&mut self, target: f32, now: Instant) {
		self.target = Some(target.max(0.0));
		self.animation =
			Some(ScrollAnimation::new(self.offset, target.max(0.0), now));
	}
	pub fn animate_by(&mut self, delta: f32, now: Instant) {
		if delta == 0.0 {
			return;
		}
		let base = self.target.filter(|v| v.is_finite()).unwrap_or(self.offset);
		self.animate_to(
			(f64::from(base) + f64::from(delta)).clamp(0.0, f64::from(f32::MAX))
				as f32,
			now,
		);
	}
	pub fn wheel_by(&mut self, delta: f32, now: Instant) {
		if (self.target.unwrap_or(self.offset) - self.offset) * delta < 0.0 {
			self.target = None;
		}
		self.animate_by(delta, now);
	}
	pub fn advance(&mut self, now: Instant, bounds: ScrollBounds) -> bool {
		let Some(animation) = self.animation else {
			return false;
		};
		self.offset = animation.offset_at(now).clamp(0.0, bounds.max);
		let at_end = bounds.complete
			&& animation.to >= bounds.max - 0.5
			&& self.offset >= bounds.max - 0.5;
		if !animation.finished(now) && !at_end {
			return true;
		}
		self.animation = None;
		self.resolve(bounds);
		false
	}
	pub fn deadline(&self, now: Instant) -> Option<Instant> {
		self.animation
			.map(|animation| (now + SCROLL_FRAME).min(animation.end()))
	}
}

/// A selection near a viewport edge asks for a fixed scroll step.
pub fn selection_scroll(
	y: f32,
	top: f32,
	bottom: f32,
	offset: f32,
	max: f32,
) -> f32 {
	if y < top + 24.0 && offset > 0.0 {
		-14.0
	} else if y > bottom - 24.0 && offset < max {
		14.0
	} else {
		0.0
	}
}
