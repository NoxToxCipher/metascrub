//! JPEG: rebuild the marker stream from an allowlist.
//!
//! A JPEG is a sequence of marker segments. The ones that describe how to
//! decode the picture (quantization tables, Huffman tables, frame and scan
//! headers) are structural; the `APPn` and `COM` segments are a free-for-all
//! that vendors have used for EXIF, XMP, IPTC, Photoshop resource blocks,
//! preview images, depth maps, audio clips and whole second JPEGs.
//!
//! So the rule is: copy the structural markers, synthesize the two application
//! segments that affect decoding, and drop every other `APPn` including the
//! ones that did not exist when this was written.
//!
//! Two things that are easy to miss and are handled here:
//!
//! - **The trailer.** Bytes after `EOI` are not part of the image and most
//!   tools never look at them, which makes them a good hiding place. Several
//!   phone cameras genuinely append a second full-resolution image there.
//! - **The thumbnail.** JFIF, JFXX, EXIF IFD1 and MPF can each carry a
//!   preview. A thumbnail is generated once and often not regenerated after an
//!   edit, so a cropped photo can ship the uncropped original inside itself.

use crate::error::Error;
use crate::exif;
use crate::policy::{Orientation, Policy};
use crate::report::{Kind, Report};
use crate::util::{starts_with_ignore_ascii_case, Reader};

const FORMAT: &str = "JPEG";

// Markers that carry no length field and no payload.
const SOI: u8 = 0xD8;
const EOI: u8 = 0xD9;
const SOS: u8 = 0xDA;
const TEM: u8 = 0x01;
const RST0: u8 = 0xD0;
const RST7: u8 = 0xD7;
const COM: u8 = 0xFE;

/// Structural markers, copied through byte for byte.
///
/// `0xC0..=0xCF` is the frame-header range and includes `DHT` (0xC4) and `DAC`
/// (0xCC); `0xDB`..`0xDF` covers quantization tables, restart interval,
/// number-of-lines, hierarchical progression and expansion.
fn is_structural(marker: u8) -> bool {
    matches!(marker, 0xC0..=0xCF | 0xDB..=0xDF)
}

/// Identify an `APPn` segment from the null-terminated string at its head.
enum App {
    Jfif,
    Jfxx,
    Exif,
    Xmp,
    Iptc,
    Icc,
    Mpf,
    Adobe,
    Other,
}

fn classify(marker: u8, payload: &[u8]) -> App {
    match marker {
        0xE0 if starts_with_ignore_ascii_case(payload, b"JFIF\0") => App::Jfif,
        0xE0 if starts_with_ignore_ascii_case(payload, b"JFXX\0") => App::Jfxx,
        0xE1 if starts_with_ignore_ascii_case(payload, b"Exif\0") => App::Exif,
        0xE1 if payload.starts_with(b"http://ns.adobe.com/xap/1.0/\0") => App::Xmp,
        0xE1 if payload.starts_with(b"http://ns.adobe.com/xmp/extension/\0") => App::Xmp,
        0xE2 if payload.starts_with(b"ICC_PROFILE\0") => App::Icc,
        0xE2 if payload.starts_with(b"MPF\0") => App::Mpf,
        0xED if payload.starts_with(b"Photoshop 3.0\0") => App::Iptc,
        0xEE if payload.starts_with(b"Adobe") => App::Adobe,
        _ => App::Other,
    }
}

pub(crate) fn sanitize(
    input: &[u8],
    policy: &Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    let mut r = Reader::new(input);
    if r.take(2) != Some(&[0xFF, SOI]) {
        return Err(Error::malformed(FORMAT, "missing start-of-image marker"));
    }

    let mut out = Vec::with_capacity(input.len());
    out.extend_from_slice(&[0xFF, SOI]);

    loop {
        // Markers may be preceded by any number of 0xFF fill bytes.
        let Some(mut byte) = r.u8() else {
            report.warn(
                "the file ends without an end-of-image marker, so it was truncated \
                         before it reached us; one has been added",
            );
            out.extend_from_slice(&[0xFF, EOI]);
            break;
        };
        if byte != 0xFF {
            return Err(Error::malformed(
                FORMAT,
                format!("expected a marker at offset {}, found {byte:#04x}", r.pos() - 1),
            ));
        }
        while byte == 0xFF {
            match r.u8() {
                Some(b) => byte = b,
                None => {
                    out.extend_from_slice(&[0xFF, EOI]);
                    return Ok(out);
                }
            }
        }
        let marker = byte;

        match marker {
            EOI => {
                out.extend_from_slice(&[0xFF, EOI]);
                let trailing = r.remaining();
                if trailing > 0 {
                    // Not an error: it is common, and it is exactly the kind of
                    // thing an allowlist exists to catch.
                    report.removed(Kind::Trailer, "after EOI", trailing);
                }
                break;
            }
            // Standalone markers, legal here but carrying nothing.
            TEM | RST0..=RST7 | SOI => continue,
            _ => {}
        }

        let Some(len) = r.u16_be() else {
            return Err(Error::malformed(
                FORMAT,
                format!("truncated {marker:#04x} segment header"),
            ));
        };
        if len < 2 {
            return Err(Error::malformed(FORMAT, format!("{marker:#04x} declares length {len}")));
        }
        let Some(payload) = r.take(len as usize - 2) else {
            return Err(Error::malformed(
                FORMAT,
                format!("{marker:#04x} claims {len} bytes but the file ends first"),
            ));
        };

        if marker == SOS {
            emit(&mut out, SOS, payload);
            let entropy = scan_entropy(&mut r);
            out.extend_from_slice(entropy);
            continue;
        }

        if is_structural(marker) {
            emit(&mut out, marker, payload);
            continue;
        }

        if marker == COM {
            report.removed(Kind::Comment, "COM", payload.len());
            continue;
        }

        if !matches!(marker, 0xE0..=0xEF) {
            // Not structural, not an application segment: reserved or unknown.
            report.removed(Kind::UnknownStructure, format!("marker {marker:#04x}"), payload.len());
            continue;
        }

        handle_app(marker, payload, policy, report, &mut out);
    }

    Ok(out)
}

/// Decide what to do with one `APPn` segment.
fn handle_app(marker: u8, payload: &[u8], policy: &Policy, report: &mut Report, out: &mut Vec<u8>) {
    let app_name = format!("APP{}", marker - 0xE0);

    match classify(marker, payload) {
        // JFIF carries pixel density plus an optional uncompressed thumbnail.
        // The density is worth keeping for print sizing and identifies nobody;
        // the thumbnail has to go. Re-emitted in canonical form rather than
        // trimmed, so nothing after the fields we understand survives.
        App::Jfif => match rebuild_jfif(payload) {
            Some((canonical, thumb_bytes)) => {
                if thumb_bytes > 0 {
                    report.removed(Kind::Thumbnail, "APP0 JFIF thumbnail", thumb_bytes);
                }
                emit(out, marker, &canonical);
            }
            None => report.removed(Kind::UnknownStructure, "APP0 malformed JFIF", payload.len()),
        },

        // The JFIF extension segment exists to hold a thumbnail. Nothing else.
        App::Jfxx => report.removed(Kind::Thumbnail, "APP0 JFXX", payload.len()),

        App::Exif => {
            let findings = exif::inspect_tiff(payload.get(6..).unwrap_or_default());
            report.found_location |= findings.gps;
            report.removed(Kind::Exif, &app_name, payload.len());
            if findings.maker_note {
                report.removed(Kind::MakerNote, "APP1 EXIF maker note", 0);
            }
            if findings.thumbnail {
                report.removed(Kind::Thumbnail, "APP1 EXIF IFD1", 0);
            }

            if policy.orientation == Orientation::PreserveMinimal {
                if let Some(o) = findings.orientation {
                    // Synthesized, not copied: see exif::minimal_orientation_exif.
                    emit(out, marker, &exif::minimal_orientation_exif(o));
                }
            } else if findings.orientation.is_some_and(|o| o != 1) {
                report.warn(
                    "this photo recorded a rotation in its EXIF and that tag has been removed, \
                     so it may now display sideways; use the preserve-orientation setting if \
                     that matters more than leaving no EXIF at all",
                );
            }
        }

        App::Xmp => report.removed(Kind::Xmp, &app_name, payload.len()),
        App::Iptc => report.removed(Kind::Iptc, "APP13 Photoshop/IPTC", payload.len()),

        // The Multi-Picture Format index points at whole extra images stored in
        // the trailer, which we drop anyway.
        App::Mpf => report.removed(Kind::Thumbnail, "APP2 MPF", payload.len()),

        App::Icc => {
            if policy.keep_icc() {
                emit(out, marker, payload);
            } else {
                report.removed(Kind::ColorProfile, "APP2 ICC_PROFILE", payload.len());
            }
        }

        // The Adobe segment holds the colour transform flag. Drop it and a
        // CMYK or YCCK JPEG decodes with inverted colours, which is a
        // correctness bug rather than a privacy win, since the segment carries
        // no identifying content. Re-emitted canonically with its flag words
        // zeroed and only the transform byte carried over.
        App::Adobe => match payload.get(11) {
            Some(&transform) => {
                let mut canonical = Vec::with_capacity(12);
                canonical.extend_from_slice(b"Adobe");
                canonical.extend_from_slice(&100u16.to_be_bytes());
                canonical.extend_from_slice(&[0, 0, 0, 0, transform]);
                if payload.len() > 12 {
                    report.removed(Kind::UnknownStructure, "APP14 Adobe tail", payload.len() - 12);
                }
                emit(out, marker, &canonical);
            }
            None => report.removed(Kind::UnknownStructure, "APP14 short Adobe", payload.len()),
        },

        App::Other => report.removed(Kind::UnknownStructure, &app_name, payload.len()),
    }
}

/// Rebuild a JFIF APP0 without its thumbnail. Returns the new payload and the
/// number of thumbnail bytes dropped.
fn rebuild_jfif(payload: &[u8]) -> Option<(Vec<u8>, usize)> {
    // "JFIF\0", version(2), units(1), Xdensity(2), Ydensity(2), Xthumb, Ythumb.
    if payload.len() < 14 {
        return None;
    }
    let mut canonical = Vec::with_capacity(14);
    canonical.extend_from_slice(&payload[..12]);
    canonical.extend_from_slice(&[0, 0]); // no thumbnail
    Some((canonical, payload.len() - 14))
}

/// Consume entropy-coded data following an `SOS` header and return it.
///
/// Inside the scan, a literal `0xFF` byte is escaped as `0xFF 0x00`, and
/// restart markers `0xFF 0xD0`..`0xFF 0xD7` are part of the stream. Anything
/// else after an `0xFF` is the next real marker.
fn scan_entropy<'a>(r: &mut Reader<'a>) -> &'a [u8] {
    let start = r.pos();
    while let Some(byte) = r.u8() {
        if byte != 0xFF {
            continue;
        }
        // Peek at the byte after the 0xFF. If it is a real marker, rewind so
        // the main loop sees the whole pair.
        let marker_pos = r.pos();
        match r.u8() {
            // A stuffed zero encodes a literal 0xFF in the scan data.
            Some(0x00) => continue,
            // Restart markers belong to the scan and carry no payload.
            Some(RST0..=RST7) => continue,
            // Any number of 0xFF fill bytes may precede a real marker, so this
            // one might be the lead-in to EOI rather than scan data. Rewind onto
            // it and let the next iteration judge it on the byte that follows.
            //
            // Treating a second 0xFF as ordinary scan data instead walks past a
            // fill-padded EOI and swallows the trailer, which is precisely where
            // a payload would be hidden from a tool that only reads to EOI.
            Some(0xFF) => {
                let _ = r.seek(marker_pos);
                continue;
            }
            Some(_) | None => {
                let _ = r.seek(marker_pos - 1);
                return r.slice_from(start);
            }
        }
    }
    r.slice_from(start)
}

fn emit(out: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    out.push(0xFF);
    out.push(marker);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Assurance, Report};
    use crate::Format;

    fn seg(marker: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![0xFF, marker];
        v.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
        v.extend_from_slice(payload);
        v
    }

    /// A structurally valid JPEG skeleton: SOI, the requested segments, then a
    /// minimal scan and EOI. The entropy bytes are not decodable image data,
    /// which does not matter here because nothing in this module decodes.
    fn skeleton(segments: &[Vec<u8>]) -> Vec<u8> {
        let mut v = vec![0xFF, SOI];
        for s in segments {
            v.extend_from_slice(s);
        }
        v.extend_from_slice(&seg(0xDB, &[0u8; 65])); // DQT
        v.extend_from_slice(&seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0])); // SOF0
        v.extend_from_slice(&seg(0xC4, &[0u8; 20])); // DHT
        v.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        v.extend_from_slice(&[0x12, 0x34, 0xFF, 0x00, 0x56]); // entropy, with a stuffed FF
        v.extend_from_slice(&[0xFF, EOI]);
        v
    }

    fn run(input: &[u8], policy: &Policy) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Jpeg, input.len());
        let out = sanitize(input, policy, &mut report).expect("valid skeleton");
        (out, report)
    }

    /// A marker may be preceded by any number of `0xFF` fill bytes, so `FF FF D9`
    /// is a legal end-of-image. If the entropy scanner treats the second `0xFF`
    /// as ordinary scan data it walks straight past the EOI and swallows the
    /// trailer, which is exactly where a payload would be hidden.
    #[test]
    fn fill_bytes_before_eoi_do_not_hide_a_trailer() {
        let mut j = vec![0xFF, SOI];
        j.extend_from_slice(&seg(0xDB, &[0u8; 65]));
        j.extend_from_slice(&seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0]));
        j.extend_from_slice(&seg(0xC4, &[0u8; 20]));
        j.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        j.extend_from_slice(&[0x12, 0x34]);
        j.extend_from_slice(&[0xFF, 0xFF, EOI]); // EOI behind a fill byte
        j.extend_from_slice(b"SECRET-GPS-PAYLOAD");

        let (out, report) = run(&j, &Policy::default());

        assert!(
            !out.windows(18).any(|w| w == b"SECRET-GPS-PAYLOAD"),
            "a trailer hidden behind a fill byte survived the rebuild"
        );
        assert!(
            report.removed.iter().any(|r| r.kind == Kind::Trailer),
            "the trailer was not even reported"
        );
    }

    /// aCropalypse (CVE-2023-21036): an editor cropped an image but wrote the
    /// smaller result over the original without truncating, so the cropped-out
    /// pixels stayed on disk after the end marker and were recoverable. A file
    /// arriving here may already be in that state. The rebuild copies only up to
    /// EOI, so any recoverable tail is dropped and the output is exactly the
    /// bytes we constructed, with no leftover from the input.
    #[test]
    fn an_acropalypse_style_trailer_is_dropped_and_reported() {
        let mut j = vec![0xFF, SOI];
        j.extend_from_slice(&seg(0xDB, &[0u8; 65]));
        j.extend_from_slice(&seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0]));
        j.extend_from_slice(&seg(0xC4, &[0u8; 20]));
        j.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        j.extend_from_slice(&[0x12, 0x34]);
        j.extend_from_slice(&[0xFF, EOI]);
        // The "recoverable original" a naive crop leaves behind.
        j.extend_from_slice(b"RECOVERABLE-UNCROPPED-IMAGE-DATA");

        let (out, report) = run(&j, &Policy::default());

        assert!(
            !out.windows(31).any(|w| w == b"RECOVERABLE-UNCROPPED-IMAGE-DATA"),
            "the recoverable tail survived"
        );
        // The output ends at its own EOI, carrying nothing from the input tail.
        assert_eq!(&out[out.len() - 2..], &[0xFF, EOI]);
        assert!(report.removed.iter().any(|r| r.kind == Kind::Trailer));
    }

    /// Runs of fill bytes are legal anywhere a marker may appear.
    #[test]
    fn long_fill_runs_before_eoi_are_handled() {
        let mut j = vec![0xFF, SOI];
        j.extend_from_slice(&seg(0xDB, &[0u8; 65]));
        j.extend_from_slice(&seg(0xC0, &[8, 0, 8, 0, 8, 1, 1, 0x11, 0]));
        j.extend_from_slice(&seg(0xC4, &[0u8; 20]));
        j.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        j.extend_from_slice(&[0x12, 0x34]);
        j.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, EOI]);
        j.extend_from_slice(b"TRAILING");

        let (out, _) = run(&j, &Policy::default());
        assert!(!out.windows(8).any(|w| w == b"TRAILING"));
    }

    fn exif_app1(orientation: u16, with_gps: bool) -> Vec<u8> {
        let mut p = b"Exif\0\0".to_vec();
        p.extend_from_slice(b"MM");
        p.extend_from_slice(&42u16.to_be_bytes());
        p.extend_from_slice(&8u32.to_be_bytes());
        let entries: u16 = if with_gps { 2 } else { 1 };
        p.extend_from_slice(&entries.to_be_bytes());
        p.extend_from_slice(&0x0112u16.to_be_bytes());
        p.extend_from_slice(&3u16.to_be_bytes());
        p.extend_from_slice(&1u32.to_be_bytes());
        p.extend_from_slice(&((orientation as u32) << 16).to_be_bytes());
        if with_gps {
            let gps_at = 8 + 2 + 24 + 4;
            p.extend_from_slice(&0x8825u16.to_be_bytes());
            p.extend_from_slice(&4u16.to_be_bytes());
            p.extend_from_slice(&1u32.to_be_bytes());
            p.extend_from_slice(&(gps_at as u32).to_be_bytes());
            p.extend_from_slice(&0u32.to_be_bytes()); // no next IFD
                                                      // The GPS sub-IFD: one latitude entry.
            p.extend_from_slice(&1u16.to_be_bytes());
            p.extend_from_slice(&0x0002u16.to_be_bytes());
            p.extend_from_slice(&5u16.to_be_bytes());
            p.extend_from_slice(&3u32.to_be_bytes());
            p.extend_from_slice(&0u32.to_be_bytes());
            p.extend_from_slice(&0u32.to_be_bytes());
        } else {
            p.extend_from_slice(&0u32.to_be_bytes());
        }
        seg(0xE1, &p)
    }

    #[test]
    fn exif_xmp_iptc_and_comments_all_go() {
        let input = skeleton(&[
            exif_app1(1, false),
            seg(0xE1, b"http://ns.adobe.com/xap/1.0/\0<x:xmpmeta/>"),
            seg(0xED, b"Photoshop 3.0\08BIM\x04\x04secret caption"),
            seg(COM, b"taken with my phone"),
        ]);
        let (out, report) = run(&input, &Policy::default());

        for needle in [&b"Exif\0\0"[..], b"ns.adobe.com", b"Photoshop 3.0", b"taken with my phone"]
        {
            assert!(
                !out.windows(needle.len()).any(|w| w == needle),
                "{:?} survived",
                String::from_utf8_lossy(needle)
            );
        }
        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Exif));
        assert!(kinds.contains(&Kind::Xmp));
        assert!(kinds.contains(&Kind::Iptc));
        assert!(kinds.contains(&Kind::Comment));
        assert_eq!(report.assurance, Assurance::Complete);
    }

    #[test]
    fn gps_is_surfaced_on_its_own() {
        let (_, report) = run(&skeleton(&[exif_app1(1, true)]), &Policy::default());
        assert!(report.found_location);
        let (_, report) = run(&skeleton(&[exif_app1(1, false)]), &Policy::default());
        assert!(!report.found_location);
    }

    #[test]
    fn the_compressed_scan_is_copied_bit_for_bit() {
        // The point of rebuilding rather than re-encoding: no quality loss.
        let input = skeleton(&[exif_app1(6, true), seg(COM, b"x")]);
        let (out, _) = run(&input, &Policy::default());
        let entropy = &[0x12u8, 0x34, 0xFF, 0x00, 0x56];
        assert!(out.windows(entropy.len()).any(|w| w == entropy));
        // Structural segments survive too.
        assert!(out.windows(4).any(|w| w == [0xFF, 0xC0, 0x00, 0x0B]));
    }

    #[test]
    fn an_unknown_application_segment_is_dropped_without_being_understood() {
        // The whole argument for an allowlist. APP5 is not a thing we parse.
        let input = skeleton(&[seg(0xE5, b"VendorPrivate\0serial=ABC123")]);
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(6).any(|w| w == b"ABC123"));
        assert_eq!(report.removed[0].kind, Kind::UnknownStructure);
        assert_eq!(report.removed[0].location, "APP5");
    }

    #[test]
    fn data_appended_after_end_of_image_is_dropped() {
        let mut input = skeleton(&[]);
        input.extend_from_slice(b"appended secret payload");
        let (out, report) = run(&input, &Policy::default());
        assert!(out.ends_with(&[0xFF, EOI]));
        assert!(!out.windows(6).any(|w| w == b"secret"));
        assert_eq!(report.removed.iter().filter(|r| r.kind == Kind::Trailer).count(), 1);
    }

    #[test]
    fn jfif_density_survives_but_its_thumbnail_does_not() {
        let mut jfif = b"JFIF\0".to_vec();
        jfif.extend_from_slice(&[1, 1]); // version 1.01
        jfif.push(1); // units: dots per inch
        jfif.extend_from_slice(&300u16.to_be_bytes());
        jfif.extend_from_slice(&300u16.to_be_bytes());
        jfif.extend_from_slice(&[2, 2]); // a 2x2 thumbnail follows
        jfif.extend_from_slice(&[0xAA; 12]);

        let (out, report) = run(&skeleton(&[seg(0xE0, &jfif)]), &Policy::default());
        assert!(out.windows(4).any(|w| w == 300u16.to_be_bytes().repeat(2).as_slice()));
        assert!(!out.windows(12).any(|w| w == [0xAA; 12]));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Thumbnail));
    }

    #[test]
    fn the_adobe_colour_transform_is_preserved_but_rebuilt() {
        let mut adobe = b"Adobe".to_vec();
        adobe.extend_from_slice(&[0x00, 0x64, 0xDE, 0xAD, 0xBE, 0xEF, 2]);
        adobe.extend_from_slice(b"trailing junk");

        let (out, report) = run(&skeleton(&[seg(0xEE, &adobe)]), &Policy::default());
        let at = out.windows(5).position(|w| w == b"Adobe").expect("Adobe segment kept");
        assert_eq!(out[at + 11], 2, "the transform byte must survive");
        assert_eq!(&out[at + 7..at + 11], &[0, 0, 0, 0], "flag words are zeroed");
        assert!(!out.windows(13).any(|w| w == b"trailing junk"));
        assert!(report.removed.iter().any(|r| r.location == "APP14 Adobe tail"));
    }

    #[test]
    fn icc_follows_the_policy() {
        let icc = {
            let mut p = b"ICC_PROFILE\0".to_vec();
            p.extend_from_slice(&[1, 1]);
            p.extend_from_slice(b"profile-body-here");
            p
        };
        let (dropped, report) = run(&skeleton(&[seg(0xE2, &icc)]), &Policy::strict());
        assert!(!dropped.windows(11).any(|w| w == b"ICC_PROFILE"));
        assert!(report.removed.iter().any(|r| r.kind == Kind::ColorProfile));

        let (kept, _) = run(&skeleton(&[seg(0xE2, &icc)]), &Policy::preserve_appearance());
        assert!(kept.windows(17).any(|w| w == b"profile-body-here"));
    }

    #[test]
    fn orientation_can_be_carried_over_in_a_freshly_built_block() {
        let input = skeleton(&[exif_app1(6, true)]);
        let (out, report) = run(&input, &Policy::preserve_appearance());

        // A rebuilt EXIF block is present and holds only the orientation.
        let at = out.windows(6).position(|w| w == b"Exif\0\0").expect("minimal EXIF emitted");
        let block_len = u16::from_be_bytes([out[at - 2], out[at - 1]]) as usize;
        assert_eq!(block_len, 32 + 2, "only the synthesized 32-byte block");
        let found = exif::inspect_tiff(&out[at + 6..at + 32]);
        assert_eq!(found.orientation, Some(6));
        assert!(!found.gps, "the GPS sub-IFD must not come across");
        assert!(report.found_location, "and it is still reported as having been there");
    }

    #[test]
    fn dropping_a_real_rotation_warns_the_user() {
        let (_, rotated) = run(&skeleton(&[exif_app1(6, false)]), &Policy::strict());
        assert!(rotated.warnings.iter().any(|w| w.contains("sideways")));

        // Orientation 1 is "already upright", so there is nothing to warn about.
        let (_, upright) = run(&skeleton(&[exif_app1(1, false)]), &Policy::strict());
        assert!(upright.warnings.is_empty());
    }

    #[test]
    fn restart_markers_inside_the_scan_are_not_mistaken_for_the_next_segment() {
        let mut input = vec![0xFF, SOI];
        input.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        input.extend_from_slice(&[0x11, 0xFF, RST0, 0x22, 0xFF, RST7, 0x33]);
        input.extend_from_slice(&[0xFF, EOI]);

        let (out, report) = run(&input, &Policy::default());
        assert!(out.windows(7).any(|w| w == [0x11, 0xFF, RST0, 0x22, 0xFF, RST7, 0x33]));
        assert!(report.removed.is_empty());
    }

    #[test]
    fn fill_bytes_before_a_marker_are_tolerated() {
        let mut input = vec![0xFF, SOI, 0xFF, 0xFF, 0xFF];
        input.extend_from_slice(&seg(COM, b"drop me"));
        input.extend_from_slice(&[0xFF, EOI]);
        let (out, report) = run(&input, &Policy::default());
        assert!(!out.windows(7).any(|w| w == b"drop me"));
        assert_eq!(report.removed[0].kind, Kind::Comment);
    }

    #[test]
    fn a_bad_header_is_an_error_rather_than_a_half_strip() {
        let mut report = Report::new(Format::Jpeg, 0);
        assert!(sanitize(b"not a jpeg", &Policy::default(), &mut report).is_err());

        // A segment claiming more bytes than the file holds.
        let mut input = vec![0xFF, SOI, 0xFF, 0xE1];
        input.extend_from_slice(&9999u16.to_be_bytes());
        input.extend_from_slice(b"short");
        assert!(sanitize(&input, &Policy::default(), &mut report).is_err());

        // A segment whose declared length cannot even cover the length field.
        let bad = vec![0xFF, SOI, 0xFF, 0xE1, 0x00, 0x01];
        assert!(sanitize(&bad, &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = skeleton(&[exif_app1(6, true), seg(COM, b"hello"), seg(0xE5, b"vendor")]);
        for n in 0..full.len() {
            let mut report = Report::new(Format::Jpeg, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }

    #[test]
    fn a_truncated_scan_still_produces_a_terminated_file() {
        let mut input = vec![0xFF, SOI];
        input.extend_from_slice(&seg(SOS, &[1, 1, 0, 0, 63, 0]));
        input.extend_from_slice(&[0x11, 0x22, 0x33]); // ends mid-scan
        let (out, report) = run(&input, &Policy::default());
        assert!(out.ends_with(&[0xFF, EOI]));
        assert!(report.warnings.iter().any(|w| w.contains("truncated")));
    }
}
