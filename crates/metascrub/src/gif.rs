//! GIF: rebuild the block stream from an allowlist.
//!
//! A GIF is a header, a logical screen descriptor, an optional global colour
//! table, then a sequence of blocks ending in a trailer byte. The blocks that
//! draw the picture (image descriptors and their data) and control animation
//! (graphic control extensions, and the NETSCAPE loop extension) are kept. The
//! blocks that carry metadata are dropped:
//!
//! - **Comment extensions** (`0x21 0xFE`) hold free text, and editors have used
//!   them for author and software strings.
//! - **Application extensions** (`0x21 0xFF`) other than the NETSCAPE loop
//!   counter. This is where XMP is stored, in an "XMP DataXMP" application
//!   block, and where various tools stamp their own identifiers.
//! - **Plain-text extensions** (`0x21 0x01`), an obsolete way to render text
//!   that no modern decoder produces and that can carry arbitrary strings.
//! - **The trailer.** Anything after the `0x3B` trailer byte is not part of the
//!   image, the same hiding place JPEG and PNG have after their end markers.

use crate::error::Error;
use crate::report::{Kind, Report};
use crate::util::Reader;

const FORMAT: &str = "GIF";
const EXTENSION: u8 = 0x21;
const IMAGE_SEP: u8 = 0x2C;
const TRAILER: u8 = 0x3B;
const GRAPHIC_CONTROL: u8 = 0xF9;
const COMMENT: u8 = 0xFE;
const PLAIN_TEXT: u8 = 0x01;
const APPLICATION: u8 = 0xFF;

pub(crate) fn sanitize(
    input: &[u8],
    _policy: &crate::Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    let mut r = Reader::new(input);

    // Header: "GIF87a" or "GIF89a".
    let header = r.take(6).ok_or_else(|| Error::malformed(FORMAT, "truncated header"))?;
    if &header[..3] != b"GIF" {
        return Err(Error::malformed(FORMAT, "not a GIF header"));
    }

    let mut out = Vec::with_capacity(input.len());
    out.extend_from_slice(header);

    // Logical screen descriptor: width(2), height(2), packed(1), bg(1), aspect(1).
    let lsd = r.take(7).ok_or_else(|| Error::malformed(FORMAT, "truncated screen descriptor"))?;
    out.extend_from_slice(lsd);

    // Global colour table, if the packed field's high bit is set.
    let packed = lsd[4];
    if packed & 0x80 != 0 {
        let size = 3 * (1usize << ((packed & 0x07) + 1));
        let gct = r
            .take(size)
            .ok_or_else(|| Error::malformed(FORMAT, "truncated global colour table"))?;
        out.extend_from_slice(gct);
    }

    // The spec allows one NETSCAPE loop extension; extras are a smuggling channel
    // (2 attacker-chosen bytes each), so only the first is kept.
    let mut netscape_seen = false;

    loop {
        let Some(marker) = r.u8() else {
            // A GIF should end at its trailer; a missing one means truncation.
            report
                .warn("the file ended without a trailer byte, so it was truncated; one was added");
            out.push(TRAILER);
            break;
        };
        match marker {
            TRAILER => {
                out.push(TRAILER);
                let trailing = r.remaining();
                if trailing > 0 {
                    report.removed(Kind::Trailer, "after trailer", trailing);
                }
                break;
            }
            IMAGE_SEP => {
                // Image descriptor: 9 bytes, then an optional local colour table,
                // then the LZW-compressed image as sub-blocks. All kept.
                out.push(IMAGE_SEP);
                let desc = r
                    .take(9)
                    .ok_or_else(|| Error::malformed(FORMAT, "truncated image descriptor"))?;
                out.extend_from_slice(desc);
                if desc[8] & 0x80 != 0 {
                    let size = 3 * (1usize << ((desc[8] & 0x07) + 1));
                    let lct = r
                        .take(size)
                        .ok_or_else(|| Error::malformed(FORMAT, "truncated local colour table"))?;
                    out.extend_from_slice(lct);
                }
                // LZW minimum code size, then sub-blocks.
                let min_code =
                    r.u8().ok_or_else(|| Error::malformed(FORMAT, "truncated image data"))?;
                out.push(min_code);
                copy_sub_blocks(&mut r, &mut out)?;
            }
            EXTENSION => {
                let label =
                    r.u8().ok_or_else(|| Error::malformed(FORMAT, "truncated extension"))?;
                match label {
                    GRAPHIC_CONTROL => {
                        // Animation timing and transparency: structural, kept —
                        // but canonicalized, not copied. A conformant GCE is
                        // exactly one 4-byte sub-block; copying the sub-block
                        // chain verbatim let arbitrary trailing sub-blocks ride
                        // through as COMPLETE (the same smuggle class as NETSCAPE
                        // and VP8X). Emit exactly the 4 structural bytes.
                        let block = read_sub_blocks(&mut r)?;
                        let mut fields = [0u8; 4];
                        let first = block.first().map(|b| b.as_slice()).unwrap_or(&[]);
                        let n = first.len().min(4);
                        fields[..n].copy_from_slice(&first[..n]);
                        out.push(EXTENSION);
                        out.push(label);
                        out.push(4);
                        out.extend_from_slice(&fields);
                        out.push(0); // block terminator
                        let total: usize = block.iter().map(|b| b.len()).sum();
                        if total > 4 || block.len() > 1 {
                            report.removed(
                                Kind::UnknownStructure,
                                "non-standard bytes in graphic-control extension",
                                total.saturating_sub(4),
                            );
                        }
                    }
                    APPLICATION => {
                        // Keep only the NETSCAPE loop counter, which controls
                        // whether an animation repeats. Everything else here,
                        // including XMP, is metadata.
                        let block = read_sub_blocks(&mut r)?;
                        let is_loop = block
                            .first()
                            .map(|b| b.len() >= 11 && &b[..11] == b"NETSCAPE2.0")
                            .unwrap_or(false);
                        if is_loop && !netscape_seen {
                            // Canonicalize instead of copying the parsed block back.
                            // Only the two-byte loop count is meaningful; trailing
                            // bytes in the identifier sub-block and any extra data
                            // sub-blocks are a smuggling spot (arbitrary bytes ride
                            // through as a "loop" and the file still reports COMPLETE,
                            // exactly the oversized-VP8X class). Emit the fixed
                            // 11-byte identifier and a single 3-byte loop sub-block,
                            // so nothing else can survive by construction.
                            let (lo, hi) = block
                                .get(1)
                                .filter(|b| b.len() >= 3 && b[0] == 0x01)
                                .map(|b| (b[1], b[2]))
                                .unwrap_or((0, 0)); // 0,0 = loop forever (the default)
                            out.push(EXTENSION);
                            out.push(label);
                            out.push(11);
                            out.extend_from_slice(b"NETSCAPE2.0");
                            out.push(3);
                            out.extend_from_slice(&[0x01, lo, hi]);
                            out.push(0); // block terminator
                            netscape_seen = true;

                            // Disclose if the input carried more than the canonical
                            // 14 bytes (11 identifier + 3 loop), i.e. something was
                            // hidden in it.
                            let total: usize = block.iter().map(|b| b.len()).sum();
                            let canonical = block.len() == 2
                                && block[0].len() == 11
                                && block.get(1).map(|b| b.len()) == Some(3);
                            if !canonical {
                                report.removed(
                                    Kind::UnknownStructure,
                                    "non-standard bytes in NETSCAPE loop extension",
                                    total.saturating_sub(14),
                                );
                            }
                        } else {
                            let bytes: usize = block.iter().map(|b| b.len()).sum();
                            let name = block
                                .first()
                                .map(|b| {
                                    String::from_utf8_lossy(&b[..b.len().min(11)]).into_owned()
                                })
                                .unwrap_or_default();
                            let kind = if name.contains("XMP") {
                                Kind::Xmp
                            } else {
                                Kind::UnknownStructure
                            };
                            report.removed(kind, format!("application extension {name}"), bytes);
                        }
                    }
                    COMMENT => {
                        let bytes = skip_sub_blocks(&mut r)?;
                        report.removed(Kind::Comment, "comment extension", bytes);
                    }
                    PLAIN_TEXT => {
                        let bytes = skip_sub_blocks(&mut r)?;
                        report.removed(Kind::UnknownStructure, "plain-text extension", bytes);
                    }
                    other => {
                        let bytes = skip_sub_blocks(&mut r)?;
                        report.removed(
                            Kind::UnknownStructure,
                            format!("extension {other:#04x}"),
                            bytes,
                        );
                    }
                }
            }
            other => {
                return Err(Error::malformed(
                    FORMAT,
                    format!("unexpected block marker {other:#04x}"),
                ));
            }
        }
    }

    Ok(out)
}

/// Copy a chain of sub-blocks (each: length byte, then that many bytes) up to
/// and including the terminating zero-length block, verbatim.
fn copy_sub_blocks(r: &mut Reader, out: &mut Vec<u8>) -> crate::Result<()> {
    loop {
        let len = r.u8().ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block"))?;
        out.push(len);
        if len == 0 {
            return Ok(());
        }
        let data = r
            .take(len as usize)
            .ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block data"))?;
        out.extend_from_slice(data);
    }
}

/// Consume a chain of sub-blocks without keeping them; return the bytes skipped.
fn skip_sub_blocks(r: &mut Reader) -> crate::Result<usize> {
    let mut total = 0;
    loop {
        let len = r.u8().ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block"))?;
        if len == 0 {
            return Ok(total);
        }
        r.take(len as usize).ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block data"))?;
        total += len as usize;
    }
}

/// Read a chain of sub-blocks into owned pieces, so their content can be
/// inspected (to tell a NETSCAPE loop from an XMP packet) before deciding.
fn read_sub_blocks(r: &mut Reader) -> crate::Result<Vec<Vec<u8>>> {
    let mut blocks = Vec::new();
    loop {
        let len = r.u8().ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block"))?;
        if len == 0 {
            return Ok(blocks);
        }
        let data = r
            .take(len as usize)
            .ok_or_else(|| Error::malformed(FORMAT, "truncated sub-block data"))?;
        blocks.push(data.to_vec());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;
    use crate::Format;

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Gif, input.len());
        let out = sanitize(input, &crate::Policy::default(), &mut report).expect("valid gif");
        (out, report)
    }

    /// A minimal 1x1 GIF89a: header, screen descriptor with a 2-colour global
    /// table, one image, trailer. Extensions are added by the caller.
    fn gif(extensions: &[u8]) -> Vec<u8> {
        let mut v = b"GIF89a".to_vec();
        v.extend_from_slice(&[1, 0, 1, 0, 0x80, 0, 0]); // 1x1, GCT flag, size 0 -> 2 entries
        v.extend_from_slice(&[0, 0, 0, 255, 255, 255]); // 2-colour GCT
        v.extend_from_slice(extensions);
        // image descriptor 1x1, no LCT
        v.extend_from_slice(&[0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0]);
        v.extend_from_slice(&[0x02]); // LZW min code size
        v.extend_from_slice(&[0x02, 0x44, 0x01]); // one sub-block
        v.push(0x00); // block terminator
        v.push(0x3B); // trailer
        v
    }

    fn comment_ext(text: &[u8]) -> Vec<u8> {
        let mut v = vec![0x21, 0xFE, text.len() as u8];
        v.extend_from_slice(text);
        v.push(0x00);
        v
    }

    #[test]
    fn a_comment_is_removed_but_the_image_survives() {
        let input = gif(&comment_ext(b"Author: Jane Photographer"));
        let (out, report) = run(&input);
        assert!(!out.windows(6).any(|w| w == b"Author"), "comment survived");
        assert!(report.removed.iter().any(|r| r.kind == Kind::Comment));
        // The image data and trailer are still there.
        assert_eq!(*out.last().unwrap(), 0x3B);
        assert!(out.windows(1).any(|w| w == [0x2C]), "image descriptor was dropped");
    }

    #[test]
    fn an_xmp_application_extension_is_removed() {
        // Application extension whose identifier is "XMP DataXMP".
        let mut ext = vec![0x21, 0xFF, 11];
        ext.extend_from_slice(b"XMP DataXMP");
        ext.extend_from_slice(&[5]);
        ext.extend_from_slice(b"<x/>a");
        ext.push(0x00);
        let (out, report) = run(&gif(&ext));
        assert!(!out.windows(3).any(|w| w == b"XMP"), "XMP survived");
        assert!(report.removed.iter().any(|r| r.kind == Kind::Xmp));
    }

    #[test]
    fn the_netscape_loop_extension_is_kept() {
        let mut ext = vec![0x21, 0xFF, 11];
        ext.extend_from_slice(b"NETSCAPE2.0");
        ext.extend_from_slice(&[3, 1, 0, 0]); // loop sub-block
        ext.push(0x00);
        let (out, _report) = run(&gif(&ext));
        assert!(out.windows(11).any(|w| w == b"NETSCAPE2.0"), "the animation loop was dropped");
    }

    #[test]
    fn a_netscape_extension_cannot_smuggle_extra_bytes() {
        // A "loop" extension with junk appended to the identifier sub-block and
        // an extra data sub-block carrying a payload. The loop must survive; the
        // smuggled bytes must not (they used to ride through as COMPLETE).
        let mut ext = vec![0x21, 0xFF, 14]; // identifier sub-block, 14 bytes
        ext.extend_from_slice(b"NETSCAPE2.0"); // 11 bytes...
        ext.extend_from_slice(b"GPS"); // ...+3 smuggled bytes in the identifier
        ext.extend_from_slice(&[3, 1, 5, 0]); // loop sub-block: count = 5
        ext.extend_from_slice(&[9]); // extra data sub-block, 9 bytes
        ext.extend_from_slice(b"SECRETGPS");
        ext.push(0x00); // terminator
        let (out, report) = run(&gif(&ext));
        assert!(out.windows(11).any(|w| w == b"NETSCAPE2.0"), "the loop was dropped");
        assert!(!out.windows(3).any(|w| w == b"GPS"), "identifier-tail smuggle survived");
        assert!(!out.windows(9).any(|w| w == b"SECRETGPS"), "extra sub-block smuggle survived");
        assert!(
            report.removed.iter().any(|r| r.kind == Kind::UnknownStructure),
            "the smuggled bytes were dropped silently instead of being disclosed"
        );
    }

    #[test]
    fn data_after_the_trailer_is_removed() {
        let mut input = gif(&[]);
        input.extend_from_slice(b"HIDDEN-AFTER-TRAILER");
        let (out, report) = run(&input);
        assert!(!out.windows(6).any(|w| w == b"HIDDEN"));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Trailer));
    }

    #[test]
    fn truncated_input_is_an_error_not_a_panic() {
        for cut in 0..30 {
            let full = gif(&comment_ext(b"x"));
            let mut report = Report::new(Format::Gif, cut);
            let _ = sanitize(&full[..cut.min(full.len())], &crate::Policy::default(), &mut report);
        }
    }
}
