//! Semantic Markdown; raw HTML is limited to a small supported subset.
use crate::html;
use comrak::{
	Arena, Options,
	nodes::{AstNode, ListType, NodeValue, TableAlignment},
	parse_document,
};
use std::{
	borrow::Cow,
	cell::Cell,
	collections::{HashMap, HashSet},
	ops::Range,
	rc::Rc,
	sync::Arc,
};

use super::{
	Block, BlockKind, CellAlign, Document, Inline, InlineKind, ListItem,
	RichText, TextStyle, content_identity, fingerprint, front_matter,
	incremental, plain_text, semantic_key,
};

struct Reader<'s> {
	source: &'s str,
	lines: Vec<usize>,
	footnotes: HashMap<String, u32>,
	/// Definitions the full parse already owns; snippets only reference them.
	document_footnotes: &'s HashSet<String>,
	/// Definitions kept until every disclosure's references are known.
	footnote_blocks: HashMap<String, Block>,
	/// Appended definitions resolve references but never emit note bodies.
	snippet_end: Option<usize>,
	/// Digit columns every note reserves for its number; see
	/// [`BlockKind::Footnote`].
	footnote_column: u32,
	/// How many `<details>` elements this document has already numbered.
	details_ordinal: u32,
	/// The document's reference and footnote definitions, which resolve inside
	/// a snippet the same way they do in a full parse.
	definitions: &'s str,
	limits: crate::limits::Limits,
	/// Cumulative snippet bytes whose global definitions were injected. A
	/// document-wide ceiling keeps repeated snippet parses from multiplying
	/// the definitions.
	reparse_used: Rc<Cell<usize>>,
	reparse_limit: usize,
}

impl Reader<'_> {
	fn range(&self, node: &AstNode<'_>) -> Range<usize> {
		let data = node.data.borrow();
		let p = data.sourcepos;
		let line_start = self
			.lines
			.get(p.start.line.saturating_sub(1))
			.copied()
			.unwrap_or(0);
		let start = line_start + p.start.column.saturating_sub(1);
		let end = if matches!(data.value, NodeValue::ThematicBreak) {
			// Comrak can leave a break open through trailing blank lines at
			// EOF. Its source range belongs only to the marker's line.
			let line = &self.source[line_start..];
			line_start + line.find(['\r', '\n']).unwrap_or(line.len())
		} else {
			self.lines
				.get(p.end.line.saturating_sub(1))
				.copied()
				.unwrap_or(0)
				+ p.end.column
		};
		// Comrak columns are byte offsets unless sourcepos_chars is enabled.
		let mut start = start.min(self.source.len());
		let mut end = end.min(self.source.len()).max(start);
		while !self.source.is_char_boundary(start) {
			start -= 1;
		}
		while !self.source.is_char_boundary(end) {
			end += 1;
		}
		start..end
	}

	fn inlines<'a>(
		&self,
		node: &'a AstNode<'a>,
		style: &TextStyle,
		out: &mut RichText,
		depth: usize,
		mut from: usize,
	) {
		// Comrak builds the AST iteratively but produces recursion as deep as
		// the input demands; past the budget the remaining text is kept flat
		// instead of descending.
		if depth >= self.limits.inline_depth {
			let text = self.flattened_from(node, from);
			if !text.is_empty() {
				out.push(Inline {
					kind: InlineKind::Text(text),
					style: style.clone(),
					source: self.range(node).start.max(from)
						..self.range(node).end,
				});
			}
			return;
		}
		// Raw HTML tags are siblings, so a supported tag opens a style scope
		// that the matching closing tag ends; unsupported markup stays source.
		let mut style = style.clone();
		let mut scopes: Vec<(String, TextStyle)> = Vec::new();
		for child in node.children() {
			let source = self.range(child);
			if from > 0 && source.end <= from {
				continue;
			}
			if matches!(child.data.borrow().value, NodeValue::HtmlInline(_))
				&& let Some(len) = html::svg_len(&self.source[source.start..])
				&& source.start + len <= self.range(node).end
			{
				let end = source.start + len;
				let markers = container_markers(child);
				let raw =
					HtmlSource::new(&self.source[source.start..end], &markers);
				let (_, image) = html::svg(&raw.text).unwrap();
				out.push(Inline {
					kind: InlineKind::Image(image),
					style: style.clone(),
					source: source.start..end,
				});
				from = end;
				continue;
			}
			let mut child_style = style.clone();
			let value = child.data.borrow();
			let kind = match &value.value {
				NodeValue::Text(t) => Some(InlineKind::Text(self.text_after(
					t,
					source.clone(),
					from,
				))),
				NodeValue::SoftBreak => Some(InlineKind::Text(" ".into())),
				NodeValue::LineBreak => {
					Some(InlineKind::LineBreak { justify: false })
				}
				NodeValue::Code(c) => {
					child_style.code = true;
					Some(InlineKind::Text(self.text_after(
						&c.literal,
						source.clone(),
						from,
					)))
				}
				NodeValue::Raw(t) => {
					child_style.code = true;
					Some(InlineKind::Text(self.text_after(
						t,
						source.clone(),
						from,
					)))
				}
				NodeValue::HtmlInline(t) => match html::inline(t) {
					html::Inline::Image(image) => {
						Some(InlineKind::Image(image))
					}
					html::Inline::Ignore => continue,
					html::Inline::Break => {
						Some(InlineKind::LineBreak { justify: true })
					}
					html::Inline::Open { name, patch } => {
						scopes.push((name, style.clone()));
						apply_patch(&patch, &mut style);
						continue;
					}
					html::Inline::Close { name } => {
						if let Some(i) =
							scopes.iter().rposition(|(open, _)| *open == name)
						{
							style = scopes[i].1.clone();
							scopes.truncate(i);
						}
						continue;
					}
					html::Inline::Literal => {
						child_style.code = true;
						Some(InlineKind::Text(t.clone()))
					}
				},
				NodeValue::Math(m) => Some(InlineKind::Math {
					latex: self.text_after(&m.literal, source.clone(), from),
					display: m.display_math,
				}),
				NodeValue::FootnoteReference(f) => {
					let label = f.ix.to_string();
					child_style.superscript = true;
					child_style.footnote_ref = true;
					child_style.link = Some(super::footnote::url(&label));
					Some(InlineKind::FootnoteRef(f.ix))
				}
				NodeValue::Strong => {
					child_style.bold = true;
					None
				}
				NodeValue::Emph => {
					child_style.italic = true;
					None
				}
				NodeValue::Strikethrough => {
					child_style.strike = true;
					None
				}
				NodeValue::Link(l) => {
					child_style.link = Some(l.url.clone());
					None
				}
				NodeValue::Image(link) => {
					let mut alt = Vec::new();
					self.inlines(
						child,
						&TextStyle::default(),
						&mut alt,
						depth + 1,
						from,
					);
					let alt = plain_text(&alt);
					Some(InlineKind::Image(crate::image::ImageSpec {
						src: link.url.clone(),
						alt,
						title: link.title.clone(),
						width: None,
						height: None,
					}))
				}
				_ => None,
			};
			if let Some(kind) = kind {
				out.push(Inline {
					kind,
					style: child_style,
					source: source.start.max(from)..source.end,
				});
			} else {
				self.inlines(child, &child_style, out, depth + 1, from);
			}
		}
	}

	fn text_after(
		&self,
		text: &str,
		source: Range<usize>,
		from: usize,
	) -> String {
		if source.start >= from {
			return text.to_string();
		}
		let mut cursor = source.start;
		for (i, ch) in text.char_indices() {
			// Code spans normalize line endings to spaces.
			if let Some(at) = self.source[cursor..source.end].find(|raw| {
				raw == ch || ch == ' ' && matches!(raw, '\r' | '\n')
			}) {
				let at = cursor + at;
				if at >= from {
					return text[i..].to_string();
				}
				cursor =
					at + self.source[at..].chars().next().unwrap().len_utf8();
				if ch == ' ' && self.source[at..].starts_with("\r\n") {
					cursor += 1;
				}
			}
		}
		String::new()
	}

	/// The readable text after `from`, collected without recursion.
	fn flattened_from<'a>(&self, node: &'a AstNode<'a>, from: usize) -> String {
		let mut text = String::new();
		for descendant in node.descendants() {
			let source = self.range(descendant);
			if from > 0 && source.end <= from {
				continue;
			}
			match &descendant.data.borrow().value {
				NodeValue::Text(t) => {
					text.push_str(&self.text_after(t, source, from))
				}
				NodeValue::Raw(t) => {
					text.push_str(&self.text_after(t, source, from))
				}
				NodeValue::Code(c) => {
					text.push_str(&self.text_after(&c.literal, source, from))
				}
				NodeValue::Math(m) => {
					text.push_str(&self.text_after(&m.literal, source, from))
				}
				NodeValue::SoftBreak => text.push(' '),
				NodeValue::LineBreak => text.push('\n'),
				_ => {}
			}
		}
		text
	}

	fn rich<'a>(&self, node: &'a AstNode<'a>) -> RichText {
		let mut text = Vec::new();
		self.inlines(node, &TextStyle::default(), &mut text, 0, 0);
		merge_text(text)
	}

	fn blocks<'a>(
		&mut self,
		node: &'a AstNode<'a>,
		depth: usize,
	) -> Vec<Block> {
		let children: Vec<&'a AstNode<'a>> = node.children().collect();
		self.sequence(&children, depth)
	}

	/// One sibling list. A raw HTML container may span several siblings, so
	/// the walk keeps a stack of open frames; the Markdown siblings Comrak
	/// already parsed become the body and are never re-parsed.
	fn sequence<'a>(
		&mut self,
		children: &[&'a AstNode<'a>],
		depth: usize,
	) -> Vec<Block> {
		let mut out = Vec::new();
		let mut stack: Vec<Frame> = Vec::new();
		let mut i = 0;
		while i < children.len() {
			if let Some(consumed) =
				self.svg(children, i, depth, frame_target(&mut stack, &mut out))
			{
				i += consumed;
				continue;
			}
			let child = children[i];
			if matches!(&child.data.borrow().value, NodeValue::HtmlBlock(_)) {
				self.html_events(child, depth, &mut stack, &mut out);
			} else {
				let content_depth = depth + stack.len();
				match self.child_block(child, content_depth) {
					Child::Block(block) => {
						push_block(&mut stack, &mut out, block);
					}
					Child::Skip => {}
					Child::Flatten => {
						let blocks = self.blocks(child, content_depth + 1);
						extend_blocks(&mut stack, &mut out, blocks);
					}
				}
			}
			i += 1;
		}
		// An element that never closed keeps the literal fallback: the opener
		// is shown as HTML source and its collected content follows it.
		while let Some(frame) = stack.pop() {
			let mut blocks = Vec::new();
			if let Some(block) = html_leaf_block(
				self.source,
				&frame.raw,
				frame.start..frame.start.saturating_add(frame.raw.len()),
			) {
				blocks.push(block);
			}
			blocks.extend(frame.blocks);
			extend_blocks(&mut stack, &mut out, blocks);
		}
		out
	}

	/// Feed one raw HTML block's container events into the open frames.
	fn html_events<'a>(
		&mut self,
		child: &'a AstNode<'a>,
		depth: usize,
		stack: &mut Vec<Frame>,
		out: &mut Vec<Block>,
	) {
		let source = self.source;
		let child_range = self.range(child);
		let markers = container_markers(child);
		let original = HtmlSource::new(&source[child_range.clone()], &markers);
		for event in html::block_events(&original.text) {
			match event {
				html::BlockEvent::Text { text, range } => {
					let at = map_source(&original, &child_range, range);
					if let Some(block) = html_leaf_block(source, &text, at) {
						push_block(stack, out, block);
					}
				}
				html::BlockEvent::Summary { text, range } => {
					let set = if let Some(frame) = stack.last_mut()
						&& let FrameKind::Details { summary, .. } =
							&mut frame.kind && summary.is_none()
					{
						*summary = Some(text.clone());
						frame.raw.push_str(&original.text[range.clone()]);
						true
					} else {
						false
					};
					if !set {
						let at = map_source(&original, &child_range, range);
						let html = format!("<summary>{text}</summary>");
						if let Some(block) = html_leaf_block(source, &html, at)
						{
							push_block(stack, out, block);
						}
					}
				}
				html::BlockEvent::Open { name, open, range } => {
					let at = map_source(&original, &child_range, range.clone());
					if depth + stack.len() >= self.limits.block_depth {
						// Too deep: keep the tag as source instead of nesting.
						if let Some(block) = html_leaf_block(
							source,
							&original.text[range.clone()],
							at,
						) {
							push_block(stack, out, block);
						}
						continue;
					}
					let kind = if name == "details" {
						let ordinal = self.details_ordinal;
						self.details_ordinal += 1;
						FrameKind::Details {
							open,
							ordinal,
							summary: None,
						}
					} else {
						FrameKind::Transparent { name }
					};
					stack.push(Frame {
						kind,
						blocks: Vec::new(),
						raw: original.text[range.clone()].to_string(),
						start: at.start,
					});
				}
				html::BlockEvent::Close { name, range } => {
					let at = map_source(&original, &child_range, range.clone());
					if !self.close_frame(&name, at.end, stack, out, source) {
						// A stray close keeps only its own fragment as source.
						if let Some(block) = html_leaf_block(
							source,
							&original.text[range.clone()],
							at,
						) {
							push_block(stack, out, block);
						}
					}
				}
			}
		}
	}

	/// Pop the frame a closing tag names, or report that none matches so the
	/// caller can keep the literal fallback.
	fn close_frame(
		&self,
		name: &str,
		end: usize,
		stack: &mut Vec<Frame>,
		out: &mut Vec<Block>,
		source: &str,
	) -> bool {
		// An end tag closes the nearest matching frame and implicitly closes
		// anything still open inside it.
		let Some(index) = stack.iter().rposition(|frame| match &frame.kind {
			FrameKind::Transparent { name: open } => name == open,
			FrameKind::Details { .. } => name == "details",
		}) else {
			return false;
		};
		while stack.len() > index {
			let frame = stack.pop().expect("index is in range");
			self.finish_frame(frame, end, stack, out, source);
		}
		true
	}

	/// Close one frame into its parent: a transparent frame flattens and a
	/// details frame becomes one block.
	fn finish_frame(
		&self,
		frame: Frame,
		end: usize,
		stack: &mut [Frame],
		out: &mut Vec<Block>,
		source: &str,
	) {
		match frame.kind {
			FrameKind::Transparent { .. } => {
				extend_blocks(stack, out, frame.blocks);
			}
			FrameKind::Details {
				open,
				ordinal,
				summary,
			} => {
				let span = frame.start..end.max(frame.start);
				let summary = self.summary_rich(summary.as_deref(), &span);
				let block = details_block_of(
					source,
					open,
					ordinal,
					summary,
					frame.blocks,
					span,
				);
				push_block(stack, out, block);
			}
		}
	}

	/// SVG may cross Comrak's blank-line HTML boundaries.
	fn svg<'a>(
		&mut self,
		children: &[&'a AstNode<'a>],
		start: usize,
		depth: usize,
		out: &mut Vec<Block>,
	) -> Option<usize> {
		if depth >= self.limits.block_depth {
			return None;
		}
		let node = children[start];
		let range = self.range(node);
		let begin = match &node.data.borrow().value {
			NodeValue::HtmlBlock(_) => range.start,
			NodeValue::Paragraph => node.children().find_map(|child| {
				if !matches!(
					child.data.borrow().value,
					NodeValue::HtmlInline(_)
				) {
					return None;
				}
				let at = self.range(child).start;
				let len = html::svg_len(&self.source[at..])?;
				(at + len > range.end).then_some(at)
			})?,
			_ => return None,
		};
		let mut end = begin + html::svg_len(&self.source[begin..])?;
		let mut close = (start..children.len())
			.find(|&i| self.range(children[i]).end >= end)?;
		let markers = container_markers(node);
		if begin > range.start {
			let prefix =
				HtmlSource::new(&self.source[range.start..begin], &markers);
			out.extend(self.markdown_blocks_at(
				&prefix.text,
				depth,
				range.start..begin,
			));
		}
		let mut begin = begin;
		loop {
			let raw = HtmlSource::new(&self.source[begin..end], &markers);
			let (_, image) = html::svg(&raw.text)?;
			let source = begin..end;
			let kind = BlockKind::Paragraph(vec![Inline {
				kind: InlineKind::Image(image),
				style: TextStyle::default(),
				source: source.clone(),
			}]);
			out.push(Block {
				id: fingerprint(&(
					std::mem::discriminant(&kind),
					&self.source[source.clone()],
				)),
				content_key: semantic_key(&kind),
				source,
				kind,
			});
			let tail_end = self.range(children[close]).end;
			let mut rest = &self.source[end..tail_end];
			loop {
				let next =
					without_markers(rest.trim_start(), &markers).trim_start();
				if next.len() == rest.len() {
					break;
				}
				rest = next;
			}
			let next_begin = tail_end - rest.len();
			if !rest.is_empty()
				&& let Some(len) = html::svg_len(&self.source[next_begin..])
				&& let Some(next_close) = (close..children.len())
					.find(|&i| self.range(children[i]).end >= next_begin + len)
			{
				begin = next_begin;
				end = begin + len;
				close = next_close;
				continue;
			}
			let tail = HtmlSource::new(&self.source[end..tail_end], &markers);
			if !tail.text.trim().is_empty() {
				out.extend(self.markdown_blocks_at(
					&tail.text,
					depth,
					end..tail_end,
				));
			}
			break;
		}
		Some(close - start + 1)
	}

	fn child_block<'a>(
		&mut self,
		child: &'a AstNode<'a>,
		depth: usize,
	) -> Child {
		let source = self.range(child);
		let data = child.data.borrow();
		let kind = if depth >= self.limits.block_depth {
			BlockKind::Code {
				language: "nested Markdown".into(),
				text: self.source[source.clone()].to_string(),
			}
		} else {
			match &data.value {
				NodeValue::FrontMatter(text) => {
					match front_matter::parse(text) {
						front_matter::Content::Empty => {
							return Child::Skip;
						}
						front_matter::Content::Source(yaml) => {
							let code = BlockKind::Code {
								language: front_matter::LANGUAGE.into(),
								text: yaml,
							};
							let id = fingerprint(&(
								std::mem::discriminant(&code),
								&self.source[source.clone()],
							));
							BlockKind::FrontMatter {
								open: false,
								blocks: vec![Block {
									id,
									content_key: semantic_key(&code),
									source: source.clone(),
									kind: code,
								}],
							}
						}
					}
				}
				NodeValue::Paragraph => BlockKind::Paragraph(self.rich(child)),
				NodeValue::Heading(h) => {
					let text = self.rich(child);
					BlockKind::Heading {
						level: h.level,
						text,
						anchor: String::new(),
					}
				}
				// The info string's first word is the language; trailing
				// words are metadata, so `mermaid title="x"` still renders.
				NodeValue::CodeBlock(c)
					if c.info.split_whitespace().next() == Some("mermaid") =>
				{
					BlockKind::Paragraph(vec![Inline {
						kind: InlineKind::Image(crate::image::ImageSpec {
							src: crate::image::mermaid_source(&c.literal),
							// An empty `alt` draws no caption and keeps the
							// fence source out of the reading text, so only
							// the placeholder message is selectable.
							alt: String::new(),
							title: String::new(),
							width: None,
							height: None,
						}),
						style: TextStyle::default(),
						source: source.clone(),
					}])
				}
				NodeValue::CodeBlock(c) if c.info.trim() == "math" => {
					BlockKind::Paragraph(vec![Inline {
						kind: InlineKind::Math {
							latex: c.literal.clone(),
							display: true,
						},
						style: TextStyle::default(),
						source: source.clone(),
					}])
				}
				NodeValue::CodeBlock(c) => BlockKind::Code {
					language: c.info.clone(),
					text: c.literal.clone(),
				},
				NodeValue::HtmlBlock(h) => match html::block(&h.literal) {
					html::Block::Unsupported => BlockKind::Code {
						language: "HTML source".into(),
						text: h.literal.clone(),
					},
					html::Block::Empty => return Child::Skip,
					html::Block::Rule => BlockKind::Rule,
					html::Block::Heading { level, text } => {
						let text = html_rich(text, &source);
						BlockKind::Heading {
							level,
							text,
							anchor: String::new(),
						}
					}
					html::Block::Paragraph(text) => {
						BlockKind::Paragraph(html_rich(text, &source))
					}
				},
				NodeValue::ThematicBreak => BlockKind::Rule,
				NodeValue::BlockQuote => BlockKind::Quote {
					label: None,
					blocks: self.blocks(child, depth + 1),
				},
				NodeValue::Alert(a) => BlockKind::Quote {
					label: Some(format!("{:?}", a.alert_type)),
					blocks: self.blocks(child, depth + 1),
				},
				NodeValue::List(l) => BlockKind::List {
					start: (l.list_type == ListType::Ordered)
						.then_some(l.start),
					tight: l.tight,
					items: child
						.children()
						.map(|item| {
							let checked = match &item.data.borrow().value {
								NodeValue::TaskItem(t) => {
									Some(t.symbol.is_some())
								}
								_ => None,
							};
							ListItem {
								checked,
								blocks: self.blocks(item, depth + 1),
							}
						})
						.collect(),
				},
				NodeValue::Table(t) => BlockKind::Table {
					align: t
						.alignments
						.iter()
						.map(|a| match a {
							TableAlignment::Center => CellAlign::Center,
							TableAlignment::Right => CellAlign::Right,
							_ => CellAlign::Left,
						})
						.collect(),
					rows: child
						.children()
						.map(|r| r.children().map(|c| self.rich(c)).collect())
						.collect(),
				},
				NodeValue::FootnoteDefinition(f)
					if (self.snippet_end.is_none()
						&& f.total_references == 0)
						|| self.snippet_end.is_some_and(|end| {
							source.start >= end
								|| self
									.document_footnotes
									.contains(&note_key(&f.name))
						}) =>
				{
					return Child::Skip;
				}
				NodeValue::FootnoteDefinition(f) => BlockKind::Footnote {
					label: self
						.footnotes
						.get(&note_key(&f.name))
						.map_or_else(|| f.name.clone(), u32::to_string),
					column: self.footnote_column,
					blocks: self.blocks(child, depth + 1),
				},
				_ => {
					return Child::Flatten;
				}
			}
		};
		// Content identity deliberately excludes source offsets, which shift on append/insert.
		let id = fingerprint(&(
			std::mem::discriminant(&kind),
			&self.source[source.clone()],
		));
		let content_key = semantic_key(&kind);
		let block = Block {
			id,
			content_key,
			source,
			kind,
		};
		if let NodeValue::FootnoteDefinition(f) = &data.value {
			// Snippet-only definitions also belong at the document's end.
			self.footnote_blocks.insert(note_key(&f.name), block);
			Child::Skip
		} else {
			Child::Block(block)
		}
	}

	/// Restore snippet coordinates after removing disclosure tags and quote prefixes.
	fn markdown_blocks_at(
		&mut self,
		text: &str,
		depth: usize,
		source: Range<usize>,
	) -> Vec<Block> {
		let mut blocks = self.markdown_blocks(text, depth);
		let offsets = source_offsets(text, self.source, source);
		fn range(range: &mut Range<usize>, offsets: &[usize]) {
			let start = offsets[range.start.min(offsets.len() - 1)];
			let end = if range.end > range.start {
				offsets[(range.end - 1).min(offsets.len() - 1)] + 1
			} else {
				start
			};
			*range = start..end.max(start);
		}
		fn rich(text: &mut RichText, offsets: &[usize]) {
			for inline in text {
				range(&mut inline.source, offsets);
			}
		}
		fn walk(blocks: &mut [Block], offsets: &[usize]) {
			for block in blocks {
				range(&mut block.source, offsets);
				match &mut block.kind {
					BlockKind::Paragraph(text)
					| BlockKind::Heading { text, .. } => rich(text, offsets),
					BlockKind::Details {
						summary, blocks, ..
					} => {
						rich(summary, offsets);
						walk(blocks, offsets);
					}
					BlockKind::Quote { blocks, .. }
					| BlockKind::Footnote { blocks, .. }
					| BlockKind::FrontMatter { blocks, .. } => walk(blocks, offsets),
					BlockKind::List { items, .. } => {
						for item in items {
							walk(&mut item.blocks, offsets);
						}
					}
					BlockKind::Table { rows, .. } => {
						for row in rows {
							for cell in row {
								rich(cell, offsets);
							}
						}
					}
					_ => {}
				}
			}
		}
		walk(&mut blocks, &offsets);
		blocks
	}

	/// The block list `text` describes, parsed by the ordinary pipeline. The
	/// shared anchors keep headings inside the snippet unique in the document.
	fn markdown_blocks(&mut self, text: &str, depth: usize) -> Vec<Block> {
		if text.trim().is_empty() {
			return Vec::new();
		}
		// A snippet only holds part of the document, so a reference or note it
		// uses may be defined outside it; parsing it together with the
		// document's definitions resolves those, and only the blocks the
		// snippet itself covers are kept. Terminate it the same way even without
		// definitions, so an unused definition cannot change its EOF ranges.
		if !text.contains('[') {
			return self.snippet(text, depth, None);
		}
		let cost = text.len().saturating_add(self.definitions.len());
		if self.reparse_used.get().saturating_add(cost) > self.reparse_limit {
			// Over the document-wide ceiling: keep the snippet readable without
			// the global definitions so repeated parses stay bounded.
			return self.snippet(text, depth, None);
		}
		self.reparse_used
			.set(self.reparse_used.get().saturating_add(cost));
		let definitions =
			incremental::missing_definitions(self.definitions, text);
		let mut joined =
			String::with_capacity(text.len() + definitions.len() + 2);
		joined.push_str(text);
		joined.push_str("\n\n");
		joined.push_str(&definitions);
		let blocks = self.snippet(&joined, depth, Some(text));
		// An unclosed fence or HTML block can swallow the appended
		// definitions; then the bare snippet parses to what the full document
		// puts there.
		if blocks
			.iter()
			.any(|b| b.source.start < text.len() && b.source.end > text.len())
		{
			return self.snippet(text, depth, None);
		}
		blocks
			.into_iter()
			.filter(|b| b.source.start < text.len())
			.collect()
	}

	/// `text` parsed by the ordinary pipeline, falling back to `bare` when a
	/// literal block absorbs the appended bytes.
	fn snippet(
		&mut self,
		text: &str,
		depth: usize,
		bare: Option<&str>,
	) -> Vec<Block> {
		let arena = Arena::new();
		let mut options = markdown_options();
		// Another disclosure may reference a definition this snippet owns.
		options.parse.leave_footnote_definitions = true;
		let root = parse_document(&arena, text, &options);
		let lines = line_starts(text);
		if let Some(bare) = bare
			&& root.descendants().any(|node| {
				let data = node.data.borrow();
				let literal = match &data.value {
					NodeValue::CodeBlock(_) => true,
					NodeValue::HtmlBlock(h) => {
						// Raw HTML can consume synthetic blank lines while its
						// source position excludes them.
						if h.block_type <= 5
							&& text.ends_with("\n\n")
							&& h.literal.ends_with("\n\n")
						{
							return true;
						}
						true
					}
					_ => false,
				};
				let position = |p: comrak::nodes::LineColumn| {
					lines[p.line.saturating_sub(1)] + p.column
				};
				literal
					&& position(data.sourcepos.start).saturating_sub(1)
						< bare.len() && position(data.sourcepos.end) > bare.len()
			}) {
			return self.snippet(bare, depth, None);
		}
		// A note keeps the number the document gave it, so a reference inside
		// a snippet and the note block outside it still agree.
		let mut next_footnote =
			self.footnotes.values().copied().max().unwrap_or(0) + 1;
		for node in root.descendants() {
			if let NodeValue::FootnoteReference(f) =
				&mut node.data.borrow_mut().value
			{
				f.ix = *self.footnotes.entry(note_key(&f.name)).or_insert_with(
					|| {
						let ix = next_footnote;
						next_footnote += 1;
						ix
					},
				);
			}
		}
		let mut reader = Reader {
			source: text,
			lines,
			footnotes: std::mem::take(&mut self.footnotes),
			document_footnotes: self.document_footnotes,
			footnote_blocks: std::mem::take(&mut self.footnote_blocks),
			snippet_end: Some(bare.map_or(text.len(), str::len)),
			footnote_column: self.footnote_column,
			details_ordinal: std::mem::take(&mut self.details_ordinal),
			definitions: self.definitions,
			limits: self.limits,
			reparse_used: Rc::clone(&self.reparse_used),
			reparse_limit: self.reparse_limit,
		};
		let blocks = reader.blocks(root, depth);
		self.footnotes = reader.footnotes;
		self.footnote_blocks = reader.footnote_blocks;
		self.details_ordinal = reader.details_ordinal;
		blocks
	}

	/// The summary's rich text: its Markdown inline content, or its plain text
	/// when it is not phrasing content.
	/// The summary's rich text. GFM keeps raw HTML inside `<summary>` and
	/// does not parse Markdown, so `**Bold**` stays literal.
	fn summary_rich(
		&self,
		text: Option<&str>,
		source: &Range<usize>,
	) -> RichText {
		let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
			return RichText::new();
		};
		let mut out = match html::block(text) {
			html::Block::Paragraph(spans) => html_rich(spans, source),
			_ => RichText::new(),
		};
		if out.is_empty() {
			out.push(Inline {
				kind: InlineKind::Text(text.trim().to_string()),
				style: TextStyle::default(),
				source: source.clone(),
			});
		}
		merge_text(out)
	}
}

/// One open raw-HTML container while a sibling list is walked.
struct Frame {
	kind: FrameKind,
	blocks: Vec<Block>,
	/// The opener's HTML literal, used when the element never closes.
	raw: String,
	/// Source offset where the opener begins.
	start: usize,
}

enum FrameKind {
	Transparent {
		name: String,
	},
	Details {
		open: bool,
		ordinal: u32,
		summary: Option<String>,
	},
}

fn frame_target<'a>(
	stack: &'a mut [Frame],
	out: &'a mut Vec<Block>,
) -> &'a mut Vec<Block> {
	match stack.last_mut() {
		Some(frame) => &mut frame.blocks,
		None => out,
	}
}

fn push_block(stack: &mut [Frame], out: &mut Vec<Block>, block: Block) {
	frame_target(stack, out).push(block);
}

fn extend_blocks(
	stack: &mut [Frame],
	out: &mut Vec<Block>,
	blocks: Vec<Block>,
) {
	frame_target(stack, out).extend(blocks);
}

fn map_source(
	original: &HtmlSource,
	child: &Range<usize>,
	range: Range<usize>,
) -> Range<usize> {
	let mapped = original.range(range);
	child.start + mapped.start..child.start + mapped.end
}

fn clamp_range(source: &str, range: Range<usize>) -> Range<usize> {
	let mut start = range.start.min(source.len());
	let mut end = range.end.min(source.len());
	while !source.is_char_boundary(start) {
		start -= 1;
	}
	while !source.is_char_boundary(end) {
		end -= 1;
	}
	start..end.max(start)
}

fn html_leaf_block(
	source: &str,
	text: &str,
	range: Range<usize>,
) -> Option<Block> {
	let range = clamp_range(source, range);
	let kind = match html::block(text) {
		html::Block::Unsupported => BlockKind::Code {
			language: "HTML source".into(),
			text: text.to_string(),
		},
		html::Block::Empty => return None,
		html::Block::Rule => BlockKind::Rule,
		html::Block::Heading { level, text } => BlockKind::Heading {
			level,
			text: html_rich(text, &range),
			anchor: String::new(),
		},
		html::Block::Paragraph(text) => {
			BlockKind::Paragraph(html_rich(text, &range))
		}
	};
	Some(block_of(source, kind, range))
}

fn block_of(source: &str, kind: BlockKind, range: Range<usize>) -> Block {
	let range = clamp_range(source, range);
	let id =
		fingerprint(&(std::mem::discriminant(&kind), &source[range.clone()]));
	Block {
		id,
		content_key: semantic_key(&kind),
		source: range,
		kind,
	}
}

fn details_block_of(
	source: &str,
	open: bool,
	ordinal: u32,
	summary: RichText,
	blocks: Vec<Block>,
	range: Range<usize>,
) -> Block {
	let range = clamp_range(source, range);
	let kind = BlockKind::Details {
		open,
		ordinal,
		summary,
		blocks,
	};
	let id = fingerprint(&(
		std::mem::discriminant(&kind),
		&source[range.clone()],
		ordinal,
	));
	Block {
		id,
		content_key: semantic_key(&kind),
		source: range,
		kind,
	}
}

/// What one AST child contributes to its parent's block list.
enum Child {
	/// A finished block.
	Block(Block),
	/// Nothing readable, such as an empty HTML block.
	Skip,
	/// A container that only groups its children, which take its place.
	Flatten,
}

enum ContainerMarker {
	Indent(usize),
	Quote,
}

/// Continuation prefixes in the order the AST's containers consume them.
fn container_markers<'a>(node: &'a AstNode<'a>) -> Vec<ContainerMarker> {
	let mut markers = Vec::new();
	for parent in node.ancestors().skip(1) {
		match &parent.data.borrow().value {
			NodeValue::Item(list) => markers.push(ContainerMarker::Indent(
				list.marker_offset + list.padding,
			)),
			NodeValue::TaskItem(_) => {
				if let Some(list) = parent.parent()
					&& let NodeValue::List(list) = &list.data.borrow().value
				{
					markers.push(ContainerMarker::Indent(
						list.marker_offset + list.padding,
					));
				}
			}
			NodeValue::FootnoteDefinition(_) => {
				markers.push(ContainerMarker::Indent(4))
			}
			NodeValue::BlockQuote | NodeValue::Alert(_) => {
				markers.push(ContainerMarker::Quote)
			}
			_ => {}
		}
	}
	markers.reverse();
	markers
}

/// HTML without container prefixes, mapped to its original byte offsets.
struct HtmlSource<'a> {
	text: Cow<'a, str>,
	lines: Vec<(usize, usize)>,
}

impl<'a> HtmlSource<'a> {
	fn new(source: &'a str, markers: &[ContainerMarker]) -> Self {
		if markers.is_empty() {
			return Self {
				text: Cow::Borrowed(source),
				lines: Vec::new(),
			};
		}
		let mut text = String::with_capacity(source.len());
		let mut lines = Vec::new();
		let mut at = 0;
		for line in source.split_inclusive(['\r', '\n']) {
			let body = without_markers(line, markers);
			lines.push((text.len(), at + line.len() - body.len()));
			text.push_str(body);
			at += line.len();
		}
		Self {
			text: Cow::Owned(text),
			lines,
		}
	}

	fn range(&self, range: Range<usize>) -> Range<usize> {
		let original = |at| {
			if self.lines.is_empty() {
				return at;
			}
			let i = self.lines.partition_point(|(start, _)| *start <= at) - 1;
			let (start, source_start) = self.lines[i];
			source_start + at - start
		};
		original(range.start)..original(range.end)
	}
}

/// One line with its enclosing list indentation and quote markers removed.
fn without_markers<'a>(line: &'a str, markers: &[ContainerMarker]) -> &'a str {
	let mut rest = line;
	for marker in markers {
		let indent = rest.len() - rest.trim_start_matches(' ').len();
		if let ContainerMarker::Indent(width) = marker {
			if indent < *width {
				break;
			}
			rest = &rest[*width..];
			continue;
		}
		// Four spaces already mean code, not a marker.
		if indent > 3 {
			break;
		}
		let Some(after) = rest[indent..].strip_prefix('>') else {
			break;
		};
		rest = match after.as_bytes().first() {
			Some(b' ' | b'\t') => &after[1..],
			_ => after,
		};
	}
	rest
}

/// The byte offset of the start of every line, in the sense comrak counts
/// them. Comrak ends a line at `\n`, `\r`, and `\r\n`; a table built from
/// `\n` alone would point the source ranges at the wrong line wherever a
/// lone carriage return occurs.
fn line_starts(source: &str) -> Vec<usize> {
	let mut lines = vec![0];
	let bytes = source.as_bytes();
	let mut i = 0;
	while i < bytes.len() {
		match bytes[i] {
			b'\n' => {
				i += 1;
				lines.push(i);
			}
			b'\r' => {
				i += 1;
				if i < bytes.len() && bytes[i] == b'\n' {
					i += 1;
				}
				lines.push(i);
			}
			_ => i += 1,
		}
	}
	lines
}

/// The identity of a footnote label: comrak matches labels case-insensitively
/// and collapses whitespace, so every label map is keyed by this.
fn note_key(name: &str) -> String {
	name.split_whitespace()
		.collect::<Vec<_>>()
		.join(" ")
		.to_lowercase()
}

pub fn parse(source: impl Into<Arc<str>>) -> Document {
	let source = source.into();
	let arena = Arena::new();
	let root = parse_document(&arena, &source, &markdown_options());
	let lines = line_starts(&source);
	let footnotes: HashMap<String, u32> = root
		.descendants()
		.filter_map(|n| match &n.data.borrow().value {
			NodeValue::FootnoteReference(f) => Some((note_key(&f.name), f.ix)),
			_ => None,
		})
		.collect();
	let document_footnotes: HashSet<String> = root
		.descendants()
		.filter_map(|node| {
			if let NodeValue::FootnoteDefinition(note) =
				&node.data.borrow().value
				&& note.total_references > 0
			{
				Some(note_key(&note.name))
			} else {
				None
			}
		})
		.collect();
	let footnote_column = footnotes
		.values()
		.copied()
		.max()
		.unwrap_or(1)
		.to_string()
		.len() as u32;
	let definitions = if incremental::definition_free(&source) {
		String::new()
	} else {
		incremental::definitions(&source)
	};
	let mut reader = Reader {
		source: &source,
		lines,
		footnotes,
		document_footnotes: &document_footnotes,
		footnote_blocks: HashMap::new(),
		snippet_end: None,
		footnote_column,
		details_ordinal: 0,
		definitions: &definitions,
		limits: crate::limits::Limits::default(),
		reparse_used: Rc::new(Cell::new(0)),
		reparse_limit: source.len().saturating_mul(8).clamp(1 << 20, 64 << 20),
	};
	let mut blocks = reader.blocks(root, 0);
	let column = reader
		.footnotes
		.values()
		.copied()
		.max()
		.unwrap_or(1)
		.to_string()
		.len() as u32;
	let mut notes: Vec<_> = reader
		.footnote_blocks
		.into_iter()
		.filter(|(name, _)| {
			reader.footnotes.contains_key(name)
				|| document_footnotes.contains(name)
		})
		.collect();
	notes.sort_by(|(left, _), (right, _)| {
		(reader.footnotes.get(left), left)
			.cmp(&(reader.footnotes.get(right), right))
	});
	for (name, mut note) in notes {
		if let BlockKind::Footnote {
			label,
			column: note_column,
			..
		} = &mut note.kind
		{
			if let Some(number) = reader.footnotes.get(&name) {
				*label = number.to_string();
			}
			*note_column = column;
		}
		note.content_key = semantic_key(&note.kind);
		blocks.push(note);
	}
	incremental::relabel_headings(&mut blocks);
	Document {
		source,
		content_id: content_identity(&blocks),
		blocks,
	}
}

/// The comrak configuration every Markdown parse shares, including the body
/// of a `<details>` element.
fn markdown_options() -> Options<'static> {
	let mut options = Options::default();
	options.extension.table = true;
	// A document may open with `---` fenced YAML metadata. Comrak only splits
	// it off: it hands the block back verbatim, delimiters included, and the
	// YAML itself is read in `front_matter`.
	options.extension.front_matter_delimiter = Some("---".into());
	options.extension.strikethrough = true;
	options.extension.tasklist = true;
	options.extension.autolink = true;
	options.extension.footnotes = true;
	options.extension.alerts = true;
	options.extension.math_dollars = true;
	options.extension.math_latex = true;
	options.extension.math_code = true;
	// CommonMark's flanking rules miss emphasis that ends next to CJK text,
	// as in `**重要です。**但`, where the closing run follows punctuation.
	options.extension.cjk_friendly_emphasis = true;
	options
}

fn apply_patch(patch: &html::Patch, style: &mut TextStyle) {
	match patch {
		html::Patch::Bold => style.bold = true,
		html::Patch::Italic => style.italic = true,
		html::Patch::Strike => style.strike = true,
		html::Patch::Code => style.code = true,
		html::Patch::Superscript => style.superscript = true,
		html::Patch::Link(url) => style.link = Some(url.clone()),
		html::Patch::None => {}
	}
}

/// Map snippet bytes to original lines, allowing removed quote prefixes and synthetic breaks.
fn source_offsets(
	text: &str,
	original: &str,
	source: Range<usize>,
) -> Vec<usize> {
	let raw = &original[source.clone()];
	let mut offsets = vec![source.start; text.len() + 1];
	let mut cursor = 0;
	let lines = line_starts(text);
	for (index, &at) in lines.iter().enumerate() {
		let line =
			&text[at..lines.get(index + 1).copied().unwrap_or(text.len())];
		let content = line.trim_end_matches(['\r', '\n']);
		if !content.is_empty()
			&& let Some(found) = raw[cursor..].find(content)
		{
			cursor += found;
			for i in 0..content.len() {
				offsets[at + i] = source.start + cursor + i;
			}
			cursor += content.len();
		}
		for i in content.len()..line.len() {
			offsets[at + i] = source.start + cursor;
			if raw.as_bytes().get(cursor) == Some(&line.as_bytes()[i]) {
				cursor += 1;
			}
		}
	}
	offsets[text.len()] = source.start + cursor;
	offsets
}

fn html_rich(spans: Vec<html::Span>, source: &Range<usize>) -> RichText {
	spans
		.into_iter()
		.map(|span| Inline {
			kind: span
				.image
				.map_or_else(|| InlineKind::Text(span.text), InlineKind::Image),
			style: TextStyle {
				bold: span.style.bold,
				italic: span.style.italic,
				strike: span.style.strike,
				code: span.style.code,
				superscript: span.style.superscript,
				link: span.style.link.map(|url| url.to_string()),
				..TextStyle::default()
			},
			source: source.clone(),
		})
		.collect()
}

/// Merge neighboring runs that share a style so a dropped comment or tag does
/// not leave a double space behind.
fn merge_text(text: RichText) -> RichText {
	let mut out: RichText = Vec::with_capacity(text.len());
	for span in text {
		let InlineKind::Text(t) = &span.kind else {
			out.push(span);
			continue;
		};
		let mut merged = false;
		if let Some(last) = out.last_mut()
			&& last.style == span.style
			&& let InlineKind::Text(prev) = &mut last.kind
		{
			if prev.ends_with(char::is_whitespace)
				&& t.starts_with(char::is_whitespace)
			{
				let len = prev.trim_end().len();
				prev.truncate(len);
				prev.push(' ');
				prev.push_str(t.trim_start());
			} else {
				prev.push_str(t);
			}
			// Comrak's source positions can run backwards across siblings — a
			// paragraph that follows a link reference definition keeps the
			// definition line's columns — so taking the second span's end
			// verbatim can invert the merged range. The end only ever grows.
			last.source.end = last.source.end.max(span.source.end);
			merged = true;
		}
		if !merged {
			out.push(span);
		}
	}
	out
}
