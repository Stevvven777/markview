//! Direct fragment programs and their ordered geometry ranges. No textures.
use crate::{gpu::Gpu, pipeline};
use std::{
	collections::HashMap,
	ops::Range,
	sync::{Arc, Weak},
};

struct Program {
	pipeline: wgpu::RenderPipeline,
	owners: Vec<Weak<str>>,
}

#[derive(Default)]
pub(super) struct ColorFields {
	cache: HashMap<String, Program>,
	runs: Vec<(Range<u32>, Arc<str>)>,
	compilations: usize,
}

impl ColorFields {
	pub(super) fn begin(&mut self) {
		self.runs.clear();
	}
	pub(super) fn end(&mut self) {
		// Equal programs can have independently allocated source owners.
		self.cache.retain(|_, program| {
			program.owners.retain(|owner| owner.strong_count() > 0);
			!program.owners.is_empty()
		});
	}
	pub(super) fn record(
		&mut self,
		range: Range<u32>,
		source: &Arc<str>,
		gpu: &Gpu,
	) {
		if !self.cache.contains_key(source.as_ref()) {
			self.cache.insert(
				source.to_string(),
				Program {
					pipeline: pipeline::color_field(
						&gpu.device,
						gpu.format,
						source,
					),
					owners: Vec::new(),
				},
			);
			self.compilations += 1;
		}
		let owners = &mut self.cache.get_mut(source.as_ref()).unwrap().owners;
		let owner = Arc::downgrade(source);
		if !owners.iter().any(|existing| existing.ptr_eq(&owner)) {
			owners.push(owner);
		}
		self.runs.push((range, source.clone()));
	}
	pub(super) fn runs(&self) -> &[(Range<u32>, Arc<str>)] {
		&self.runs
	}
	pub(super) fn pipeline(&self, source: &Arc<str>) -> &wgpu::RenderPipeline {
		&self.cache[source.as_ref()].pipeline
	}
	pub(super) fn stats(&self) -> (usize, usize) {
		(self.cache.len(), self.compilations)
	}
}
