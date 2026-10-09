//! Release the shell before starting a desktop reader.
use crate::cli::{LaunchOptions, Mode};
use anyhow::{Context, Result};
use std::process::{Command, Stdio};

pub(super) fn launch(args: &LaunchOptions) -> Result<bool> {
	if args.mode != Mode::Window || args.foreground || args.background_child {
		return Ok(false);
	}
	// Launch Services sends Apple Events to the original application process.
	#[cfg(target_os = "macos")]
	{
		// SAFETY: `getppid` takes no arguments and accesses no Rust memory.
		#[allow(unsafe_code)]
		if unsafe { libc::getppid() } == 1 {
			return Ok(false);
		}
	}
	let mut child = Command::new(std::env::current_exe()?);
	child
		.arg("--background-child")
		.args(std::env::args_os().skip(1));
	detach(&mut child);
	child
		.spawn()
		.context("Cannot start the background reader")?;
	Ok(true)
}

fn detach(child: &mut Command) {
	child
		.stdin(Stdio::null())
		.stdout(Stdio::null())
		.stderr(Stdio::null());
	#[cfg(unix)]
	{
		use std::os::unix::process::CommandExt;
		// SAFETY: The hook only calls async-signal-safe `setsid` and reads
		// `errno`; it allocates nothing and touches no shared Rust state.
		#[allow(unsafe_code)]
		unsafe {
			child.pre_exec(|| {
				if libc::setsid() == -1 {
					return Err(std::io::Error::last_os_error());
				}
				Ok(())
			});
		}
	}
	#[cfg(windows)]
	{
		use std::os::windows::process::CommandExt;
		use windows_sys::Win32::System::Threading::{
			CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS,
		};
		child.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
	}
}
