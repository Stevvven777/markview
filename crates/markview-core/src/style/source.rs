//! Parsed declarations shared across resolved media environments.
use std::{
	collections::BTreeMap,
	ops::Deref,
	sync::{Arc, OnceLock},
};

use super::{
	ConditionSet, MediaContext, Rule, Stylesheet, StylesheetProperties,
};

type RuleLayer = Arc<[(ConditionSet, u32, Rule)]>;

/// Validated stylesheet declarations before media selection and cascading.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StylesheetSource {
	pub(super) properties: Arc<StylesheetProperties>,
	pub(super) layers: Vec<RuleLayer>,
	pub(super) has_media: bool,
}

impl Deref for StylesheetSource {
	type Target = StylesheetProperties;
	fn deref(&self) -> &Self::Target {
		&self.properties
	}
}

impl StylesheetSource {
	/// Shared declarations beneath every reader and export stylesheet.
	pub fn builtin() -> Arc<Self> {
		static BASE: OnceLock<Arc<StylesheetSource>> = OnceLock::new();
		BASE.get_or_init(|| {
			Arc::new(
				Self::parse(include_str!("../../styles/builtin.mvss.toml"))
					.expect("builtin stylesheet"),
			)
		})
		.clone()
	}

	/// Raw declarations only; merging these never reintroduces fallback fields.
	pub fn named_rules(id: &str) -> Option<Arc<Self>> {
		static SHEETS: OnceLock<Vec<(&str, Arc<StylesheetSource>)>> =
			OnceLock::new();
		SHEETS
			.get_or_init(|| {
				[
					("light", include_str!("../../styles/light.mvss.toml")),
					("dark", include_str!("../../styles/dark.mvss.toml")),
					("celadon", include_str!("../../styles/celadon.mvss.toml")),
					(
						"blueprint",
						include_str!("../../styles/blueprint.mvss.toml"),
					),
					(
						"rosewood",
						include_str!("../../styles/rosewood.mvss.toml"),
					),
					("8-bit", include_str!("../../styles/8-bit.mvss.toml")),
					("print", include_str!("../../styles/print.mvss.toml")),
					(
						"monochrome",
						include_str!("../../styles/monochrome.mvss.toml"),
					),
					(
						"qibaishi",
						include_str!("../../styles/qibaishi.mvss.toml"),
					),
					("vangogh", include_str!("../../styles/vangogh.mvss.toml")),
					(
						"mondrian",
						include_str!("../../styles/mondrian.mvss.toml"),
					),
				]
				.into_iter()
				.map(|(id, source)| {
					(
						id,
						Arc::new(
							Self::parse(source).expect("bundled stylesheet"),
						),
					)
				})
				.collect()
			})
			.iter()
			.find(|(name, _)| *name == id)
			.map(|(_, sheet)| sheet.clone())
	}

	/// Merge declarations without selecting media or building a lookup index.
	pub fn merge(&mut self, higher: &Self) {
		Arc::make_mut(&mut self.properties).overlay(higher);
		self.layers.extend(higher.layers.iter().cloned());
		self.has_media |= higher.has_media;
	}
	/// The number of declarations, including currently inapplicable rules.
	pub fn rule_count(&self) -> usize {
		self.layers.iter().map(|layer| layer.len()).sum()
	}
	/// Select media, cascade layers and index only the effective rules.
	pub fn resolve(self: &Arc<Self>, media: MediaContext) -> Stylesheet {
		let mut sheet = Stylesheet {
			properties: self.properties.clone(),
			rules: self.resolve_rules(media),
			media,
			source: Some(self.clone()),
			..Stylesheet::default()
		};
		sheet.reindex();
		sheet
	}
	pub(super) fn resolve_rules(
		&self,
		media: MediaContext,
	) -> BTreeMap<ConditionSet, Rule> {
		let mut rules = BTreeMap::<ConditionSet, Rule>::new();
		for layer in &self.layers {
			for (conditions, mask, rule) in layer.iter() {
				if *mask == 0 || mask & media.0 != 0 {
					rules.entry(*conditions).or_default().overlay(rule);
				}
			}
		}
		rules
	}
	pub(super) fn from_resolved(sheet: &Stylesheet) -> Self {
		Self {
			properties: sheet.properties.clone(),
			layers: vec![
				sheet
					.rules
					.iter()
					.map(|(conditions, rule)| (*conditions, 0, rule.clone()))
					.collect(),
			],
			has_media: false,
		}
	}
}
