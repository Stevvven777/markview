use markview_core::document::{self, BlockKind};

#[test]
fn an_unused_reference_definition_keeps_preceding_details_unchanged() {
	for newline in ["\n", "\r\n", "\r"] {
		for body in [
			"+ \n  [^1]: x\n      ",
			"+ [used]\n      ",
			"\n```\n[used]",
			"[\n\n<?L:|LL\n  </other><?L:|LL\n ",
			"<details>+ \n  [^1]: x\n      </details>",
		] {
			for definitions in ["", "[used]: /used\n"] {
				let source = format!(
					"<details>{body}</details>\n\nEnd\n\n{definitions}"
				)
				.replace('\n', newline);
				let before = document::parse(source.clone());
				let after = document::parse(format!(
					"{source}{newline}[unused]: /unused{newline}"
				));
				// The test owns the details block, not whatever follows it.
				assert_eq!(before.blocks[0], after.blocks[0], "{source:?}");
			}
		}
	}
}

#[test]
fn an_unused_reference_definition_keeps_preceding_table_unchanged() {
	let source = "|---|:-:|---:|\n      |---|:-:|---:|\n|---|:-:|---:|\n|---|:-:|---|\n#\n";
	for newline in ["\n", "\r\n", "\r"] {
		let source = source.replace('\n', newline);
		let before = document::parse(source.clone());
		let after = document::parse(format!(
			"{source}{newline}[unused]: /unused{newline}"
		));
		assert_eq!(before.blocks, after.blocks);
		let BlockKind::Table { rows, .. } = &before.blocks[1].kind else {
			panic!("expected the table")
		};
		let cell = &rows[1][2][0];
		assert_eq!(&source[cell.source.clone()], "---");
	}
}
