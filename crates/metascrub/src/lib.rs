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
pub use report::{Assurance, Kind, Removed, Report};

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
    if let Some(limit) = policy.max_input_bytes {
        if input.len() as u64 > limit {
            return Err(Error::TooLarge { len: input.len() as u64, limit });
        }
    }
    let format = detect(input);
    let mut report = Report::new(format, input.len());

    let data = match format {
        #[cfg(feature = "image")]
        Format::Jpeg => jpeg::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::Png => png::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::WebP => webp::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "image")]
        Format::Heif | Format::Avif => heif::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "pdf")]
        Format::Pdf => pdf::sanitize(input, policy, &mut report)?,
        #[cfg(feature = "ooxml")]
        Format::Ooxml | Format::OpenDocument => ooxml::sanitize(input, policy, &mut report)?,
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
    use std::io::Write;

    let (src, dst) = (src.as_ref(), dst.as_ref());
    let input = std::fs::read(src)?;
    let out = sanitize(&input, policy)?;

    let dir = dst.parent().unwrap_or_else(|| std::path::Path::new("."));
    let stem = dst.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();

    // A few attempts, in case the name is already taken. `create_new` fails
    // rather than following an existing path, so losing the race is an error
    // and never a write to somebody else's file.
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
                let write_res = f.write_all(&out.data).and_then(|()| f.sync_all());
                drop(f);
                if let Err(e) = write_res {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
                if let Err(e) = std::fs::rename(&tmp, dst) {
                    let _ = std::fs::remove_file(&tmp);
                    return Err(e.into());
                }
                return Ok(out.report);
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

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn size_limit_is_enforced_before_parsing() {
        let policy = Policy { max_input_bytes: Some(8), ..Policy::default() };
        assert!(matches!(sanitize(&[0u8; 9], &policy), Err(Error::TooLarge { .. })));
    }
}
