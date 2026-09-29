//! Structure-aware mutation (F3).
//!
//! Pure `Arbitrary`/byte mutation grows coverage slowly on line-oriented
//! formats. These mutators edit whole lines and markers of a document: they
//! change heading levels, list markers, fence languages, table columns and
//! math delimiters, and insert, duplicate, move or delete whole lines drawn
//! from a structural alphabet. A quarter of the calls defer to libFuzzer's
//! byte mutator, which still catches encoding-level bugs.

use libfuzzer_sys::fuzzer_mutate;

/// SplitMix64: small, fast, deterministic per `seed`.
pub struct Rng(u64);

impl Rng {
	pub fn new(seed: u32) -> Self {
		// A zero state would stay zero; the offset guarantees forward progress.
		Self(u64::from(seed).wrapping_add(0x9E3779B97F4A7C15))
	}
	pub fn u32(&mut self) -> u32 {
		self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
		let mut z = self.0;
		z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
		z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
		(z ^ (z >> 31)) as u32
	}
	pub fn range(&mut self, n: usize) -> usize {
		if n == 0 {
			return 0;
		}
		self.u32() as usize % n
	}
	pub fn chance(&mut self, numerator: usize, denominator: usize) -> bool {
		self.range(denominator) < numerator
	}
}

/// Structural one-liners the Markdown mutator can insert at any position.
const MD_LINES: &[&str] = &[
	"---",
	"### Heading three",
	"#### Deeper",
	"- item",
	"* other item",
	"1. numbered",
	"   - nested",
	"1. ordered again",
	"```",
	"```rust",
	"~~~yaml",
	"let x = 1 + 1; // code",
	"| a | b | c |",
	"|---|:-:|---:|",
	"| 1 | two | **three** |",
	"$e = mc^2$",
	"$$\\int_0^1 x^2\\,dx$$",
	"![alt](https://example.com/a.png){width=120}",
	"[link](https://example.com/doc#sec)",
	"[^1]: a footnote body",
	"see [^1] and the text",
	"<details open>",
	"<summary>the summary line</summary>",
	"</details>",
	"<br>",
	"**bold** __ital__ ~~gone~~ `code`",
	"中文段落 with CJK text mixed in",
	"\ttab-indented line",
	"a long unbroken word wordwordword wordwordword wordwordword",
	"  ",
	"",
];

/// Structural one-liners the MVSS mutator can insert.
const MVSS_LINES: &[&str] = &[
	"format_version = 2",
	"version = 3",
	"targets = [\"ui\"]",
	"[fontdef]",
	"id = \"F\"",
	"lookfor = [\"F\", \"Fallback\"]",
	"type = \"cjk\"",
	"[[font-family]]",
	"name = \"DL\"",
	"[meta]",
	"name = \"Theme\"",
	"description = \"d\"",
	"[page]",
	"paper = \"a4\"",
	"margin = [18.0]",
	"[page.header]",
	"rule_width = 1.0",
	"rule_color = \"#FF0000\"",
	"[[rule]]",
	"conditions = [\"body\"]",
	"font_size = 1.1",
	"color = \"#010203\"",
	"padding = [1.0, 2.0, 3.0, 4.0]",
	"border_width = 0.5",
	"border_edges = [0.0, 1.0, 2.0, 3.0]",
	"corner_radii = [1.0, 2.0, 3.0, 4.0]",
	"[[rule]]",
	"conditions = [\"heading\"]",
	"space_before = 0.5",
	"shape = [{ symbol = \"•\" }]",
	"[svg]",
	"generic_families = [{ family = \"serif\", candidates = [\"F\"] }]",
	"[mermaid]",
	"families = [\"F\"]",
];

/// Structure-aware Markdown mutation: line and marker edits over the input,
/// with a byte-mutation fallback.
pub fn markdown(
	data: &mut [u8],
	size: usize,
	max_size: usize,
	seed: u32,
) -> usize {
	let size = size.min(max_size);
	let mut rng = Rng::new(seed);
	if size == 0 || rng.chance(1, 4) {
		return fuzzer_mutate(data, size, max_size);
	}
	let buf = &mut data[..size];
	let mut lines: Vec<Vec<u8>> =
		buf.split(|b| *b == b'\n').map(Vec::from).collect();
	if lines.is_empty() {
		lines.push(Vec::new());
	}
	match rng.range(6) {
		// Rewrite one line's structural marker.
		0 => {
			let i = rng.range(lines.len());
			alter_md_line(&mut rng, &mut lines[i]);
		}
		// Insert a structural line.
		1 => {
			let new = if rng.chance(1, 2) {
				MD_LINES[rng.range(MD_LINES.len())].as_bytes().to_vec()
			} else {
				lines[rng.range(lines.len())].clone()
			};
			lines.insert(rng.range(lines.len() + 1), new);
		}
		// Delete a line.
		2 => {
			lines.remove(rng.range(lines.len()));
		}
		// Move a line.
		3 => {
			let from = rng.range(lines.len());
			let line = lines.remove(from);
			lines.insert(rng.range(lines.len() + 1), line);
		}
		// Byte-perturb one line's content.
		4 => {
			let i = rng.range(lines.len());
			let line = &mut lines[i];
			perturb(&mut rng, line);
		}
		// Change one line's indentation.
		_ => {
			let i = rng.range(lines.len());
			let line = &mut lines[i];
			let stripped = line
				.iter()
				.position(|b| !b.is_ascii_whitespace())
				.unwrap_or(line.len());
			line.drain(..stripped);
			line.splice(
				0..0,
				"  ".repeat(rng.range(4)).chars().map(|c| c as u8),
			);
		}
	}
	let mut out: Vec<u8> = Vec::with_capacity(size);
	for (i, line) in lines.iter().enumerate() {
		if i > 0 {
			out.push(b'\n');
		}
		out.extend_from_slice(line);
		if out.len() >= max_size {
			break;
		}
	}
	out.truncate(max_size);
	data[..out.len()].copy_from_slice(&out);
	out.len()
}

/// Marker-level rewrites for one Markdown line.
fn alter_md_line(rng: &mut Rng, line: &mut Vec<u8>) {
	let stripped = line
		.iter()
		.position(|b| !b.is_ascii_whitespace())
		.unwrap_or(line.len());
	let indent: Vec<u8> = line[..stripped].to_vec();
	let mut body: Vec<u8> = line.split_off(stripped);
	let head = |n: usize| body.get(..n).unwrap_or(&[]);
	let is_list = head(1)
		.first()
		.is_some_and(|c| *c == b'-' || *c == b'*' || *c == b'+')
		|| (head(2).last().is_some_and(|c| *c == b'.')
			&& head(2).first().is_some_and(|c| c.is_ascii_digit()));
	match rng.range(8) {
		// Heading: set a new level.
		0 if head(1) == b"#" => {
			let level = 1 + rng.range(6);
			while body.len() > level {
				body.pop();
			}
			while body.len() < level {
				body.push(b'#');
			}
			if body.get(level) != Some(&b' ') {
				body.insert(level, b' ');
			}
			while body.len() < level + 1 {
				body.push(b'x');
			}
		}
		// Turn a plain line into a heading.
		1 if head(1) != b"#" && !body.is_empty() => {
			let level = 1 + rng.range(3);
			body.splice(0..0, vec![b'#'; level]);
			if body.get(level) != Some(&b' ') {
				body.insert(level, b' ');
			}
		}
		// List marker: cycle between -, *, + and ordered.
		2 if is_list => {
			let is_ordered = head(2).last() == Some(&b'.')
				&& head(2).first().is_some_and(|c| c.is_ascii_digit());
			let marker: &[u8] = if is_ordered {
				if rng.chance(1, 2) { b"* " } else { b"2. " }
			} else {
				match rng.range(3) {
					0 => b"- ",
					1 => b"* ",
					_ => b"1. ",
				}
			};
			// Drop the current marker up to its space, keep the item text.
			let rest_start = body
				.iter()
				.position(|b| *b == b' ')
				.map(|i| i + 1)
				.unwrap_or(0);
			let rest: Vec<u8> = body[rest_start..].to_vec();
			body.clear();
			body.extend_from_slice(marker);
			body.extend_from_slice(&rest);
		}
		// Code fence: change the fence kind or strip the language.
		3 if head(3) == b"```" || head(3) == b"~~~" => {
			// The language is alphanumeric; a pure ```lang line drops the
			// whole remainder.
			let rest_start = match body
				.iter()
				.skip(3)
				.position(|b| !b.is_ascii_alphanumeric())
			{
				Some(i) => 3 + i,
				None => body.len(),
			};
			let rest: Vec<u8> = body[rest_start..].to_vec();
			body.clear();
			let fence = if rng.chance(1, 2) { "~~~" } else { "```" };
			let lang = if rng.chance(3, 4) {
				["", "rust", "python", "js", "go", "yaml", "c"][rng.range(7)]
			} else {
				""
			};
			body.extend_from_slice(fence.as_bytes());
			body.extend_from_slice(lang.as_bytes());
			body.extend_from_slice(&rest);
		}
		// Table row: add or drop a cell.
		4 => {
			if body.first() == Some(&b'|') {
				if rng.chance(1, 2) {
					body.extend_from_slice(b" | cell");
				} else if let Some(pos) = body.iter().rposition(|b| *b == b'|')
					&& body.len() > 2
				{
					let from = pos.saturating_sub(3).min(pos);
					body.drain(from..=pos);
				}
			} else {
				body.insert(0, b'|');
				body.push(b'|');
			}
		}
		// Math: toggle the $ delimiters.
		5 if head(1) == b"$" || rng.chance(1, 3) => {
			if body.first() == Some(&b'$') {
				while body.first() == Some(&b'$') {
					body.remove(0);
				}
				while body.last() == Some(&b'$') {
					body.pop();
				}
			} else if !body.is_empty() {
				body.push(b'$');
				body.insert(0, b'$');
			}
		}
		// Front matter: toggle the leading `---`.
		6 if head(3) == b"---" => {
			body.drain(..3.min(body.len()));
		}
		_ => perturb(rng, &mut body),
	}
	line.clear();
	line.extend_from_slice(&indent);
	line.extend_from_slice(&body);
}

/// MVSS is line-oriented TOML, so the mutator edits lines and values.
pub fn mvss(data: &mut [u8], size: usize, max_size: usize, seed: u32) -> usize {
	let size = size.min(max_size);
	let mut rng = Rng::new(seed);
	if size == 0 || rng.chance(1, 4) {
		return fuzzer_mutate(data, size, max_size);
	}
	let buf = &mut data[..size];
	let mut lines: Vec<Vec<u8>> =
		buf.split(|b| *b == b'\n').map(Vec::from).collect();
	if lines.is_empty() {
		lines.push(Vec::new());
	}
	match rng.range(5) {
		// Rewrite one line's value.
		0 => {
			let i = rng.range(lines.len());
			let line = &mut lines[i];
			rewrite_mvss_line(&mut rng, line);
		}
		// Insert a structural line.
		1 => {
			let new = if rng.chance(1, 2) {
				MVSS_LINES[rng.range(MVSS_LINES.len())].as_bytes().to_vec()
			} else {
				lines[rng.range(lines.len())].clone()
			};
			lines.insert(rng.range(lines.len() + 1), new);
		}
		2 => {
			lines.remove(rng.range(lines.len()));
		}
		3 => {
			let from = rng.range(lines.len());
			let line = lines.remove(from);
			lines.insert(rng.range(lines.len() + 1), line);
		}
		_ => {
			let i = rng.range(lines.len());
			let line = &mut lines[i];
			perturb(&mut rng, line);
		}
	}
	let mut out: Vec<u8> = Vec::with_capacity(size);
	for (i, line) in lines.iter().enumerate() {
		if i > 0 {
			out.push(b'\n');
		}
		out.extend_from_slice(line);
		if out.len() >= max_size {
			break;
		}
	}
	out.truncate(max_size);
	data[..out.len()].copy_from_slice(&out);
	out.len()
}

/// Change the value side of a `key = value` line, or the name of a
/// `[section]` header, to a structurally valid-looking alternative.
fn rewrite_mvss_line(rng: &mut Rng, line: &mut Vec<u8>) {
	let text = std::str::from_utf8(line).unwrap_or_default();
	if let Some((key, rest)) = text.split_once('=')
		&& !key.trim().is_empty()
		&& rest.starts_with(|c: char| !c.is_whitespace() || true)
	{
		let value = match rng.range(5) {
			0 => format!("\"{}\"", key.trim().chars().next().unwrap_or('x')),
			1 => format!("{}", rng.range(100000) as i64),
			2 => format!("{:.1}", rng.range(1000) as f64 / 10.0),
			3 => if rng.chance(1, 2) { "true" } else { "false" }.to_string(),
			_ => format!(
				"[{}, {}, {}]",
				rng.range(9),
				rng.range(9),
				rng.range(9)
			),
		};
		*line = format!("{key} = {value}").into_bytes();
	} else if text.trim_start().starts_with('[') {
		let name = [
			"fontdef",
			"meta",
			"page",
			"svg",
			"mermaid",
			"rule",
			"font-family",
		][rng.range(7)];
		let double = rng.chance(1, 3);
		*line = format!(
			"{}[{}]{}",
			if double { "[" } else { "" },
			name,
			if double { "]" } else { "" }
		)
		.into_bytes();
	} else {
		perturb(rng, line);
	}
}

/// Small in-place byte edits: flip, insert, delete, rotate.
fn perturb(rng: &mut Rng, line: &mut Vec<u8>) {
	if line.is_empty() {
		line.push(b"ab c~"[rng.range(5)]);
		return;
	}
	match rng.range(4) {
		0 => {
			let i = rng.range(line.len());
			line[i] = rng.u32() as u8;
		}
		1 => {
			line.insert(rng.range(line.len() + 1), rng.u32() as u8);
		}
		2 => {
			line.remove(rng.range(line.len()));
		}
		_ => {
			let k = rng.range(line.len());
			line.rotate_left(k);
		}
	}
}
