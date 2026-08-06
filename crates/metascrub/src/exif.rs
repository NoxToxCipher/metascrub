//! Read-only inspection of an EXIF/TIFF tag block, plus synthesis of a minimal
//! replacement.
//!
//! This deliberately does not edit EXIF. Editing a tag block in place means
//! trusting that every offset, every nested image file directory and every
//! vendor's private encoding was understood correctly, and a mistake leaves
//! data in the file while reporting success. The parsers here only *read*, to
//! answer two questions for the report ("were there GPS coordinates?", "which
//! way up is this?"), and the block itself is dropped whole.
//!
//! Everything is bounds-checked and offset-following is depth-limited, because
//! a TIFF header is a pointer structure supplied by whoever sent the file: an
//! image file directory can point at itself.

use crate::util::Reader;

/// Tags we look for, all in IFD0 unless noted.
const TAG_ORIENTATION: u16 = 0x0112;
const TAG_EXIF_IFD: u16 = 0x8769;
const TAG_GPS_IFD: u16 = 0x8825;
const TAG_MAKER_NOTE: u16 = 0x927C; // in the Exif sub-IFD
const TAG_GPS_LATITUDE: u16 = 0x0002; // in the GPS sub-IFD
const TAG_GPS_LONGITUDE: u16 = 0x0004;

/// How deep to follow sub-IFD pointers. Real files use two levels; more than
/// this is either broken or an attempt to make us walk in circles.
const MAX_IFD_DEPTH: u8 = 4;

/// What a TIFF block turned out to contain.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ExifFindings {
    /// A GPS sub-IFD was present and held an actual latitude or longitude.
    pub gps: bool,
    /// A vendor maker note was present.
    pub maker_note: bool,
    /// A second image file directory, which is where the embedded thumbnail
    /// lives.
    pub thumbnail: bool,
    /// The orientation tag's value, 1 through 8.
    pub orientation: Option<u16>,
}

/// Inspect a raw TIFF block: byte-order mark, `42`, offset to IFD0.
///
/// `tiff` must start at the byte-order mark, so a JPEG caller passes the APP1
/// payload with its leading `Exif\0\0` already removed.
pub(crate) fn inspect_tiff(tiff: &[u8]) -> ExifFindings {
    let mut found = ExifFindings::default();
    let mut r = Reader::new(tiff);

    let big = match r.take(2) {
        Some(b"MM") => true,
        Some(b"II") => false,
        _ => return found,
    };
    if r.u16_endian(big) != Some(42) {
        return found;
    }
    let Some(ifd0) = r.u32_endian(big) else { return found };

    walk_ifd(tiff, ifd0 as usize, big, 0, &mut found);
    found
}

/// Walk one image file directory, following the sub-IFD pointers we care about.
fn walk_ifd(tiff: &[u8], offset: usize, big: bool, depth: u8, found: &mut ExifFindings) {
    if depth >= MAX_IFD_DEPTH {
        return;
    }
    let mut r = Reader::new(tiff);
    if r.seek(offset).is_none() {
        return;
    }
    let Some(count) = r.u16_endian(big) else { return };

    // Each entry is 12 bytes: tag, type, count, then either the value inline or
    // an offset to it.
    for _ in 0..count {
        let (Some(tag), Some(_ty), Some(_n), Some(value)) =
            (r.u16_endian(big), r.u16_endian(big), r.u32_endian(big), r.u32_endian(big))
        else {
            return;
        };
        match tag {
            TAG_ORIENTATION => {
                // A SHORT sits in the high half of the value field on a
                // big-endian file and the low half on a little-endian one.
                let v = if big { (value >> 16) as u16 } else { value as u16 };
                if (1..=8).contains(&v) {
                    found.orientation = Some(v);
                }
            }
            TAG_EXIF_IFD | TAG_GPS_IFD => {
                walk_ifd(tiff, value as usize, big, depth + 1, found);
            }
            TAG_MAKER_NOTE => found.maker_note = true,
            TAG_GPS_LATITUDE | TAG_GPS_LONGITUDE => found.gps = true,
            _ => {}
        }
    }

    // A non-zero "next IFD" pointer at depth 0 is IFD1, the thumbnail.
    if let Some(next) = r.u32_endian(big) {
        if next != 0 && depth == 0 {
            found.thumbnail = true;
        }
    }
}

/// Build an EXIF APP1 payload holding the orientation tag and nothing else.
///
/// Synthesized from scratch rather than filtered from the original, so there is
/// no path by which an unparsed byte of the input can reach the output. The
/// result is 32 bytes: `Exif\0\0`, a big-endian TIFF header, one directory
/// entry, and a null next-IFD pointer.
pub(crate) fn minimal_orientation_exif(orientation: u16) -> Vec<u8> {
    let orientation = if (1..=8).contains(&orientation) { orientation } else { 1 };
    let mut out = Vec::with_capacity(32);
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(b"MM"); // big-endian
    out.extend_from_slice(&42u16.to_be_bytes());
    out.extend_from_slice(&8u32.to_be_bytes()); // IFD0 begins right after
    out.extend_from_slice(&1u16.to_be_bytes()); // one entry
    out.extend_from_slice(&TAG_ORIENTATION.to_be_bytes());
    out.extend_from_slice(&3u16.to_be_bytes()); // type SHORT
    out.extend_from_slice(&1u32.to_be_bytes()); // count
    out.extend_from_slice(&orientation.to_be_bytes());
    out.extend_from_slice(&[0, 0]); // pad the 4-byte value field
    out.extend_from_slice(&0u32.to_be_bytes()); // no next IFD
    out
}

/// A valid, empty TIFF block, for containers where metadata is overwritten in
/// place rather than excised and a decoder may still try to parse it.
pub(crate) fn empty_tiff() -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..2].copy_from_slice(b"MM");
    out[2..4].copy_from_slice(&42u16.to_be_bytes());
    out[4..8].copy_from_slice(&8u32.to_be_bytes()); // IFD0 offset, past the end
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a TIFF block with the given IFD0 entries and optional sub-IFDs.
    /// Entries are `(tag, type, count, value)` with the value already in the
    /// 4-byte inline form.
    fn tiff(big: bool, entries: &[(u16, u16, u32, u32)], next_ifd: u32) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(if big { b"MM" } else { b"II" });
        let put16 = |o: &mut Vec<u8>, v: u16| {
            if big {
                o.extend_from_slice(&v.to_be_bytes())
            } else {
                o.extend_from_slice(&v.to_le_bytes())
            }
        };
        let put32 = |o: &mut Vec<u8>, v: u32| {
            if big {
                o.extend_from_slice(&v.to_be_bytes())
            } else {
                o.extend_from_slice(&v.to_le_bytes())
            }
        };
        put16(&mut out, 42);
        put32(&mut out, 8);
        put16(&mut out, entries.len() as u16);
        for &(tag, ty, count, value) in entries {
            put16(&mut out, tag);
            put16(&mut out, ty);
            put32(&mut out, count);
            put32(&mut out, value);
        }
        put32(&mut out, next_ifd);
        out
    }

    #[test]
    fn orientation_is_read_in_both_byte_orders() {
        for big in [true, false] {
            // A SHORT's inline value is left-justified in the 4-byte field.
            let value = if big { 6 << 16 } else { 6 };
            let t = tiff(big, &[(TAG_ORIENTATION, 3, 1, value)], 0);
            assert_eq!(inspect_tiff(&t).orientation, Some(6), "big={big}");
        }
    }

    #[test]
    fn nonsense_orientation_values_are_ignored() {
        let t = tiff(true, &[(TAG_ORIENTATION, 3, 1, 99 << 16)], 0);
        assert_eq!(inspect_tiff(&t).orientation, None);
    }

    #[test]
    fn gps_is_found_through_the_sub_ifd_pointer() {
        // IFD0 holds a pointer; the GPS IFD it points at holds a latitude.
        let gps_ifd = tiff(true, &[(TAG_GPS_LATITUDE, 5, 3, 0)], 0);
        let gps_body = &gps_ifd[8..]; // strip the header, keep the directory

        let mut t = tiff(true, &[(TAG_GPS_IFD, 4, 1, 0)], 0);
        let gps_offset = t.len() as u32;
        t.extend_from_slice(gps_body);
        // Patch the pointer now that we know where the sub-IFD landed.
        let value_pos = 8 + 2 + 8;
        t[value_pos..value_pos + 4].copy_from_slice(&gps_offset.to_be_bytes());

        let found = inspect_tiff(&t);
        assert!(found.gps, "a GPS sub-IFD holding a latitude must be reported");
    }

    #[test]
    fn a_gps_pointer_with_no_coordinates_is_not_a_location() {
        // Some cameras write an empty GPS IFD. Reporting that as "this photo
        // has your coordinates" would train users to ignore the warning.
        let mut t = tiff(true, &[(TAG_GPS_IFD, 4, 1, 0)], 0);
        let empty_ifd_offset = t.len() as u32;
        t.extend_from_slice(&0u16.to_be_bytes()); // zero entries
        t.extend_from_slice(&0u32.to_be_bytes());
        let value_pos = 8 + 2 + 8;
        t[value_pos..value_pos + 4].copy_from_slice(&empty_ifd_offset.to_be_bytes());

        assert!(!inspect_tiff(&t).gps);
    }

    #[test]
    fn a_second_ifd_is_reported_as_a_thumbnail() {
        let t = tiff(true, &[(TAG_ORIENTATION, 3, 1, 1 << 16)], 128);
        assert!(inspect_tiff(&t).thumbnail);
    }

    #[test]
    fn a_self_referential_ifd_terminates() {
        // IFD0 points its Exif sub-IFD pointer back at itself. Without the
        // depth limit this recurses until the stack runs out.
        let t = tiff(true, &[(TAG_EXIF_IFD, 4, 1, 8)], 0);
        let _ = inspect_tiff(&t);
    }

    #[test]
    fn garbage_and_truncation_never_panic() {
        assert_eq!(inspect_tiff(b""), ExifFindings::default());
        assert_eq!(inspect_tiff(b"MM"), ExifFindings::default());
        assert_eq!(inspect_tiff(b"XX\x00\x2a\x00\x00\x00\x08"), ExifFindings::default());

        let t = tiff(true, &[(TAG_GPS_IFD, 4, 1, 0xFFFF_FFFF)], 0xFFFF_FFFF);
        let _ = inspect_tiff(&t);
        for n in 0..t.len() {
            let _ = inspect_tiff(&t[..n]);
        }
    }

    #[test]
    fn the_synthesized_block_reads_back_as_orientation_only() {
        for o in 1..=8u16 {
            let block = minimal_orientation_exif(o);
            assert_eq!(block.len(), 32);
            assert!(block.starts_with(b"Exif\0\0"));
            let found = inspect_tiff(&block[6..]);
            assert_eq!(found.orientation, Some(o));
            assert!(!found.gps && !found.maker_note && !found.thumbnail);
        }
        // Out-of-range input is clamped, not propagated.
        assert_eq!(inspect_tiff(&minimal_orientation_exif(0)[6..]).orientation, Some(1));
    }

    #[test]
    fn the_empty_block_parses_and_holds_nothing() {
        assert_eq!(inspect_tiff(&empty_tiff()), ExifFindings::default());
    }
}
