//! `metascrub` — format-aware metadata removal for images and documents
//! (DESIGN §9.1).
//!
//! Photographs and office documents carry a second payload that has nothing to
//! do with what the file looks like: GPS coordinates, camera serial numbers, the
//! author's real name, the editing session identifiers that link two documents
//! to the same machine. Sending a file sends all of it. This crate removes that
//! payload before the file leaves the device.
//!
//! ## Design: allowlist, never denylist
//!
//! The obvious way to strip metadata is to look for the known metadata blocks
//! and delete them. That approach fails silently on anything it has not been
//! taught about: a vendor's private JPEG segment, a new PNG chunk type, a
//! trailing blob appended after the end-of-image marker. Every one of those
//! survives a denylist and none of them survive an allowlist.
//!
//! So the container is not edited, it is **rebuilt**. Each parser walks the
//! input, copies across only the structures on an explicit keep-list, and drops
//! everything else including structures it does not recognise. Anything new,
//! private or deliberately hidden is discarded by default because it was never
//! on the list.
//!
//! ## Re-encode versus strip
//!
//! A rebuild of this kind is byte-exact on the pixel data: the compressed image
//! bitstream is copied through untouched, so there is no generational quality
//! loss. Re-encoding would guarantee removal too, but it costs quality on every
//! pass and it is not actually safer for *metadata*, because an allowlist
//! rebuild already carries nothing but pixels forward.
//!
//! Re-encoding does do one thing a rebuild cannot: it disturbs the sensor
//! fingerprint (PRNU) carried in the pixel values themselves. That is a
//! genuinely different protection, it is lossy, and it is deliberately **not**
//! part of this crate (DESIGN §9.2 keeps the two separate so the interface never
//! implies that stripping EXIF touched the pixels).
//!
//! ## Honest reporting
//!
//! Not every container can be rebuilt with the same confidence, so every result
//! carries an [`Assurance`] level and an itemised list of what was removed.
//! A format we cannot parse is reported as [`Assurance::None`] and returned
//! unchanged rather than passed off as clean. Claiming success on a file we did
//! not understand is the one failure mode that actively harms the user.
//!
//! ## Example
//!
//! ```no_run
//! use metascrub::{sanitize, Policy};
//!
//! let photo = std::fs::read("holiday.jpg")?;
//! let clean = sanitize(&photo, &Policy::default())?;
//! if clean.report.found_location {
//!     eprintln!("this photo carried GPS coordinates");
//! }
//! std::fs::write("holiday.clean.jpg", &clean.data)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod detect;
mod error;
mod policy;
mod report;
mod util;

// These parse pure byte structure (or, for SVG, text) and pull in no decoding
// library, so they are always built rather than gated on `image`.
mod gif;
mod raw;
mod svg;
mod tiff;
mod xmp;

#[cfg(feature = "image")]
mod exif;
#[cfg(feature = "image")]
mod heif;
#[cfg(feature = "image")]
mod jpeg;
#[cfg(feature = "image")]
mod png;
#[cfg(feature = "image")]
mod webp;

#[cfg(feature = "pdf")]
mod pdf;

#[cfg(feature = "ooxml")]
mod ooxml;
#[cfg(feature = "ooxml")]
mod xmlscrub;
#[cfg(feature = "ooxml")]
mod zip;

pub use detect::{detect, Format};
pub use error::Error;
pub use policy::{ColorProfile, Orientation, Policy};
pub use report::{Assurance, Kind, Removed, Report, Retained, Verification};

/// Result type for every fallible operation in this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// A sanitized file plus the account of what was taken out of it.
#[derive(Debug, Clone)]
pub struct Sanitized {
    /// The rebuilt file. For [`Assurance::None`] this is the input verbatim.
    pub data: Vec<u8>,
    /// What was found, what was removed, and how much to trust the result.
    pub report: Report,
}

/// Strip metadata from an in-memory file.
///
/// The format is detected from the leading bytes, never from a filename, since
/// an attacker-supplied extension is not evidence of anything.
///
/// An unrecognised format is **not** an error: the input is returned unchanged
/// with [`Assurance::None`], so a caller can present "we could not clean this"
/// as a distinct outcome from "this failed". Malformed input in a format we
/// *do* claim to handle is an error, because a partial parse means a partial
/// strip.
pub fn sanitize(input: &[u8], policy: &Policy) -> Result<Sanitized> {
    sanitize_at_depth(input, policy, 0)
}

/// Sanitize, then check the result: re-scan the cleaned output to confirm
/// nothing this tool removes survived, and confirm the clean is reproducible by
/// running it a second time and comparing bytes.
///
/// This is the tool marking its own homework. It cannot catch a leak in a
/// structure the tool does not know to look for (nothing can verify what it
/// cannot see), but it does catch the failure that matters most: a `Complete`
/// clean that quietly left recognised metadata behind, and any per-run value
/// leaking into the output. The findings are attached as
/// [`Report::verification`].
pub fn sanitize_verified(input: &[u8], policy: &Policy) -> Result<Sanitized> {
    let mut first = sanitize_at_depth(input, policy, 0)?;

    // Verification only means anything for a file we actually took apart. When
    // the format is one we cannot clean (`Assurance::None`) the output is the
    // input returned verbatim, so a "re-scan found nothing removable, and the
    // clean is reproducible" pass would be trivially true — and would render as
    // a green tick beside the red "NOT CLEANED" badge, asserting a clean on the
    // exact file we could not clean. Leave verification unset there.
    if first.report.assurance != Assurance::None {
        // Reproducibility: a second run must produce identical bytes.
        let second = sanitize_at_depth(input, policy, 0)?;
        let deterministic = first.data == second.data;

        // Re-inspect the cleaned output: a second scan should find nothing left
        // that we claim to remove. Disclosed residuals (a kept maker note) are
        // not "removable", so they do not count against this.
        let reinspection = sanitize_at_depth(&first.data, policy, 0)?.report;
        let output_reinspected_clean = reinspection.removed.is_empty();

        first.report.verification =
            Some(Verification { output_reinspected_clean, deterministic });
    }
    Ok(first)
}

/// How deeply the sanitizer will recurse into embedded files.
///
/// An Office document is a ZIP, and a ZIP can hold another Office document under
/// `word/media/`, which the recursion happily descends into. A file crafted to
/// nest thousands of levels deep would otherwise overflow the stack, and with
/// `panic = "abort"` a stack overflow ends the process: a denial of service
/// from a single file. Legitimate documents nest a level or two at most.
pub(crate) const MAX_RECURSION_DEPTH: u32 = 8;

/// The real entry point. `depth` is the number of container boundaries already
/// crossed; it starts at zero for the file the user handed us.
pub(crate) fn sanitize_at_depth(input: &[u8], policy: &Policy, depth: u32) -> Result<Sanitized> {
    if let Some(limit) = policy.max_input_bytes {
        if input.len() as u64 > limit {
            return Err(Error::TooLarge { len: input.len() as u64, limit });
        }
    }
    let format = detect(input);
    let mut report = Report::new(format, input.len());

    // A container that would take us past the limit is refused rather than
    // descended into. Refusing is safe: the outer file is still rebuilt, and the
    // one nested part that was too deep is reported as left alone.
    let nestable = matches!(format, Format::Ooxml | Format::OpenDocument);
    if nestable && depth >= MAX_RECURSION_DEPTH {
        report.assurance = Assurance::BestEffort;
        report.warn(
            "this file nests archives more deeply than we will follow, so the deepest parts \
             were left as they arrived; a document nested this way is almost never innocent",
        );
        report.output_len = input.len();
        return Ok(Sanitized { data: input.to_vec(), report });
    }

    let data = match format {
        #[cfg(feature = "image")]
        Format::Jpeg => jpeg::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::Png => png::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::WebP => webp::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::Heif | Format::Avif => heif::sanitize(input, policy, &mut report)?,
        Format::Gif => gif::sanitize(input, policy, &mut report)?,
        Format::Tiff => tiff::sanitize(input, policy, &mut report)?,
        Format::Svg => svg::sanitize(input, policy, &mut report)?,
        Format::Xmp => xmp::sanitize(input, policy, &mut report)?,
        Format::Raw => raw::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "pdf")]
        Format::Pdf => pdf::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "ooxml")]
        Format::Ooxml | Format::OpenDocument => ooxml::sanitize(input, policy, &mut report, depth)?,
        // Video and audio are recognised so the user is told plainly what the
        // file is and that it was NOT cleaned, rather than leaving them to
        // assume an "unknown format" might be harmless. Cleaning these is not yet
        // built.
        Format::Video => {
            report.assurance = Assurance::None;
            report.warn(
                "this is a video file, and metascrub cannot clean video yet, so nothing was \
                 removed. Videos commonly carry the GPS location where they were shot, the device \
                 model, and the exact date and time. Do not assume this file is clean. If it was \
                 recorded on a phone, the safest option today is a platform that strips video \
                 metadata, or a dedicated video tool.",
            );
            input.to_vec()
        }
        Format::Audio => {
            report.assurance = Assurance::None;
            report.warn(
                "this is an audio file, and metascrub cannot clean audio yet, so nothing was \
                 removed. Audio files carry tags that can include the device or software used, a \
                 timestamp, and any title or artist text. Do not assume this file is clean.",
            );
            input.to_vec()
        }
        _ => {
            report.assurance = Assurance::None;
            report.warn(
                "this format is not one metascrub can take apart, so nothing was removed; \
                 treat the file as if it still carries everything it arrived with",
            );
            input.to_vec()
        }
    };

    report.output_len = data.len();
    Ok(Sanitized { data, report })
}

/// Report what metadata a file carries without producing a cleaned copy.
///
/// This runs the same parsers as [`sanitize`] and discards the output, so the
/// findings are exactly the ones a real run would act on. Useful for a "what is
/// in this file?" preview before the user commits to sending it.
pub fn inspect(input: &[u8], policy: &Policy) -> Result<Report> {
    Ok(sanitize(input, policy)?.report)
}

/// Read `src`, sanitize it, and write the result to `dst`.
///
/// `dst` is written via a temporary file in the same directory and then
/// renamed, so an interrupted run cannot leave a half-written file that looks
/// clean. Passing the same path for both is therefore safe.
///
/// The temporary file is created exclusively (`create_new`) under an
/// unpredictable name, and on Unix with `0o600`. A fixed, guessable temp name in
/// a directory someone else can write to is the classic way to turn "write a
/// file" into "write through a symlink of my choosing", and the cleaned file is
/// the user's photo, so its contents should not be briefly world-readable
/// either.
pub fn sanitize_file(
    src: impl AsRef<std::path::Path>,
    dst: impl AsRef<std::path::Path>,
    policy: &Policy,
) -> Result<Report> {
    let (src, dst) = (src.as_ref(), dst.as_ref());
    let input = std::fs::read(src)?;
    let out = sanitize(&input, policy)?;
    write_atomic(dst, &out.data)?;
    Ok(out.report)
}

/// Write `data` to `dst` atomically and safely.
///
/// The one hardened writer, so an interface never has to reimplement it and get
/// it subtly weaker. Any caller that has already produced cleaned bytes (a GUI
/// with the result in memory, a batch job) should write through this rather than
/// `fs::write`.
///
/// - **Atomic.** Written to a temporary file in the same directory and renamed.
///   A rename within a directory is atomic on Unix and Windows, so `dst` is
///   either the old contents or the whole new file, never a half-written one
///   wearing a name that says it was cleaned.
/// - **Not a symlink target.** The temporary name is unpredictable and opened
///   with `create_new`, which fails rather than following an existing path. A
///   fixed, guessable temp name in a directory someone else can write to is the
///   classic way to turn "write a file" into "write through a symlink of my
///   choosing".
/// - **Not world-readable.** On Unix the temporary is created `0o600`; the
///   contents are the user's photograph and should not be readable by others
///   even for the moment before the rename.
pub fn write_atomic(dst: impl AsRef<std::path::Path>, data: &[u8]) -> Result<()> {
    use std::io::Write;

    let dst = dst.as_ref();
    let dir = dst.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = dst.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    let mut last_err = None;
    for _ in 0..8 {
        let mut nonce = [0u8; 12];
        getrandom_bytes(&mut nonce);
        let tmp = dir.join(format!(".{stem}.{}.metascrub", hex_lower(&nonce)));

        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }

        match opts.open(&tmp) {
            Ok(mut f) => {
                let write_res = f.write_all(data).and_then(|()| f.sync_all());
                drop(f);
                if let Err(e) = write_res {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
                if let Err(e) = std::fs::rename(&tmp, dst) {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = Some(e);
                continue;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(last_err
        .unwrap_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "could not create a temporary file",
            )
        })
        .into())
}

/// Fill `buf` with OS randomness, falling back to address and time entropy.
///
/// Only used to make a temporary filename unpredictable, never for keys.
fn getrandom_bytes(buf: &mut [u8]) {
    use std::hash::{BuildHasher, Hasher, RandomState};
    let mut h = RandomState::new().build_hasher();
    h.write_usize(buf.as_ptr() as usize);
    h.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    let mut state = h.finish();
    for byte in buf.iter_mut() {
        // SplitMix64, so successive bytes are not a visible counter.
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        *byte = (z ^ (z >> 31)) as u8;
    }
}

fn hex_lower(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// A random, lower-case, filesystem-safe token of `len` characters.
///
/// This is what the "random name" output option is built on. A file's *name* is
/// metadata in its own right: `IMG_20230715_193042.jpg` carries a date, a time,
/// a sequence position and a camera's `IMG_` prefix, and a hand-chosen name like
/// `berlin-march.jpg` carries more. Stripping the bytes inside the file does not
/// touch any of that, so an output whose name is a random token closes the gap.
///
/// The token only has to be unguessable enough to avoid colliding with a file
/// already in the folder and to reveal nothing about the source. It is **not key
/// material** and does not need to be: the name is public, and it says nothing
/// about the original by construction. The entropy source is the same one the
/// atomic writer uses for its temporary names.
///
/// The alphabet is RFC 4648 base32, lower-cased. Thirty-two symbols divide 256
/// evenly, so masking each random byte introduces no bias, and lower-case only
/// means a case-insensitive filesystem cannot fold two distinct names together.
pub fn random_stem(len: usize) -> String {
    const ALPHABET: &[u8; 32] = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut raw = vec![0u8; len];
    getrandom_bytes(&mut raw);
    raw.iter().map(|b| ALPHABET[(b & 0x1f) as usize] as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn random_stem_has_the_right_length_alphabet_and_varies() {
        let a = random_stem(24);
        assert_eq!(a.chars().count(), 24);
        // RFC 4648 base32, lower-cased: a-z and 2-7, nothing else.
        assert!(a.chars().all(|c| matches!(c, 'a'..='z' | '2'..='7')), "unexpected char in {a}");
        // The token is 24 base32 chars wide; its unguessable entropy is bounded
        // by the ~64-bit seed, which is ample for the only jobs it has (not
        // colliding in a directory, saying nothing about the source). Two rolls
        // being equal is effectively impossible.
        assert_ne!(random_stem(24), random_stem(24));
        assert_eq!(random_stem(0), "");
    }

    #[test]
    fn a_file_we_cannot_clean_is_never_marked_verified() {
        // A format we cannot take apart comes back Assurance::None. Attaching a
        // verification there would render as a green "verified clean" tick beside
        // the "NOT CLEANED" badge — a clean asserted on the one file we did not
        // clean. It must stay unset. (A file we DO rebuild still gets its
        // verification: that path is exercised across the roundtrip suite.)
        let junk = b"this is not a file format we know".to_vec();
        let out = sanitize_verified(&junk, &Policy::default()).unwrap();
        assert_eq!(out.report.assurance, Assurance::None);
        assert!(out.report.verification.is_none(), "an uncleaned file must carry no verification");
    }

    #[test]
    fn unknown_format_is_returned_untouched_and_flagged() {
        let junk = b"this is not a file format we know".to_vec();
        let out = sanitize(&junk, &Policy::default()).unwrap();
        assert_eq!(out.data, junk);
        assert_eq!(out.report.assurance, Assurance::None);
        assert_eq!(out.report.format, Format::Unknown);
        assert!(!out.report.warnings.is_empty(), "silence would imply the file was cleaned");
    }

    #[test]
    fn empty_input_does_not_panic() {
        let out = sanitize(&[], &Policy::default()).unwrap();
        assert_eq!(out.report.assurance, Assurance::None);
    }

    #[cfg(feature = "image")]
    #[test]
    fn verify_confirms_a_clean_jpeg_and_is_reproducible() {
        // A minimal JPEG carrying a comment. After a Complete clean, re-scanning
        // the output must find nothing removable, and the clean must be
        // byte-reproducible.
        let mut j = vec![0xFF, 0xD8]; // SOI
        j.extend_from_slice(&[0xFF, 0xFE, 0x00, 0x09]); // COM, length 9
        j.extend_from_slice(b"secret!");
        j.extend_from_slice(&[0xFF, 0xDB, 0x00, 0x43, 0x00]);
        j.extend_from_slice(&[0u8; 64]);
        j.extend_from_slice(&[0xFF, 0xC0, 0x00, 0x0B, 0x08, 0, 8, 0, 8, 1, 1, 0x11, 0]);
        j.extend_from_slice(&[0xFF, 0xC4, 0x00, 0x02]);
        j.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 1, 1, 0, 0, 0x3F, 0]);
        j.extend_from_slice(&[0x12, 0x34]);
        j.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let out = sanitize_verified(&j, &Policy::default()).unwrap();
        let v = out.report.verification.expect("verification was requested");
        assert!(v.deterministic, "a Complete clean must be reproducible");
        assert!(v.output_reinspected_clean, "re-scan found removable metadata in a Complete clean");
        assert!(v.passed());
        assert!(!out.data.windows(7).any(|w| w == b"secret!"), "the comment survived");
    }

    /// An Office document is a ZIP and can hold another Office document, so the
    /// recursion must not be able to run away. Without the depth limit this
    /// overflows the stack, which under `panic = "abort"` kills the process:
    /// a denial of service from one crafted file.
    #[cfg(feature = "ooxml")]
    #[test]
    fn deeply_nested_documents_do_not_overflow_the_stack() {
        use ::zip::write::SimpleFileOptions;
        use std::io::Write;

        fn minimal_docx(inner: Option<(&str, Vec<u8>)>) -> Vec<u8> {
            let mut buf = std::io::Cursor::new(Vec::new());
            {
                let mut z = ::zip::ZipWriter::new(&mut buf);
                let opts = SimpleFileOptions::default();
                z.start_file("[Content_Types].xml", opts).unwrap();
                z.write_all(br#"<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"/>"#).unwrap();
                z.start_file("word/document.xml", opts).unwrap();
                z.write_all(br#"<?xml version="1.0"?><document/>"#).unwrap();
                if let Some((name, data)) = inner {
                    z.start_file(name, opts).unwrap();
                    z.write_all(&data).unwrap();
                }
                z.finish().unwrap();
            }
            buf.into_inner()
        }

        let mut payload = minimal_docx(None);
        for i in 0..500 {
            payload = minimal_docx(Some((&format!("word/media/{i}.docx"), payload)));
        }

        // The assertion is simply that this returns rather than overflowing the
        // stack. Two things stop it now, and either alone is sufficient: the
        // recursion depth limit, and detecting embedded content by type so a
        // nested archive (which is not an image) is not descended into at all.
        let out = sanitize(&payload, &Policy::default()).expect("must not error");
        assert!(!out.data.is_empty());
    }

    #[test]
    fn size_limit_is_enforced_before_parsing() {
        let policy = Policy { max_input_bytes: Some(8), ..Policy::default() };
        assert!(matches!(sanitize(&[0u8; 9], &policy), Err(Error::TooLarge { .. })));
    }
}
