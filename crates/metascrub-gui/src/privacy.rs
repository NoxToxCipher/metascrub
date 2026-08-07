//! Traces the application itself leaves behind.
//!
//! The tool removes information from files. It should not replace it with a
//! record of which files were cleaned, and by default the operating system
//! makes exactly that record.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Windows adds every file opened or saved through the common file dialog to
/// Recent Items, Quick Access and the application's jump list. The dialog can
/// be told not to, with `FOS_DONTADDTORECENT`, but `rfd` does not set that flag
/// and does not expose a way to.
///
/// For this application that record is worse than the metadata it removes: a
/// list, in a well-known folder, of precisely which photographs somebody
/// thought were sensitive enough to clean. In the situation this tool is most
/// useful, that list is the thing that gets found.
///
/// So after a dialog closes, the shortcuts it just created are deleted.
///
/// **This is mitigation, not elimination.** It clears the `Recent` folder
/// entries, which is what Explorer's "Recent files" and the Start menu read.
/// Jump-list databases (`AutomaticDestinations-ms`) are an undocumented binary
/// format and are not touched. Drag and drop creates none of this, and is the
/// path the interface recommends.
///
/// Only shortcuts created in the last two minutes are removed, so an unrelated
/// entry for a file of the same name is left alone.
pub fn forget_recent(paths: &[PathBuf]) {
    let Some(recent) = recent_dir() else { return };
    let cutoff = SystemTime::now() - Duration::from_secs(120);

    for path in paths {
        let Some(stem) = path.file_name().and_then(|s| s.to_str()) else { continue };
        // Explorer names the shortcut after the full file name, plus .lnk.
        let link = recent.join(format!("{stem}.lnk"));
        if was_created_after(&link, cutoff) {
            let _ = std::fs::remove_file(&link);
        }
    }
}

fn recent_dir() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let dir = PathBuf::from(appdata).join("Microsoft").join("Windows").join("Recent");
    dir.is_dir().then_some(dir)
}

fn was_created_after(path: &Path, cutoff: SystemTime) -> bool {
    std::fs::metadata(path).and_then(|m| m.modified()).map(|t| t >= cutoff).unwrap_or(false)
}

/// Replace the panic handler so a crash cannot write a memory dump.
///
/// This crate forbids `unsafe`, so the realistic crash is a Rust panic rather
/// than an access violation. The default behaviour for `panic = "abort"` is to
/// call `abort()`, which on Windows hands the process to Windows Error
/// Reporting: a dump file on disk, and possibly a report sent to Microsoft.
///
/// At the moment of a crash this process is holding a decoded photograph and
/// whatever was extracted from it, which may include the coordinates of
/// somebody's home. That is not something to write to disk in a file nobody
/// knows exists.
///
/// Exiting directly skips the dump. The cost is that a genuine bug leaves less
/// for a developer to work with, which is the right trade for this application.
pub fn suppress_crash_dumps() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // Location only. The payload of a panic can quote file contents.
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        eprintln!("metascrub stopped unexpectedly at {location}");
        eprintln!("No crash dump was written, because this process handles private files.");
        let _ = &default_hook;
        std::process::exit(101);
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_cleanup_ignores_files_it_did_not_create() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("someone-elses.lnk");
        std::fs::write(&old, b"x").unwrap();
        // A cutoff in the future makes every existing file look old.
        let future = SystemTime::now() + Duration::from_secs(600);
        assert!(!was_created_after(&old, future));
    }

    #[test]
    fn recent_cleanup_recognises_a_fresh_file() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("ours.lnk");
        std::fs::write(&fresh, b"x").unwrap();
        let cutoff = SystemTime::now() - Duration::from_secs(120);
        assert!(was_created_after(&fresh, cutoff));
    }

    #[test]
    fn missing_files_are_not_an_error() {
        let cutoff = SystemTime::now() - Duration::from_secs(120);
        assert!(!was_created_after(Path::new("no-such-file-here.lnk"), cutoff));
        forget_recent(&[PathBuf::from("also-not-real.jpg")]); // must not panic
    }
}
