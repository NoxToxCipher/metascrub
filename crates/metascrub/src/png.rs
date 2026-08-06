//! PNG: rebuild the chunk stream from an allowlist.
//!
//! PNG's structure makes this the cleanest of the image formats. Every chunk is
//! length-prefixed, typed and CRC-protected, so a parser can walk the whole
//! file without understanding any individual chunk, and the specification is
//! explicit that a decoder may ignore chunks it does not know. That is exactly
//! the property an allowlist needs.
//!
//! What gets dropped: the text chunks (`tEXt`, `zTXt`, `iTXt`), which hold both
//! ordinary captions and the XMP packet that editors write; `eXIf`, which is
//! full EXIF including GPS; `tIME`; and every chunk type not named in
//! [`KEEP`], which covers private chunks and anything added to the format
//! after this was written.
//!
//! What gets kept: the chunks that decide what the image looks like, including
//! the APNG animation chunks, because dropping those turns an animation into a
//! single frame and that is data loss the user did not ask for.

use crate::error::Error;
use crate::policy::Policy;
use crate::report::{Kind, Report};
use crate::util::{crc32_parts, Reader};

const FORMAT: &str = "PNG";
const MAGIC: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Chunk types copied through.
///
/// Critical chunks (`IHDR`, `PLTE`, `IDAT`, `IEND`) plus the ancillary chunks
/// that affect rendering: transparency, gamma, chromaticity, rendering intent,
/// significant bits, background, physical pixel size, palette histogram, and
/// the three APNG chunks.
const KEEP: &[&[u8; 4]] = &[
    b"IHDR", b"PLTE", b"IDAT", b"IEND", // critical
    b"tRNS", b"gAMA", b"cHRM", b"sRGB", b"sBIT", b"bKGD", b"pHYs", b"hIST", // rendering
    b"acTL", b"fcTL", b"fdAT", // APNG animation
];

/// Chunk types we recognise well enough to name in the report. Everything else
/// is reported as an unrecognised structure, which is not a lesser outcome:
/// it is dropped either way.
fn describe(ty: &[u8; 4]) -> Option<Kind> {
    match ty {
        b"tEXt" | b"zTXt" => Some(Kind::Comment),
        // iTXt is where XMP lives, under the keyword "XML:com.adobe.xmp".
        b"iTXt" => Some(Kind::Xmp),
        b"eXIf" => Some(Kind::Exif),
        b"tIME" => Some(Kind::Timestamp),
        b"iCCP" => Some(Kind::ColorProfile),
        b"dSIG" => Some(Kind::UnknownStructure),
        _ => None,
    }
}

pub(crate) fn sanitize(input: &[u8], policy: &Policy, report: &mut Report) -> crate::Result<Vec<u8>> {
    let mut r = Reader::new(input);
    if r.take(8) != Some(&MAGIC) {
        return Err(Error::malformed(FORMAT, "missing PNG signature"));
    }

    let mut out = Vec::with_capacity(input.len());
    out.extend_from_slice(&MAGIC);
    let mut seen_iend = false;

    while !r.is_empty() {
        if seen_iend {
            // IEND terminates the datastream; anything after it is a trailer,
            // the same hiding place JPEG has after EOI.
            report.removed(Kind::Trailer, "after IEND", r.remaining());
            break;
        }

        let Some(len) = r.u32_be() else {
            return Err(Error::malformed(FORMAT, "truncated chunk length"));
        };
        let Some(ty) = r.take(4).and_then(|t| <[u8; 4]>::try_from(t).ok()) else {
            return Err(Error::malformed(FORMAT, "truncated chunk type"));
        };
        // The spec caps chunk length at 2^31-1; anything above is corrupt.
        if len > i32::MAX as u32 {
            return Err(Error::malformed(FORMAT, format!("chunk length {len} exceeds the limit")));
        }
        let Some(body) = r.take(len as usize) else {
            return Err(Error::malformed(
                FORMAT,
                format!("{} claims {len} bytes but the file ends first", type_name(&ty)),
            ));
        };
        let Some(stated_crc) = r.u32_be() else {
            return Err(Error::malformed(FORMAT, format!("{} has no CRC", type_name(&ty))));
        };

        let keep = KEEP.contains(&&ty) || (&ty == b"iCCP" && policy.keep_icc());

        if keep {
            // Only verify what we are about to carry forward. A bad CRC on a
            // chunk we keep means the image data itself is damaged, and
            // copying it while reporting success would hide that.
            let actual = crc32_parts(&[&ty, body]);
            if actual != stated_crc {
                return Err(Error::malformed(
                    FORMAT,
                    format!("{} fails its CRC ({actual:#010x} against {stated_crc:#010x})",
                        type_name(&ty)),
                ));
            }
            out.extend_from_slice(&len.to_be_bytes());
            out.extend_from_slice(&ty);
            out.extend_from_slice(body);
            out.extend_from_slice(&stated_crc.to_be_bytes());
        } else {
            let kind = describe(&ty).unwrap_or(Kind::UnknownStructure);
            report.removed(kind, type_name(&ty), body.len());
            if &ty == b"eXIf" {
                let found = crate::exif::inspect_tiff(body);
                report.found_location |= found.gps;
                if found.maker_note {
                    report.removed(Kind::MakerNote, "eXIf maker note", 0);
                }
            }
        }

        if &ty == b"IEND" {
            seen_iend = true;
        }
    }

    if !seen_iend {
        return Err(Error::malformed(FORMAT, "no IEND chunk; the file is truncated"));
    }
    Ok(out)
}

/// Render a chunk type for a message. Types are ASCII by specification, but the
/// input is untrusted, so non-printable bytes are escaped rather than assumed.
fn type_name(ty: &[u8; 4]) -> String {
    ty.iter()
        .map(|&b| {
            if b.is_ascii_graphic() {
                (b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Assurance;
    use crate::Format;

    fn chunk(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(ty);
        v.extend_from_slice(body);
        v.extend_from_slice(&crc32_parts(&[ty, body]).to_be_bytes());
        v
    }

    /// 1x1 greyscale header plus whatever chunks the test wants, then IEND.
    fn png(extra: &[Vec<u8>]) -> Vec<u8> {
        let mut v = MAGIC.to_vec();
        v.extend_from_slice(&chunk(b"IHDR", &[0, 0, 0, 1, 0, 0, 0, 1, 8, 0, 0, 0, 0]));
        for c in extra {
            v.extend_from_slice(c);
        }
        v.extend_from_slice(&chunk(b"IDAT", &[0x78, 0x9C, 0x63, 0x00, 0x00, 0x00, 0x02, 0x00, 0x01]));
        v.extend_from_slice(&chunk(b"IEND", b""));
        v
    }

    fn run(input: &[u8], policy: &Policy) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Png, input.len());
        let out = sanitize(input, policy, &mut report).expect("valid png");
        (out, report)
    }

    #[test]
    fn text_exif_and_time_chunks_are_removed() {
        let mut exif_body = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
        exif_body.extend_from_slice(&[0, 0, 0, 0, 0, 0]);

        let input = png(&[
            chunk(b"tEXt", b"Author\0Jane Q. Photographer"),
            chunk(b"zTXt", b"Comment\0\0compressed"),
            chunk(b"iTXt", b"XML:com.adobe.xmp\0\0\0\0\0<x:xmpmeta/>"),
            chunk(b"eXIf", &exif_body),
            chunk(b"tIME", &[0x07, 0xE8, 1, 1, 0, 0, 0]),
        ]);
        let (out, report) = run(&input, &Policy::default());

        for needle in [&b"Jane Q. Photographer"[..], b"com.adobe.xmp", b"compressed"] {
            assert!(!out.windows(needle.len()).any(|w| w == needle), "survived");
        }
        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Comment));
        assert!(kinds.contains(&Kind::Xmp));
        assert!(kinds.contains(&Kind::Exif));
        assert!(kinds.contains(&Kind::Timestamp));
        assert_eq!(report.assurance, Assurance::Complete);
    }

    #[test]
    fn image_data_and_rendering_chunks_come_through_untouched() {
        let input = png(&[
            chunk(b"gAMA", &45455u32.to_be_bytes()),
            chunk(b"pHYs", &[0, 0, 0x0B, 0x13, 0, 0, 0x0B, 0x13, 1]),
            chunk(b"tRNS", &[0, 0]),
        ]);
        let (out, report) = run(&input, &Policy::default());
        assert!(report.is_clean(), "nothing here is metadata");
        assert_eq!(out, input, "a file with no metadata must come back byte-identical");
    }

    #[test]
    fn apng_animation_chunks_are_preserved() {
        // Dropping these would silently reduce an animation to one frame.
        let input = png(&[
            chunk(b"acTL", &[0, 0, 0, 2, 0, 0, 0, 0]),
            chunk(b"fcTL", &[0u8; 26]),
            chunk(b"fdAT", &[0, 0, 0, 1, 0xAA, 0xBB]),
        ]);
        let (out, report) = run(&input, &Policy::default());
        assert!(report.is_clean());
        assert!(out.windows(4).any(|w| w == b"acTL"));
        assert!(out.windows(4).any(|w| w == b"fdAT"));
    }

    #[test]
    fn an_unknown_private_chunk_is_dropped() {
        let input = png(&[chunk(b"prVt", b"device-serial-91827364")]);
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(13).any(|w| w == b"device-serial"));
        assert_eq!(report.removed[0].kind, Kind::UnknownStructure);
        assert_eq!(report.removed[0].location, "prVt");
    }

    #[test]
    fn iccp_follows_the_policy() {
        let icc = chunk(b"iCCP", b"my monitor profile\0\0body");
        let (dropped, report) = run(&png(&[icc.clone()]), &Policy::strict());
        assert!(!dropped.windows(4).any(|w| w == b"iCCP"));
        assert_eq!(report.removed[0].kind, Kind::ColorProfile);

        let (kept, report) = run(&png(&[icc]), &Policy::preserve_appearance());
        assert!(kept.windows(4).any(|w| w == b"iCCP"));
        assert!(report.is_clean());
    }

    #[test]
    fn gps_inside_an_exif_chunk_is_surfaced() {
        // IFD0 with a GPS pointer at offset 26, whose sub-IFD holds a latitude.
        let mut body = b"MM\x00\x2a\x00\x00\x00\x08".to_vec();
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0x8825u16.to_be_bytes());
        body.extend_from_slice(&4u16.to_be_bytes());
        body.extend_from_slice(&1u32.to_be_bytes());
        body.extend_from_slice(&26u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());
        body.extend_from_slice(&1u16.to_be_bytes());
        body.extend_from_slice(&0x0002u16.to_be_bytes());
        body.extend_from_slice(&5u16.to_be_bytes());
        body.extend_from_slice(&3u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());
        body.extend_from_slice(&0u32.to_be_bytes());

        let (_, report) = run(&png(&[chunk(b"eXIf", &body)]), &Policy::default());
        assert!(report.found_location);
    }

    #[test]
    fn data_after_iend_is_dropped() {
        let mut input = png(&[]);
        input.extend_from_slice(b"hidden payload after the end");
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(6).any(|w| w == b"hidden"));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Trailer));
    }

    #[test]
    fn a_corrupt_crc_on_a_kept_chunk_is_an_error() {
        let mut input = png(&[]);
        let last = input.len() - 1;
        input[last] ^= 0xFF; // break the IEND CRC
        let mut report = Report::new(Format::Png, input.len());
        assert!(sanitize(&input, &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn a_corrupt_crc_on_a_dropped_chunk_does_not_block_the_strip() {
        // The chunk is going away regardless, so its checksum is moot.
        let mut bad = chunk(b"tEXt", b"Author\0someone");
        let last = bad.len() - 1;
        bad[last] ^= 0xFF;
        let (out, report) = run(&png(&[bad]), &Policy::default());
        assert!(!out.windows(7).any(|w| w == b"someone"));
        assert_eq!(report.removed[0].kind, Kind::Comment);
    }

    #[test]
    fn malformed_input_is_an_error_rather_than_a_partial_strip() {
        let mut report = Report::new(Format::Png, 0);
        assert!(sanitize(b"not a png at all", &Policy::default(), &mut report).is_err());

        // No IEND: we cannot tell a truncated file from a trimmed one.
        let mut no_end = MAGIC.to_vec();
        no_end.extend_from_slice(&chunk(b"IHDR", &[0; 13]));
        assert!(sanitize(&no_end, &Policy::default(), &mut report).is_err());

        // A length that would run off the end.
        let mut huge = MAGIC.to_vec();
        huge.extend_from_slice(&0xFFFF_FFFFu32.to_be_bytes());
        huge.extend_from_slice(b"IDAT");
        assert!(sanitize(&huge, &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = png(&[chunk(b"tEXt", b"k\0v"), chunk(b"prVt", b"x")]);
        for n in 0..full.len() {
            let mut report = Report::new(Format::Png, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }

    #[test]
    fn chunk_type_names_with_odd_bytes_are_escaped_not_printed_raw() {
        assert_eq!(type_name(b"tEXt"), "tEXt");
        assert_eq!(type_name(&[0x00, 0x1B, b'O', b'K']), "\\x00\\x1bOK");
    }
}
