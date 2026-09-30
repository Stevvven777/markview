//! The faces a browser shapes with.
//!
//! A browser has no filesystem to scan, so the front end hands the shaper its
//! faces as bytes. The set below is the pinned one the workspace's own shaping
//! tests use, which mirrors every family the bundled stylesheet asks for.

use markview_core::fonts::FontConfig;
use parley::fontique::Blob;
use std::sync::{Arc, OnceLock};

/// The pinned subsets, in the order the shaper registers them.
const FACES: &[&[u8]] = &[
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSerif-Regular-subset.otf"
	),
	include_bytes!("../../markview-core/tests/fonts/NotoSerif-Bold-subset.otf"),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSerif-Italic-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSans-Regular-subset.otf"
	),
	include_bytes!("../../markview-core/tests/fonts/NotoSans-Bold-subset.otf"),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSans-Italic-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansMono-Regular-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansMono-Bold-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSerifCJKsc-Regular-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSerifCJKsc-Bold-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansCJKsc-Regular-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansCJKsc-Medium-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansCJKsc-Bold-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansMonoCJKsc-Regular-subset.otf"
	),
	include_bytes!(
		"../../markview-core/tests/fonts/NotoSansMonoCJKsc-Bold-subset.otf"
	),
	include_bytes!("../../markview-core/tests/fonts/NotoColorEmoji-subset.ttf"),
];

/// Identity of the set above. The collection cache keys on the tag alone, so
/// a second `Markview` reuses the collection the first one built.
const TAG: u64 = 0x6d76_5f77_6562_0001;

/// The faces this host offers, built once and shared.
pub(crate) fn config() -> FontConfig {
	static CONFIG: OnceLock<FontConfig> = OnceLock::new();
	CONFIG
		.get_or_init(|| {
			FontConfig::from_faces(
				TAG,
				FACES
					.iter()
					.map(|data| Blob::new(Arc::new(*data)))
					.collect(),
			)
		})
		.clone()
}
