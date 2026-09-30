//! The deterministic single edit the `reparse` target applies to `md0`.
//! Shared by the target and the triage tools (`dump`, `repro`) so all
//! three exercise the identical edit.
//!
//! The op is chosen from `oracle::derive(data)`, so the edit is a pure
//! function of the input bytes.

/// The edit class applied to `md0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Op {
	/// Insert one alphabet line at a seed-chosen position.
	Insert,
	/// Delete a seed-chosen line.
	Delete,
	/// Replace a seed-chosen line with an alphabet line.
	Replace,
	/// Duplicate a seed-chosen line (copy inserted right after it).
	Duplicate,
	/// Keep only the first `k` lines.
	Truncate,
}

/// The edited document and a human-readable account of what happened.
pub struct Edit {
	/// The edited source (`md1`).
	pub md1: String,
	/// Which edit class was applied.
	pub kind: Op,
	/// Human-readable description, e.g. `insert line 3 <- "## heading now"`.
	pub description: String,
}

/// The 7-line alphabet a new/replacement line is drawn from.
const ALPHABET: [&str; 7] = [
	"an inserted line",
	"## heading now",
	"- a new item",
	"more **emphasis**",
	"$x^2$",
	"| a | b |",
	"```rust",
];

/// Apply one deterministic edit to `data`, chosen from its seed:
///
/// - `(seed >> 4) % 5` picks the op,
/// - `seed % 7` picks the alphabet line,
/// - `(seed >> 8)` picks the affected line/position.
///
/// The insert arm reproduces the original behavior exactly, so old repro
/// workflows stay meaningful.
pub fn apply_deterministic_edit(data: &[u8]) -> Edit {
	let seed = crate::oracle::derive(data);
	let md0 = String::from_utf8_lossy(data).into_owned();
	// `split_inclusive` keeps each line's trailing `\n` (except possibly
	// the last), so concatenation is lossless.
	let lines: Vec<&str> = md0.split_inclusive('\n').collect();
	let line = ALPHABET[seed as usize % 7];
	let at = (seed >> 8) as usize;

	// 0: insert `line` between lines `at - 1` and `at`.
	let insert = |at: usize| {
		let at = at % (lines.len() + 1);
		let before: String = lines.iter().take(at).copied().collect();
		let after: String = lines.iter().skip(at).copied().collect();
		Edit {
			kind: Op::Insert,
			description: format!("insert line {at} <- {line:?}"),
			md1: format!("{before}{line}\n{after}"),
		}
	};

	match (seed >> 4) % 5 {
		0 => insert(at),
		// 1: drop line `at`; nothing to drop in a 1-line document, fall
		// back to insert.
		1 if lines.len() > 1 => {
			let at = at % lines.len();
			let mut rest = lines.clone();
			let dropped = rest.remove(at);
			Edit {
				kind: Op::Delete,
				description: format!(
					"delete line {at} ({:?})",
					dropped.trim_end()
				),
				md1: rest.concat(),
			}
		}
		// 2: overwrite line `at` with `line`.
		2 => {
			let at = at % lines.len();
			let mut rest = lines.clone();
			rest[at] = line;
			Edit {
				kind: Op::Replace,
				description: format!("replace line {at} <- {line:?}"),
				md1: rest.concat(),
			}
		}
		// 3: re-insert a copy of line `at` right after itself.
		3 => {
			let at = at % lines.len();
			let content = lines[at].trim_end_matches('\n');
			Edit {
				kind: Op::Duplicate,
				description: format!("duplicate line {at} ({content:?})"),
				md1: format!(
					"{}{content}\n{}",
					lines[..=at].concat(),
					lines[at + 1..].concat()
				),
			}
		}
		// 4: keep only the first `at` lines (`lines.len()` keeps all).
		_ => {
			let at = at % (lines.len() + 1);
			Edit {
				kind: Op::Truncate,
				description: format!("truncate to {at} lines"),
				md1: lines[..at].concat(),
			}
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	/// Deterministic: the same bytes always produce the same edit.
	#[test]
	fn deterministic() {
		let data = b"# title\n\nsome paragraph\n- item\n";
		let a = apply_deterministic_edit(data);
		let b = apply_deterministic_edit(data);
		assert_eq!(a.md1, b.md1);
		assert_eq!(a.kind, b.kind);
		assert_eq!(a.description, b.description);
	}

	/// Across 64 near-identical inputs, several op kinds must occur, so
	/// the non-insert windows actually get exercised.
	#[test]
	fn ops_vary() {
		let mut kinds = std::collections::BTreeSet::new();
		for i in 0..64u8 {
			let mut data = b"# title\n\nsome paragraph\n- item\n".to_vec();
			data[0] = i;
			kinds.insert(apply_deterministic_edit(&data).kind);
		}
		assert!(kinds.len() >= 3, "only {kinds:?} occurred");
	}
}
