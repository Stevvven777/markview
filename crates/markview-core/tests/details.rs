use markview_core::document::{self, BlockKind};
use std::sync::Arc;

#[test]
fn details_after_front_matter_with_lone_carriage_returns() {
	let source = "---\n\r\r\r\u{fffd}\n---\n<details>\n<d\u{fffd}\u{fffd}\u{fffd}\n\r\r</details>";
	let doc = document::parse(source);
	assert_eq!(doc.blocks.len(), 2);
	let block = &doc.blocks[1];
	assert!(matches!(block.kind, BlockKind::Details { .. }));
	assert_eq!(
		block.source,
		source.find("<details>").unwrap()..source.len()
	);
	assert_eq!(
		&source[doc.blocks[0].source.clone()],
		"---\n\r\r\r\u{fffd}\n---"
	);
	let reparsed = document::reparse(&doc, Arc::from(source));
	assert_eq!(reparsed.content_id, doc.content_id);
	let source: Arc<str> = Arc::from(source);
	for cut in 1..source.len() {
		let _ = document::parse_prefix(&source, cut);
	}
}

#[test]
fn details_closing_range_uses_original_html_bytes() {
	for newline in ["\n", "\r", "\r\n"] {
		for marker in ["", "> "] {
			for prefix in ["<div>é", "<div>\0é", "<details>A</details>"] {
				let source = format!(
					"{marker}<details>{newline}{marker}{newline}{marker}{prefix}{newline}{marker}  </details>é{newline}"
				);
				let doc = document::parse(source.clone());
				let blocks = if marker.is_empty() {
					&doc.blocks
				} else {
					let BlockKind::Quote { blocks, .. } = &doc.blocks[0].kind
					else {
						panic!("expected a quote: {source:?}");
					};
					blocks
				};
				assert!(
					matches!(blocks[0].kind, BlockKind::Details { .. }),
					"{source:?}"
				);
				let end =
					source.rfind("</details>").unwrap() + "</details>".len();
				assert_eq!(blocks[0].source, marker.len()..end, "{source:?}");
				assert_eq!(
					&source[blocks[0].source.clone()],
					&source[marker.len()..end]
				);
				let BlockKind::Paragraph(tail) = &blocks[1].kind else {
					panic!("expected trailing text: {source:?}");
				};
				assert_eq!(document::plain_text(tail), "é");
			}
		}
	}
}
