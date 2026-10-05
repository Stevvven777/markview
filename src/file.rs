//! Bounded, read-only local file loading.
use anyhow::{Context, Result, bail};
use std::{
	fs,
	io::{self, Read},
	path::Path,
};
pub const MAX_FILE_BYTES: u64 = 32 * 1024 * 1024;

/// Opens `path` for reading, refusing anything that is not a regular file.
///
/// The open is the one step no cancellation token reaches: a FIFO with no
/// writer blocks a pathname open forever, and a thread already inside it never
/// observes a shutdown. Opening non-blockingly keeps that wait out of the
/// process, and the type then comes from the handle the open returned, so a
/// special file is refused before a byte is read from it. Reading the type
/// after the open is what makes it a race the caller cannot lose: a path
/// swapped for a FIFO between check and open cannot reintroduce a wait, because
/// the open itself does not wait.
pub(crate) fn open_regular(path: &Path) -> Result<fs::File> {
	let file = open_nonblocking(path)
		.with_context(|| format!("Cannot open {}", path.display()))?;
	if !file.metadata()?.is_file() {
		bail!("Not a regular file: {}", path.display());
	}
	Ok(file)
}

#[cfg(unix)]
fn open_nonblocking(path: &Path) -> io::Result<fs::File> {
	use std::os::unix::fs::OpenOptionsExt;
	// `O_NONBLOCK` has no effect on a regular file; it only decides whether the
	// open waits. Standard Rust opens already set `O_CLOEXEC`, so no descriptor
	// escapes into a child.
	fs::OpenOptions::new()
		.read(true)
		.custom_flags(libc::O_NONBLOCK)
		.open(path)
}

#[cfg(not(unix))]
fn open_nonblocking(path: &Path) -> io::Result<fs::File> {
	// Windows has no portable non-blocking open, so opening a device or a pipe
	// can still wait there; the type check still refuses it once it returns.
	fs::File::open(path)
}

pub fn read_document(path: &Path) -> Result<String> {
	let mut file = open_regular(path)?;
	let before = file.metadata()?;
	if before.len() > MAX_FILE_BYTES {
		bail!("MVP file size limit is 32 MiB");
	}
	let mut bytes = Vec::with_capacity(before.len() as usize);
	Read::by_ref(&mut file)
		.take(MAX_FILE_BYTES + 1)
		.read_to_end(&mut bytes)?;
	if bytes.len() as u64 > MAX_FILE_BYTES {
		bail!("MVP file size limit is 32 MiB");
	}
	let after = file.metadata()?;
	if before.len() != after.len()
		|| before.modified().ok() != after.modified().ok()
	{
		bail!("File is still being written; waiting for the next update");
	}
	let text = String::from_utf8(bytes)
		.context("File is not complete UTF-8; waiting for a valid update")?;
	Ok(match text.strip_prefix('\u{feff}') {
		Some(text) => text.to_string(),
		None => text,
	})
}

#[cfg(test)]
mod tests {
	#[cfg(unix)]
	use std::time::Duration;

	use super::*;

	/// Leaves a real FIFO, the special file an open would wait on.
	#[cfg(unix)]
	fn fifo(path: &Path) {
		let status = std::process::Command::new("mkfifo")
			.arg(path)
			.status()
			.expect("run mkfifo");
		assert!(status.success(), "mkfifo {} failed", path.display());
	}

	/// Runs `read` off the test thread, so a regression fails the test instead
	/// of hanging the suite.
	#[cfg(unix)]
	fn within_a_second(read: impl FnOnce() -> Result<String> + Send + 'static) {
		let (send, recv) = std::sync::mpsc::channel();
		std::thread::spawn(move || {
			let _ = send.send(read());
		});
		let result = recv
			.recv_timeout(Duration::from_secs(5))
			.expect("reading a FIFO blocked instead of being refused");
		assert!(result.is_err(), "a FIFO was read as a document");
	}

	#[cfg(unix)]
	#[test]
	fn a_fifo_document_is_refused_without_waiting() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("note.md");
		fifo(&path);
		within_a_second(move || read_document(&path));
	}

	#[test]
	fn a_directory_is_refused_as_a_document() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("folder.md");
		std::fs::create_dir(&path).unwrap();
		// Windows cannot open a directory without `FILE_FLAG_BACKUP_SEMANTICS`,
		// so it refuses before the type check; both refusals are correct.
		assert!(read_document(&path).is_err());
	}

	#[test]
	fn a_regular_document_still_reads() {
		let dir = tempfile::tempdir().unwrap();
		let path = dir.path().join("note.md");
		fs::write(&path, "\u{feff}# Title\n").unwrap();
		assert_eq!(read_document(&path).unwrap(), "# Title\n");
	}
}
