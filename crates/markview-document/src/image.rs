//! Semantic image sources, before resource loading or layout.
use std::borrow::Cow;

/// Prefix that marks an image source as a Mermaid diagram rather than a path
/// or URL. The scheduler strips it and renders the remainder as diagram source.
pub const MERMAID_SCHEME: &str = "mermaid:";

/// Prefix that keeps a vault-resolved image from colliding with a
/// document-relative image that spells the same source.
const VAULT_KEY_PREFIX: &str = "vault:";

/// The directory a relative image source resolves against.
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq)]
pub enum ImageBase {
	/// The directory of the Markdown document that contains the image.
	#[default]
	Document,
	/// The open folder's root, as an Obsidian vault embed expects.
	Vault,
}

/// One Markdown or HTML image as the document sees it, before any I/O.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ImageSpec {
	pub src: String,
	pub alt: String,
	pub title: String,
	/// Explicit `width` / `height` attributes; `None` keeps the aspect ratio.
	pub width: Option<u32>,
	pub height: Option<u32>,
	/// The directory a relative `src` resolves against.
	pub base: ImageBase,
}

impl ImageSpec {
	/// The resource key for snapshot entries, pixel aliases and draw commands.
	pub fn resource_key(&self) -> Cow<'_, str> {
		match self.base {
			ImageBase::Document => Cow::Borrowed(&self.src),
			ImageBase::Vault => {
				Cow::Owned(format!("{VAULT_KEY_PREFIX}{}", self.src))
			}
		}
	}
}

/// The image source for a Mermaid fence's code, so a diagram travels through
/// the image scheduler like any other source.
pub fn mermaid_source(code: &str) -> String {
	format!("{MERMAID_SCHEME}{code}")
}
