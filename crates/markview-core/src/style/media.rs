//! Output, device, platform and orientation conditions for MVSS rules.
use super::StyleTarget;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Media {
	Ui,
	Pdf,
	Desktop,
	Mobile,
	Tablet,
	Phone,
	Landscape,
	Portrait,
	Android,
	Linux,
	Windows,
	Macos,
	Ios,
	Web,
}

impl Media {
	pub(super) const fn bit(self) -> u32 {
		1 << self as u32
	}
}

/// The environment against which a rule's `media` requirements are matched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MediaContext(pub(super) u32);

impl Default for MediaContext {
	fn default() -> Self {
		Self::native(StyleTarget::Ui)
	}
}

impl MediaContext {
	pub fn new(
		target: StyleTarget,
		device: Media,
		platform: Option<Media>,
	) -> Self {
		let output = match target {
			StyleTarget::Ui => Media::Ui,
			StyleTarget::Pdf => Media::Pdf,
		};
		let mut bits = output.bit() | device.bit();
		if matches!(device, Media::Phone | Media::Tablet) {
			bits |= Media::Mobile.bit();
		}
		if let Some(platform) = platform {
			bits |= platform.bit();
		}
		Self(bits)
	}

	pub fn native(target: StyleTarget) -> Self {
		let platform = if cfg!(target_arch = "wasm32") {
			Some(Media::Web)
		} else {
			match std::env::consts::OS {
				"android" => Some(Media::Android),
				"linux" => Some(Media::Linux),
				"windows" => Some(Media::Windows),
				"macos" => Some(Media::Macos),
				"ios" => Some(Media::Ios),
				_ => None,
			}
		};
		let device = if cfg!(any(target_os = "android", target_os = "ios")) {
			Media::Phone
		} else {
			Media::Desktop
		};
		Self::new(target, device, platform)
	}

	pub fn with_device(mut self, device: Media) -> Self {
		self.0 &= !(Media::Desktop.bit()
			| Media::Mobile.bit()
			| Media::Tablet.bit()
			| Media::Phone.bit());
		self.0 |= device.bit();
		if matches!(device, Media::Phone | Media::Tablet) {
			self.0 |= Media::Mobile.bit();
		}
		self
	}

	/// Match orientation to the whole viewport or the exported paper size.
	pub fn with_size(mut self, width: f32, height: f32) -> Self {
		self.0 &= !(Media::Landscape.bit() | Media::Portrait.bit());
		self.0 |= if width > height {
			Media::Landscape
		} else {
			Media::Portrait
		}
		.bit();
		self
	}
}
