use super::{ImageInfo, ImageSnapshot, Pixels};
use crate::{
	scene::{Draw, Paint, Rect},
	style::Color,
};
use std::{num::NonZeroU32, sync::Arc};

/// A cached `color(x, y)` field, sampled in normalized component coordinates.
#[derive(Clone, Debug)]
pub enum ColorField {
	Solid(Color),
	Raster(Arc<Pixels>),
}

/// Compose functions into one pixel buffer before freezing it for drawing.
pub struct ColorFieldBuilder {
	size: [NonZeroU32; 2],
	pixels: Vec<Color>,
}

impl ColorFieldBuilder {
	/// Paint a layer over the previous result using linear-light source-over.
	pub fn paint(self, mut color: impl FnMut(f32, f32) -> Color) -> Self {
		self.map(|x, y, background| over(color(x, y), background))
	}

	/// Transform the current color, for example to apply an alpha mask.
	pub fn map(
		mut self,
		mut color: impl FnMut(f32, f32, Color) -> Color,
	) -> Self {
		let [width, height] = self.size.map(NonZeroU32::get);
		for (index, pixel) in self.pixels.iter_mut().enumerate() {
			*pixel = color(
				((index % width as usize) as f32 + 0.5) / width as f32,
				((index / width as usize) as f32 + 0.5) / height as f32,
				*pixel,
			);
		}
		self
	}

	/// Collapse uniform or transparent results; otherwise retain one shared image.
	pub fn finish(self) -> ColorField {
		if self.pixels.iter().all(|pixel| pixel.0 & 0xFF == 0) {
			ColorField::Solid(Color(0))
		} else if self.pixels.iter().all(|pixel| *pixel == self.pixels[0]) {
			ColorField::Solid(self.pixels[0])
		} else {
			let [width, height] = self.size.map(NonZeroU32::get);
			ColorField::Raster(Arc::new(Pixels {
				width,
				height,
				rgba: self
					.pixels
					.iter()
					.flat_map(|pixel| pixel.0.to_be_bytes())
					.collect(),
			}))
		}
	}
}

fn over(foreground: Color, background: Color) -> Color {
	let f = foreground.0.to_be_bytes();
	let b = background.0.to_be_bytes();
	if f[3] == 0 {
		return background;
	}
	if f[3] == 255 || b[3] == 0 {
		return foreground;
	}
	let fa = f[3] as f32 / 255.;
	let ba = b[3] as f32 / 255. * (1. - fa);
	let alpha = fa + ba;
	let linear = |value: u8| {
		let c = value as f32 / 255.;
		if c <= 0.04045 {
			c / 12.92
		} else {
			((c + 0.055) / 1.055).powf(2.4)
		}
	};
	Color(u32::from_be_bytes(std::array::from_fn(|i| {
		let value = if i == 3 {
			alpha
		} else {
			let c = (linear(f[i]) * fa + linear(b[i]) * ba) / alpha;
			if c <= 0.0031308 {
				c * 12.92
			} else {
				1.055 * c.powf(1. / 2.4) - 0.055
			}
		};
		(value * 255.).round() as u8
	})))
}

impl ColorField {
	/// Start a transparent component at a bounded physical pixel `size`.
	pub fn builder(size: [NonZeroU32; 2]) -> ColorFieldBuilder {
		ColorFieldBuilder {
			size,
			pixels: vec![
				Color(0);
				size[0].get() as usize * size[1].get() as usize
			],
		}
	}

	/// Sample pixel centers at a bounded physical `size`, retaining straight-alpha
	/// sRGB pixels only when the result needs more than one solid color.
	pub fn rasterize(
		size: [NonZeroU32; 2],
		mut color: impl FnMut(f32, f32) -> Color,
	) -> Self {
		Self::builder(size).map(|x, y, _| color(x, y)).finish()
	}

	/// Register a generated resource and return its native draw. Keep `source`
	/// unique to the component and change `version` whenever the pixels change.
	pub fn draw(
		&self,
		rect: Rect,
		source: &str,
		version: u64,
		images: &mut ImageSnapshot,
	) -> Draw {
		match self {
			Self::Solid(color) => {
				if let Some(old) = images.entries.remove(source) {
					images.pixels.remove(source, old.version);
				}
				Draw::Rect(rect, Paint::Color(*color))
			}
			Self::Raster(pixels) => {
				let old = images.entries.insert(
					source.into(),
					ImageInfo {
						version,
						size: Some((pixels.width, pixels.height)),
						error: None,
					},
				);
				images.pixels.insert(source.into(), version, pixels.clone());
				if let Some(old) = old
					&& old.version != version
				{
					images.pixels.remove(source, old.version);
				}
				Draw::Image {
					src: source.into(),
					version,
					rect,
					title: String::new(),
				}
			}
		}
	}
}
