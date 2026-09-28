//! Compressed PDF resources and decoded-image fallback for standalone callers.
use anyhow::{Result, anyhow};
use krilla::image::{BitsPerComponent, CustomImage, Image, ImageColorspace};
use markview_core::image::ImageSnapshot;
use markview_core::image::Pixels;
use std::collections::HashMap;
use std::{
	hash::{Hash, Hasher},
	sync::{Arc, mpsc},
};

/// A compressed PDF image; cloning shares the encoded resource.
#[derive(Clone, Debug)]
pub struct PreparedImage(Image);

impl PreparedImage {
	/// Compresses pixels on Krilla's rayon pool before returning. Waiting here
	/// keeps the loader's concurrency bound effective for compression too.
	pub fn new(pixels: &Pixels) -> Result<Self> {
		let (finished, receiver) = mpsc::channel::<()>();
		let mut rgb = Vec::with_capacity(pixels.rgba.len() / 4 * 3);
		let mut alpha = pixels
			.rgba
			.chunks_exact(4)
			.any(|p| p[3] != 255)
			.then(|| Vec::with_capacity(pixels.rgba.len() / 4));
		for p in pixels.rgba.chunks_exact(4) {
			rgb.extend_from_slice(&p[..3]);
			if let Some(alpha) = &mut alpha {
				alpha.push(p[3]);
			}
		}
		let image = Image::from_custom(
			RawImage(Arc::new(RawImageInner {
				rgb,
				alpha,
				width: pixels.width,
				height: pixels.height,
				_finished: finished,
			})),
			false,
		)
		.map_err(|error| anyhow!("PDF image compression: {error}"))?;
		// Krilla drops its custom source after compression, disconnecting this
		// channel. No raw channels remain queued when the loader continues.
		let _ = receiver.recv();
		Ok(Self(image))
	}
}

#[derive(Clone)]
struct RawImage(Arc<RawImageInner>);

struct RawImageInner {
	rgb: Vec<u8>,
	alpha: Option<Vec<u8>>,
	width: u32,
	height: u32,
	_finished: mpsc::Sender<()>,
}

impl Hash for RawImage {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.0.width.hash(state);
		self.0.height.hash(state);
		self.0.rgb.hash(state);
		self.0.alpha.hash(state);
	}
}

impl CustomImage for RawImage {
	fn color_channel(&self) -> &[u8] {
		&self.0.rgb
	}
	fn alpha_channel(&self) -> Option<&[u8]> {
		self.0.alpha.as_deref()
	}
	fn bits_per_component(&self) -> BitsPerComponent {
		BitsPerComponent::Eight
	}
	fn size(&self) -> (u32, u32) {
		(self.0.width, self.0.height)
	}
	fn icc_profile(&self) -> Option<&[u8]> {
		None
	}
	fn color_space(&self) -> ImageColorspace {
		ImageColorspace::Rgb
	}
}

pub struct Images<'a> {
	snapshot: &'a ImageSnapshot,
	prepared: Option<&'a HashMap<(String, u64), PreparedImage>>,
	cache: HashMap<(String, u64), Option<Image>>,
}

impl<'a> Images<'a> {
	pub fn new(
		snapshot: &'a ImageSnapshot,
		prepared: Option<&'a HashMap<(String, u64), PreparedImage>>,
	) -> Self {
		Self {
			snapshot,
			prepared,
			cache: HashMap::new(),
		}
	}

	/// Resolves an image or reports why exporting it would leave a blank area.
	pub fn get(&mut self, src: &str, version: u64) -> Result<Image> {
		let key = (src.to_owned(), version);
		if let Some(image) = self.prepared.and_then(|images| images.get(&key)) {
			return Ok(image.0.clone());
		}
		if let Some(image) = self.cache.get(&key) {
			return image.clone().ok_or_else(|| self.missing(src, version));
		}
		let pixels = self.snapshot.pixels.decoded().get(src).cloned();
		let image = pixels.map(|pixels| {
			Image::from_rgba8(pixels.rgba.to_vec(), pixels.width, pixels.height)
		});
		self.cache.insert(key, image.clone());
		image.ok_or_else(|| self.missing(src, version))
	}

	fn missing(&self, src: &str, version: u64) -> anyhow::Error {
		let reason = self
			.snapshot
			.entries
			.get(src)
			.and_then(|info| info.error.as_deref())
			.unwrap_or(concat!(
				"neither a prepared PDF image nor decoded pixels are available; ",
				"the image may have been evicted or not loaded"
			));
		anyhow!(
			"PDF export aborted: cannot embed image {src:?} (version {version}): {reason}"
		)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use markview_core::image::Pixels;
	use std::sync::Arc;

	#[test]
	fn poisoned_pixels_report_an_error_and_a_new_version_can_be_embedded() {
		let snapshot = ImageSnapshot::default();
		std::thread::scope(|scope| {
			assert!(
				scope
					.spawn(|| {
						let _pixels = snapshot.pixels.decoded.lock().unwrap();
						panic!("injected failure");
					})
					.join()
					.is_err()
			);
		});
		let mut images = Images::new(&snapshot, None);
		assert!(images.get("image", 1).is_err());
		snapshot.pixels.decoded().insert(
			"image".into(),
			Arc::new(Pixels {
				width: 1,
				height: 1,
				rgba: Arc::from([255, 0, 0, 255]),
			}),
		);
		assert!(images.get("image", 2).is_ok());
		assert!(!snapshot.pixels.decoded.is_poisoned());
	}
}
