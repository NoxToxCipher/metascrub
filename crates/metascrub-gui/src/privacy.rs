//! Traces the application itself leaves behind.
//!
//! The tool removes information from files. It should not replace it with a
//! record of which files were cleaned, and by default the operating system
//! makes exactly that record.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// How recently a trace must have appeared for us to assume it is ours.
///
/// Long enough to cover a slow dialog and a large file, short enough that an
/// unrelated entry from earlier in the day is left alone.
const OURS_IF_NEWER_THAN: Duration = Duration::from_secs(120);

/// Delete the record the operating system just made of which files were opened.
///
/// Every desktop keeps one, in a different place and a different format, so
/// each platform gets its own implementation below. They share a rule: only
/// remove a trace that appeared in the last two minutes and names a file this
/// session actually handled.
pub fn forget_recent(paths: &[PathBuf]) {
    windows_recent(paths);
    #[cfg(target_os = "linux")]
    xdg::forget_recent(paths);
}

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
///
/// Not compiled out on other platforms, because the check is a directory lookup
/// that fails immediately when `%APPDATA%` is unset, and keeping it building
/// everywhere keeps its tests running everywhere.
fn windows_recent(paths: &[PathBuf]) {
    let Some(recent) = recent_dir() else { return };
    let cutoff = SystemTime::now() - OURS_IF_NEWER_THAN;

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

/// The same leak, on Linux.
///
/// This was missing for a long time, and the omission was not a small one. The
/// Windows note above describes the record as worse than the metadata the tool
/// removes. All of that is true on Linux, and the file is easier to read: no
/// binary shell-link format, no undocumented jump-list database, just XML in a
/// predictable place that any text editor opens.
///
/// `rfd` reaches the file chooser through the XDG desktop portal, and the
/// portal's own backend, running in its own process, is the thing that records
/// the file. Nothing this application passes to `rfd` can prevent that, because
/// the recording happens on the other side of a D-Bus call, in a program we do
/// not control and cannot pass a flag to.
///
/// Two stores are cleaned:
///
/// - `recently-used.xbel`, written by GTK, which is what GNOME, Xfce, Cinnamon
///   and the GTK file chooser itself all read.
/// - `RecentDocuments/*.desktop`, written by KDE.
///
/// **This is mitigation, not elimination**, exactly as on Windows. A desktop
/// that has already read the file into a running process keeps its copy until
/// it next writes one out. Tracker, Zeitgeist, Baloo and KDE's activity manager
/// keep their own indexes and are not touched. Drag and drop creates none of
/// this, and remains the path the interface recommends.
#[cfg(target_os = "linux")]
mod xdg {
    use super::{was_created_after, OURS_IF_NEWER_THAN};
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;
    use std::path::{Path, PathBuf};
    use std::time::SystemTime;

    pub fn forget_recent(paths: &[PathBuf]) {
        let cutoff = SystemTime::now() - OURS_IF_NEWER_THAN;
        let Some(data) = data_home() else { return };
        clean_xbel(&data.join("recently-used.xbel"), paths, cutoff);
        clean_kde(&data.join("RecentDocuments"), paths, cutoff);
    }

    fn data_home() -> Option<PathBuf> {
        if let Some(dir) = std::env::var_os("XDG_DATA_HOME") {
            let dir = PathBuf::from(dir);
            if dir.is_absolute() {
                return Some(dir);
            }
        }
        Some(PathBuf::from(std::env::var_os("HOME")?).join(".local").join("share"))
    }

    /// Drop our bookmarks from GTK's recent list.
    ///
    /// The file is only touched if it was written in the last two minutes,
    /// which is the same "is this ours" test the Windows path uses, and it
    /// means an untouched machine's file is never rewritten at all.
    fn clean_xbel(file: &Path, paths: &[PathBuf], cutoff: SystemTime) {
        if !was_created_after(file, cutoff) {
            return;
        }
        let Ok(xml) = std::fs::read_to_string(file) else { return };
        let Some(cleaned) = strip_bookmarks(&xml, paths) else { return };
        // Same atomic write the cleaned photographs get: a temporary file in
        // the same directory, fsync, rename. A half-written recent list would
        // lose the user's unrelated history, which is not ours to break.
        let _ = metascrub::write_atomic(file, cleaned.as_bytes());
    }

    /// KDE writes one small desktop file per document, named after it.
    fn clean_kde(dir: &Path, paths: &[PathBuf], cutoff: SystemTime) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension() != Some(OsStr::new("desktop")) || !was_created_after(&path, cutoff)
            {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            // The document is named on a URL= line, percent-encoded the same
            // way the xbel hrefs are.
            let names_ours = text.lines().filter_map(|l| l.split_once('=')).any(|(key, value)| {
                key.starts_with("URL") && paths.iter().any(|p| uri_is(value.trim(), p))
            });
            if names_ours {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    /// Remove every `<bookmark>` element whose href names one of `paths`.
    ///
    /// Returns `None` when nothing matched, so an unaffected file is left
    /// alone rather than rewritten with identical contents.
    ///
    /// This walks the text rather than parsing the XML. That is a deliberate
    /// limit and not laziness: the file is machine-generated with a shape that
    /// has not changed in twenty years, and a real parser would mean a new
    /// dependency processing a file in the user's home directory, to delete
    /// three lines from it. What it must never do is mangle the rest of the
    /// file, so it copies every byte it does not positively identify.
    fn strip_bookmarks(xml: &str, paths: &[PathBuf]) -> Option<String> {
        let bytes = xml.as_bytes();
        let mut out = String::with_capacity(xml.len());
        let mut cursor = 0usize;
        let mut removed = 0usize;

        while let Some(rel) = xml[cursor..].find("<bookmark") {
            let start = cursor + rel;
            // "<bookmark:applications>" also starts with "<bookmark". Only a
            // space, a slash or a closing angle means the element itself.
            let after = bytes.get(start + "<bookmark".len()).copied();
            if !matches!(after, Some(b' ') | Some(b'\t') | Some(b'\n') | Some(b'/') | Some(b'>')) {
                out.push_str(&xml[cursor..start + "<bookmark".len()]);
                cursor = start + "<bookmark".len();
                continue;
            }
            let Some(open_end) = end_of_tag(xml, start) else { break };
            // "<bookmark ... />" is the whole element. Testing for a trailing
            // '/' after trimming would never fire, because the slice ends at
            // the '>' itself, and the element would then swallow everything up
            // to the next "</bookmark>" -- taking unrelated entries with it.
            let self_closing = xml[start..open_end].ends_with("/>");
            let element_end = if self_closing {
                open_end
            } else {
                match xml[open_end..].find("</bookmark>") {
                    Some(rel) => open_end + rel + "</bookmark>".len(),
                    None => break, // truncated file: copy the rest untouched
                }
            };

            let ours = href_of(&xml[start..open_end])
                .is_some_and(|href| paths.iter().any(|p| uri_is(&href, p)));
            if ours {
                // Take the indentation in front of the element and the newline
                // behind it, so removing an entry does not leave a blank line
                // where one was never there before.
                let keep_to =
                    xml[cursor..start].rfind('\n').map(|i| cursor + i + 1).unwrap_or(start);
                out.push_str(&xml[cursor..keep_to]);
                let mut resume = element_end;
                if xml[resume..].starts_with('\n') {
                    resume += 1;
                }
                cursor = resume;
                removed += 1;
            } else {
                out.push_str(&xml[cursor..element_end]);
                cursor = element_end;
            }
        }
        (removed > 0).then(|| {
            out.push_str(&xml[cursor..]);
            out
        })
    }

    /// Index just past the `>` that closes the tag starting at `start`,
    /// ignoring any `>` inside a quoted attribute value.
    fn end_of_tag(xml: &str, start: usize) -> Option<usize> {
        let mut quote: Option<u8> = None;
        for (offset, byte) in xml.as_bytes()[start..].iter().enumerate() {
            match (quote, byte) {
                (Some(q), b) if *b == q => quote = None,
                (None, b'"') => quote = Some(b'"'),
                (None, b'\'') => quote = Some(b'\''),
                (None, b'>') => return Some(start + offset + 1),
                _ => {}
            }
        }
        None
    }

    fn href_of(tag: &str) -> Option<String> {
        let rest = tag.split_once("href")?.1.trim_start().strip_prefix('=')?.trim_start();
        let quote = rest.chars().next()?;
        if quote != '"' && quote != '\'' {
            return None;
        }
        let value = rest[quote.len_utf8()..].split(quote).next()?;
        Some(unescape_xml(value))
    }

    fn unescape_xml(value: &str) -> String {
        // Ordered so the ampersand is last: doing it first would turn a literal
        // "&amp;lt;" into "<".
        value
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
            .replace("&apos;", "'")
            .replace("&amp;", "&")
    }

    /// Does this `file://` URI name this path?
    ///
    /// Compared after percent-decoding, as bytes. A Linux path is bytes, not
    /// text, so a file whose name is not valid UTF-8 still matches instead of
    /// quietly escaping the cleanup.
    fn uri_is(uri: &str, path: &Path) -> bool {
        let Some(encoded) = uri.strip_prefix("file://") else { return false };
        // "file:///path" after the prefix is "/path"; "file://host/path" is not
        // something a file chooser produces for a local file.
        percent_decode(encoded) == path.as_os_str().as_bytes()
    }

    fn percent_decode(text: &str) -> Vec<u8> {
        let bytes = text.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'%' && i + 2 < bytes.len() {
                if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                    out.push(hi * 16 + lo);
                    i += 3;
                    continue;
                }
            }
            out.push(bytes[i]);
            i += 1;
        }
        out
    }

    fn hex(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const FILE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0">
  <bookmark href="file:///home/j/holiday.jpg" added="2026-08-17T09:00:00Z">
    <info><metadata owner="http://freedesktop.org">
      <bookmark:applications>
        <bookmark:application name="metascrub" count="1"/>
      </bookmark:applications>
    </metadata></info>
  </bookmark>
  <bookmark href="file:///home/j/taxes.pdf" added="2026-08-17T09:01:00Z"/>
  <bookmark href="file:///home/j/unrelated.odt" added="2026-08-17T09:02:00Z">
    <info/>
  </bookmark>
</xbel>
"#;

        #[test]
        fn removes_only_the_named_file() {
            let out = strip_bookmarks(FILE, &[PathBuf::from("/home/j/holiday.jpg")]).unwrap();
            assert!(!out.contains("holiday.jpg"));
            assert!(out.contains("taxes.pdf"));
            assert!(out.contains("unrelated.odt"));
            // The nested application element must not have confused the scan.
            assert!(out.contains("<xbel"), "the document was destroyed");
            assert!(out.trim_end().ends_with("</xbel>"));
        }

        #[test]
        fn removes_a_self_closing_entry() {
            let out = strip_bookmarks(FILE, &[PathBuf::from("/home/j/taxes.pdf")]).unwrap();
            assert!(!out.contains("taxes.pdf"));
            assert!(out.contains("holiday.jpg"));
            assert!(out.contains("unrelated.odt"));
        }

        #[test]
        fn removes_several_at_once() {
            let out = strip_bookmarks(
                FILE,
                &[PathBuf::from("/home/j/holiday.jpg"), PathBuf::from("/home/j/unrelated.odt")],
            )
            .unwrap();
            assert!(out.contains("taxes.pdf"));
            assert!(!out.contains("holiday.jpg"));
            assert!(!out.contains("unrelated.odt"));
        }

        #[test]
        fn a_file_with_nothing_of_ours_is_not_rewritten() {
            assert!(strip_bookmarks(FILE, &[PathBuf::from("/home/j/never-seen.png")]).is_none());
            assert!(strip_bookmarks(FILE, &[]).is_none());
        }

        #[test]
        fn matches_through_percent_encoding() {
            let xml = r#"<xbel>
  <bookmark href="file:///home/j/my%20holiday%20%282026%29.jpg" added="x"/>
</xbel>"#;
            let out =
                strip_bookmarks(xml, &[PathBuf::from("/home/j/my holiday (2026).jpg")]).unwrap();
            assert!(!out.contains("bookmark"));
        }

        #[test]
        fn matches_through_xml_escaping() {
            let xml = r#"<xbel>
  <bookmark href="file:///home/j/this&amp;that.jpg" added="x"/>
</xbel>"#;
            let out = strip_bookmarks(xml, &[PathBuf::from("/home/j/this&that.jpg")]).unwrap();
            assert!(!out.contains("bookmark"));
        }

        #[test]
        fn a_similar_name_is_not_a_match() {
            // Prefix, suffix and different-directory near-misses must all stay.
            for near in ["/home/j/holiday.jpeg", "/home/j/holiday.jp", "/other/holiday.jpg"] {
                assert!(
                    strip_bookmarks(FILE, &[PathBuf::from(near)]).is_none(),
                    "{near} should not have matched"
                );
            }
        }

        #[test]
        fn a_truncated_file_is_left_alone_rather_than_half_written() {
            let cut = r#"<xbel>
  <bookmark href="file:///home/j/holiday.jpg" added="x">
    <info>"#;
            assert!(strip_bookmarks(cut, &[PathBuf::from("/home/j/holiday.jpg")]).is_none());
        }

        #[test]
        fn quoted_angle_brackets_do_not_end_the_tag_early() {
            let xml = r#"<xbel>
  <bookmark href="file:///home/j/a%3Eb.jpg" title="a > b" added="x"/>
</xbel>"#;
            let out = strip_bookmarks(xml, &[PathBuf::from("/home/j/a>b.jpg")]).unwrap();
            assert!(!out.contains("bookmark"));
        }

        #[test]
        fn a_uri_for_another_scheme_is_not_a_path() {
            assert!(!uri_is("http://example.com/holiday.jpg", Path::new("/holiday.jpg")));
            assert!(!uri_is("/home/j/holiday.jpg", Path::new("/home/j/holiday.jpg")));
        }
    }
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
