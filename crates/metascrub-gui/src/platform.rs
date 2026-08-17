//! Things the desktop underneath tells us, and things it fails to provide.
//!
//! Kept separate from [`crate::privacy`], which is about traces the application
//! leaves behind. This module is about what it can find out and rely on.

use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;

/// The language the user's session is set to, as a bare ISO 639 code.
///
/// The Android build has picked this up for free since the day it shipped,
/// because Android resource qualifiers do it without being asked. The desktop
/// build did not do it at all: it opened in English on a Russian or Burmese
/// system and stayed there until the user found the toggle in the corner. For
/// somebody who does not read English, a language switch they cannot read is
/// not a switch.
///
/// The POSIX variables are checked in the order POSIX gives them. On Windows
/// and macOS they are normally unset, which returns `None` and leaves the
/// previous behaviour exactly as it was.
pub fn session_language() -> Option<String> {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
        let Some(value) = std::env::var_os(key) else { continue };
        let value = value.to_string_lossy();
        // LANGUAGE is a priority list, "en_AU:en". The rest are single values,
        // but splitting on ':' is harmless for them.
        let first = value.split(':').next().unwrap_or("");
        // "ru_RU.UTF-8@euro" -> "ru"
        let tag = first.split(['_', '.', '@', '-']).next().unwrap_or("").to_ascii_lowercase();
        // "C" and "POSIX" mean "no locale chosen", not "a language called C".
        if tag.is_empty() || tag == "c" || tag == "posix" {
            continue;
        }
        return Some(tag);
    }
    None
}

/// Is this the Linux container inside ChromeOS?
///
/// Only used to make an error message name the right command. Crostini is a
/// deliberately minimal Debian, so several things a desktop normally provides
/// are simply absent, and telling a Chromebook user to check their desktop
/// environment would send them looking for something that does not exist.
pub fn is_crostini() -> bool {
    Path::new("/opt/google/cros-containers").is_dir()
}

/// Can this system show a file chooser at all?
///
/// `rfd` talks to the XDG desktop portal over D-Bus. When no portal is
/// installed there is nothing to talk to, and `pick_files` returns `None`,
/// which is the same value it returns when the user presses Cancel. The
/// interface cannot tell those apart, so the button appears to do nothing and
/// the user is left tapping it.
///
/// A minimal Debian has no portal. Neither does ChromeOS Crostini, which is
/// the same thing with a different name. This is checked so the application
/// can say what happened instead of looking broken.
///
/// It is deliberately only consulted *after* a dialog has already come back
/// empty. A false negative here would hide a working button, which is worse
/// than the problem being solved, so nothing is disabled on the strength of it.
#[cfg(target_os = "linux")]
pub fn file_dialog_supported() -> bool {
    // The normal case: D-Bus starts the portal on demand from an activation
    // file, so the file being present is the portal being available.
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(home) = std::env::var_os("XDG_DATA_HOME") {
        roots.push(PathBuf::from(home));
    } else if let Some(home) = std::env::var_os("HOME") {
        roots.push(PathBuf::from(home).join(".local").join("share"));
    }
    match std::env::var_os("XDG_DATA_DIRS") {
        Some(dirs) => roots.extend(std::env::split_paths(&dirs)),
        None => roots.extend([PathBuf::from("/usr/local/share"), PathBuf::from("/usr/share")]),
    }
    for root in roots {
        if root
            .join("dbus-1")
            .join("services")
            .join("org.freedesktop.portal.Desktop.service")
            .is_file()
        {
            return true;
        }
    }

    // Some systems ship the binary and start it from the session rather than
    // through activation, so a missing service file is not proof on its own.
    for path in [
        "/usr/libexec/xdg-desktop-portal",
        "/usr/lib/xdg-desktop-portal",
        "/usr/lib64/xdg-desktop-portal",
        "/usr/bin/xdg-desktop-portal",
    ] {
        if Path::new(path).is_file() {
            return true;
        }
    }
    // Debian and its derivatives put it under a multiarch directory whose name
    // depends on the architecture, so it has to be looked for rather than named.
    if let Ok(entries) = std::fs::read_dir("/usr/lib") {
        for entry in entries.flatten() {
            if entry.path().join("xdg-desktop-portal").is_file() {
                return true;
            }
        }
    }
    false
}

/// Every other platform has a file chooser in the operating system itself.
#[cfg(not(target_os = "linux"))]
pub fn file_dialog_supported() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `session_language` reads the environment, so the parsing is tested
    /// through a helper rather than by setting variables in a threaded test
    /// binary, where another test could observe the change.
    fn tag_of(value: &str) -> Option<String> {
        let first = value.split(':').next().unwrap_or("");
        let tag = first.split(['_', '.', '@', '-']).next().unwrap_or("").to_ascii_lowercase();
        (!tag.is_empty() && tag != "c" && tag != "posix").then_some(tag)
    }

    #[test]
    fn locale_strings_reduce_to_a_language() {
        assert_eq!(tag_of("ru_RU.UTF-8"), Some("ru".into()));
        assert_eq!(tag_of("my_MM"), Some("my".into()));
        assert_eq!(tag_of("en_AU:en"), Some("en".into()));
        assert_eq!(tag_of("pt_BR.UTF-8@euro"), Some("pt".into()));
        assert_eq!(tag_of("ckb-IR"), Some("ckb".into()));
    }

    #[test]
    fn the_absence_of_a_locale_is_not_a_language() {
        assert_eq!(tag_of("C"), None);
        assert_eq!(tag_of("POSIX"), None);
        assert_eq!(tag_of(""), None);
    }

    #[test]
    fn probing_for_a_portal_does_not_panic() {
        // The answer depends on the machine; that it returns at all does not.
        let _ = file_dialog_supported();
        let _ = is_crostini();
    }
}
