use super::{Draw, Rect};
use std::{fmt::Write, sync::Arc};

/// A trusted WGSL program in normalized component coordinates.
#[derive(Clone, Debug)]
pub struct ColorField {
	shader: Arc<str>,
}

/// Compose WGSL function bodies into a single fragment program.
#[derive(Default)]
pub struct ColorFieldBuilder {
	functions: String,
	calls: String,
	layers: usize,
}

impl ColorField {
	pub fn builder() -> ColorFieldBuilder {
		ColorFieldBuilder::default()
	}

	/// The renderer supplies the rectangle's logical `size` and device `scale`.
	pub fn draw(&self, rect: Rect) -> Draw {
		Draw::ColorField {
			rect,
			shader: self.shader.clone(),
		}
	}
}

impl ColorFieldBuilder {
	/// Composite a WGSL body over the previous color using linear source-over.
	/// Bodies have `x`, `y`, `size`, and `scale`; return linear straight-alpha RGBA.
	pub fn paint(self, body: &str) -> Self {
		self.layer(body, false)
	}

	/// Transform the accumulated linear RGBA, available as `color`.
	pub fn map(self, body: &str) -> Self {
		self.layer(body, true)
	}

	fn layer(mut self, body: &str, transform: bool) -> Self {
		let index = self.layers;
		let input = if transform { ", color: vec4<f32>" } else { "" };
		writeln!(self.functions,
			"fn field_layer_{index}(x: f32, y: f32, size: vec2<f32>, scale: f32{input}) -> vec4<f32> {{\n{body}\n}}"
		).unwrap();
		let call = if transform {
			format!("field_layer_{index}(x, y, size, scale, color)")
		} else {
			format!("field_over(field_layer_{index}(x, y, size, scale), color)")
		};
		writeln!(self.calls, "color = {call};").unwrap();
		self.layers += 1;
		self
	}

	/// Freeze the source; pipelines are compiled lazily and reused by the renderer.
	pub fn finish(self) -> ColorField {
		ColorField {
			shader: format!(
				"{}\nfn field_color(x: f32, y: f32, size: vec2<f32>, scale: f32) -> vec4<f32> {{\nvar color = vec4<f32>(0.0);\n{}return color;\n}}",
				self.functions, self.calls
			).into(),
		}
	}
}
