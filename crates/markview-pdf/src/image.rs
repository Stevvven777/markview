//! Decoded images, embedded once per export.
use krilla::image::Image;
use markview_core::image::ImageSnapshot;
use std::collections::HashMap;

pub struct Images<'a> {
	snapshot: &'a ImageSnapshot,
	cache: HashMap<(String, u64), Option<Image>>,
}

impl<'a> Images<'a> {
	pub fn new(snapshot: &'a ImageSnapshot) -> Self {
		Self {
			snapshot,
			cache: HashMap::new(),
		}
	}

	/// The image for a source at a version, or `None` while its pixels are
	/// missing, which is what an unavailable image already looks like on
	/// screen.
	pub fn get(&mut self, src: &str, version: u64) -> Option<Image> {
		let key = (src.to_owned(), version);
		if let Some(image) = self.cache.get(&key) {
			return image.clone();
		}
		let pixels = self.snapshot.pixels.decoded().get(src).cloned();
		let image = pixels.map(|pixels| {
			Image::from_rgba8(pixels.rgba.to_vec(), pixels.width, pixels.height)
		});
		self.cache.insert(key, image.clone());
		image
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use markview_core::image::Pixels;
	use std::sync::Arc;

	#[test]
	fn poisoned_pixels_are_omitted_and_a_new_version_can_be_embedded() {
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
		let mut images = Images::new(&snapshot);
		assert!(images.get("image", 1).is_none());
		snapshot.pixels.decoded().insert(
			"image".into(),
			Arc::new(Pixels {
				width: 1,
				height: 1,
				rgba: Arc::from([255, 0, 0, 255]),
			}),
		);
		assert!(images.get("image", 2).is_some());
		assert!(!snapshot.pixels.decoded.is_poisoned());
	}
}
