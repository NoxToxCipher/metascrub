//! WebP: rebuild the RIFF chunk list from an allowlist.
//!
//! WebP is a RIFF container holding a small set of four-character chunks. EXIF
//! and XMP have their own chunk types (`EXIF`, `XMP `), which makes them easy
//! to drop, but there is a second step that is easy to forget: the `VP8X`
//! extended-format header carries a flag byte announcing which of those chunks
//! exist. Removing the chunk without clearing the flag leaves a file that
//! promises metadata it no longer has, and some decoders will reject it.
//!
//! Animated WebP nests its frames inside `ANMF` chunks. Those are copied whole
//! rather than descended into, because the sub-chunks a frame may contain
//! (`ALPH`, `VP8 `, `VP8L`) are all image data, and the format gives metadata
//! no place to live there.

use crate::error::Error;
use crate::policy::Policy;
use crate::report::{Kind, Report};
use crate::util::Reader;

const FORMAT: &str = "WebP";

/// Chunks copied through: the two bitstream encodings, the extended header,
/// the alpha plane, and the animation container and its frames.
const KEEP: &[&[u8; 4]] = &[b"VP8 ", b"VP8L", b"VP8X", b"ALPH", b"ANIM", b"ANMF"];

/// Feature flags in the `VP8X` header's first byte. Bit 7 and 6 are reserved,
/// then ICC, alpha, EXIF, XMP, animation, and one more reserved bit.
const FLAG_ICC: u8 = 0x20;
const FLAG_EXIF: u8 = 0x08;
const FLAG_XMP: u8 = 0x04;

pub(crate) fn sanitize(
    input: &[u8],
    policy: &Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    let mut r = Reader::new(input);
    if r.take(4) != Some(b"RIFF") {
        return Err(Error::malformed(FORMAT, "missing RIFF header"));
    }
    let Some(riff_len) = r.u32_le() else {
        return Err(Error::malformed(FORMAT, "truncated RIFF length"));
    };
    if r.take(4) != Some(b"WEBP") {
        return Err(Error::malformed(FORMAT, "RIFF container is not WebP"));
    }

    // The RIFF length counts everything after itself. A file that declares more
    // than it holds is truncated; one that declares less has a trailer.
    let declared_end = 8usize.saturating_add(riff_len as usize);
    if declared_end < input.len() {
        report.removed(Kind::Trailer, "after the RIFF payload", input.len() - declared_end);
    }
    let end = declared_end.min(input.len());

    let mut chunks: Vec<([u8; 4], &[u8])> = Vec::new();

    while r.pos() < end {
        let Some(ty) = r.take(4).and_then(|t| <[u8; 4]>::try_from(t).ok()) else {
            return Err(Error::malformed(FORMAT, "truncated chunk header"));
        };
        let Some(len) = r.u32_le() else {
            return Err(Error::malformed(FORMAT, format!("{} has no length", name(&ty))));
        };
        let Some(body) = r.take(len as usize) else {
            return Err(Error::malformed(
                FORMAT,
                format!("{} claims {len} bytes but the file ends first", name(&ty)),
            ));
        };
        // RIFF pads odd-length chunk bodies to an even boundary.
        if len % 2 == 1 {
            let _ = r.u8();
        }

        if KEEP.contains(&&ty) || (&ty == b"ICCP" && policy.keep_icc()) {
            chunks.push((ty, body));
            continue;
        }

        match &ty {
            b"EXIF" => {
                let found = crate::exif::inspect_tiff(body);
                report.found_location |= found.gps;
                if found.maker_note {
                    report.removed(Kind::MakerNote, "EXIF maker note", 0);
                }
                if found.thumbnail {
                    report.removed(Kind::Thumbnail, "EXIF IFD1", 0);
                }
                report.removed(Kind::Exif, "EXIF", body.len());
            }
            b"XMP " => report.removed(Kind::Xmp, "XMP", body.len()),
            b"ICCP" => report.removed(Kind::ColorProfile, "ICCP", body.len()),
            _ => report.removed(Kind::UnknownStructure, name(&ty), body.len()),
        }
    }

    if chunks.is_empty() {
        return Err(Error::malformed(FORMAT, "no image data chunk"));
    }

    // A VP8X feature header is a fixed 10 bytes; anything past that is not part
    // of the format.
    const VP8X_LEN: usize = 10;

    // Write straight into the output. Only VP8X (~10 bytes) needs a mutable
    // copy; every other chunk — including the VP8/VP8L bitstream, which is the
    // bulk of the file — is copied once, by reference, instead of being cloned
    // into an intermediate buffer and then copied again into the output.
    let mut out = Vec::with_capacity(input.len() + 8);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&[0u8; 4]); // RIFF length, back-patched once known
    out.extend_from_slice(b"WEBP");
    for (ty, data) in chunks {
        if &ty == b"VP8X" {
            // Trailing bytes past the canonical 10 would ride through inside a
            // chunk we report as clean (decoders read only the fixed header), so
            // drop them. A conformant VP8X is untouched.
            let keep = data.len().min(VP8X_LEN);
            let mut fixed = data[..keep].to_vec();
            clear_vp8x_flags(&mut fixed, policy);
            out.extend_from_slice(&ty);
            out.extend_from_slice(&(fixed.len() as u32).to_le_bytes());
            out.extend_from_slice(&fixed);
            if fixed.len() % 2 == 1 {
                out.push(0);
            }
        } else {
            out.extend_from_slice(&ty);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(data);
            if data.len() % 2 == 1 {
                out.push(0);
            }
        }
    }

    // The RIFF length counts everything after the 8-byte "RIFF"+length header.
    let riff_len = (out.len() - 8) as u32;
    out[4..8].copy_from_slice(&riff_len.to_le_bytes());
    Ok(out)
}

/// Clear the feature bits for metadata we removed, so the header stops
/// advertising chunks that are no longer in the file.
fn clear_vp8x_flags(vp8x: &mut [u8], policy: &Policy) {
    if let Some(flags) = vp8x.first_mut() {
        *flags &= !(FLAG_EXIF | FLAG_XMP);
        if !policy.keep_icc() {
            *flags &= !FLAG_ICC;
        }
    }
}

fn name(ty: &[u8; 4]) -> String {
    ty.iter()
        .map(|&b| {
            if b.is_ascii_graphic() || b == b' ' {
                (b as char).to_string()
            } else {
                format!("\\x{b:02x}")
            }
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    fn chunk(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(ty);
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        if body.len() % 2 == 1 {
            v.push(0);
        }
        v
    }

    fn webp(chunks: &[Vec<u8>]) -> Vec<u8> {
        let mut body = b"WEBP".to_vec();
        for c in chunks {
            body.extend_from_slice(c);
        }
        let mut v = b"RIFF".to_vec();
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(&body);
        v
    }

    fn vp8x(flags: u8) -> Vec<u8> {
        let mut body = vec![flags, 0, 0, 0];
        body.extend_from_slice(&[0, 0, 0, 0, 0, 0]); // canvas width/height
        chunk(b"VP8X", &body)
    }

    fn run(input: &[u8], policy: &Policy) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::WebP, input.len());
        let out = sanitize(input, policy, &mut report).expect("valid webp");
        (out, report)
    }

    #[test]
    fn exif_and_xmp_chunks_are_removed_and_the_header_flags_cleared() {
        let input = webp(&[
            vp8x(FLAG_EXIF | FLAG_XMP | FLAG_ICC),
            chunk(b"VP8 ", &[0xAA; 10]),
            chunk(b"EXIF", b"MM\x00\x2a\x00\x00\x00\x08\x00\x00"),
            chunk(b"XMP ", b"<x:xmpmeta>secret</x:xmpmeta>"),
        ]);
        let (out, report) = run(&input, &Policy::default());

        assert!(!out.windows(6).any(|w| w == b"secret"));
        assert!(!out.windows(4).any(|w| w == b"EXIF"));

        let at = out.windows(4).position(|w| w == b"VP8X").unwrap();
        let flags = out[at + 8];
        assert_eq!(flags & (FLAG_EXIF | FLAG_XMP | FLAG_ICC), 0, "stale feature flags remain");

        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Exif));
        assert!(kinds.contains(&Kind::Xmp));
    }

    #[test]
    fn the_bitstream_survives_and_the_riff_length_is_recomputed() {
        let input = webp(&[chunk(b"VP8 ", &[0xAA; 9]), chunk(b"XMP ", b"drop me")]);
        let (out, _) = run(&input, &Policy::default());

        assert!(out.windows(9).any(|w| w == [0xAA; 9]));
        let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
        assert_eq!(declared, out.len() - 8, "the RIFF length must match what we wrote");
    }

    #[test]
    fn odd_length_chunks_stay_padded() {
        let input = webp(&[chunk(b"VP8 ", &[0xAA; 7])]);
        let (out, _) = run(&input, &Policy::default());
        assert_eq!(out.len() % 2, 0);
        let declared = u32::from_le_bytes([out[4], out[5], out[6], out[7]]) as usize;
        assert_eq!(declared, out.len() - 8);
    }

    #[test]
    fn animation_chunks_are_preserved() {
        let input = webp(&[
            vp8x(0x02),
            chunk(b"ANIM", &[0, 0, 0, 0, 0, 0]),
            chunk(b"ANMF", b"frame one payload"),
            chunk(b"ANMF", b"frame two payload"),
        ]);
        let (out, report) = run(&input, &Policy::default());
        assert!(report.is_clean());
        assert_eq!(out.windows(4).filter(|w| *w == b"ANMF").count(), 2);
    }

    #[test]
    fn icc_follows_the_policy_in_both_the_chunk_and_the_flag() {
        let input = webp(&[vp8x(FLAG_ICC), chunk(b"ICCP", b"profile"), chunk(b"VP8 ", &[0; 4])]);

        let (dropped, report) = run(&input, &Policy::strict());
        assert!(!dropped.windows(4).any(|w| w == b"ICCP"));
        let at = dropped.windows(4).position(|w| w == b"VP8X").unwrap();
        assert_eq!(dropped[at + 8] & FLAG_ICC, 0);
        assert!(report.removed.iter().any(|r| r.kind == Kind::ColorProfile));

        let (kept, _) = run(&input, &Policy::preserve_appearance());
        assert!(kept.windows(7).any(|w| w == b"profile"));
        let at = kept.windows(4).position(|w| w == b"VP8X").unwrap();
        assert_eq!(kept[at + 8] & FLAG_ICC, FLAG_ICC, "the flag must still match reality");
    }

    #[test]
    fn an_unknown_chunk_is_dropped() {
        let input = webp(&[chunk(b"VP8 ", &[0; 4]), chunk(b"XtRa", b"vendor-serial-1234")]);
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(13).any(|w| w == b"vendor-serial"));
        assert_eq!(report.removed[0].kind, Kind::UnknownStructure);
        assert_eq!(report.removed[0].location, "XtRa");
    }

    #[test]
    fn bytes_past_the_declared_riff_length_are_dropped() {
        let mut input = webp(&[chunk(b"VP8 ", &[0; 4])]);
        input.extend_from_slice(b"appended after the container");
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(8).any(|w| w == b"appended"));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Trailer));
    }

    #[test]
    fn a_file_with_no_image_data_is_rejected() {
        let mut report = Report::new(Format::WebP, 0);
        let input = webp(&[chunk(b"XMP ", b"metadata only")]);
        assert!(sanitize(&input, &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn malformed_input_is_an_error() {
        let mut report = Report::new(Format::WebP, 0);
        assert!(sanitize(b"RIFF\x04\x00\x00\x00WAVE", &Policy::default(), &mut report).is_err());
        assert!(sanitize(b"nope", &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = webp(&[vp8x(0x0C), chunk(b"VP8 ", &[0; 5]), chunk(b"EXIF", b"MM\x00\x2a")]);
        for n in 0..full.len() {
            let mut report = Report::new(Format::WebP, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }
}
