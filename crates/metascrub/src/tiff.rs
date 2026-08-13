//! TIFF: rebuild each image file directory from an allowlist of structural tags.
//!
//! TIFF is a directory of tagged entries, some of which point (by file offset)
//! at the pixel data. Metadata — the camera make and model, the timestamp, the
//! GPS sub-directory, XMP, IPTC, the embedded thumbnail — lives in the same
//! directory as the handful of tags that actually describe how to decode the
//! image. This module keeps only that handful and copies the pixel data those
//! tags reference, byte for byte, into a freshly built file. Everything not on
//! the keep-list is gone because it was never written to the output, not because
//! it was found and deleted.
//!
//! Because the pixel data is copied verbatim (no re-compression, no re-encode)
//! the picture is bit-for-bit identical; only the container is new. That makes
//! this a [`Complete`](crate::Assurance::Complete) rebuild.
//!
//! Multi-page TIFFs (faxes, scanned documents) keep every page. The only
//! directories dropped whole are those flagged as reduced-resolution — the
//! thumbnail a camera embeds — which are pure metadata by another name.

use crate::error::Error;
use crate::report::{Kind, Report};

const FORMAT: &str = "TIFF";

// Tags that describe how to decode the image. Everything else is dropped.
const T_NEW_SUBFILE_TYPE: u16 = 254;
const T_SUBFILE_TYPE: u16 = 255;
const T_IMAGE_WIDTH: u16 = 256;
const T_IMAGE_LENGTH: u16 = 257;
const T_BITS_PER_SAMPLE: u16 = 258;
const T_COMPRESSION: u16 = 259;
const T_PHOTOMETRIC: u16 = 262;
const T_THRESHOLDING: u16 = 263;
const T_FILL_ORDER: u16 = 266;
const T_STRIP_OFFSETS: u16 = 273;
const T_ORIENTATION: u16 = 274;
const T_SAMPLES_PER_PIXEL: u16 = 277;
const T_ROWS_PER_STRIP: u16 = 278;
const T_STRIP_BYTE_COUNTS: u16 = 279;
const T_X_RESOLUTION: u16 = 282;
const T_Y_RESOLUTION: u16 = 283;
const T_PLANAR_CONFIG: u16 = 284;
const T_RESOLUTION_UNIT: u16 = 296;
const T_PREDICTOR: u16 = 317;
const T_COLOR_MAP: u16 = 320;
const T_TILE_WIDTH: u16 = 322;
const T_TILE_LENGTH: u16 = 323;
const T_TILE_OFFSETS: u16 = 324;
const T_TILE_BYTE_COUNTS: u16 = 325;
const T_EXTRA_SAMPLES: u16 = 338;
const T_SAMPLE_FORMAT: u16 = 339;
const T_JPEG_TABLES: u16 = 347;

const KEEP: &[u16] = &[
    T_NEW_SUBFILE_TYPE, T_SUBFILE_TYPE, T_IMAGE_WIDTH, T_IMAGE_LENGTH, T_BITS_PER_SAMPLE,
    T_COMPRESSION, T_PHOTOMETRIC, T_THRESHOLDING, T_FILL_ORDER, T_STRIP_OFFSETS, T_ORIENTATION,
    T_SAMPLES_PER_PIXEL, T_ROWS_PER_STRIP, T_STRIP_BYTE_COUNTS, T_X_RESOLUTION, T_Y_RESOLUTION,
    T_PLANAR_CONFIG, T_RESOLUTION_UNIT, T_PREDICTOR, T_COLOR_MAP, T_TILE_WIDTH, T_TILE_LENGTH,
    T_TILE_OFFSETS, T_TILE_BYTE_COUNTS, T_EXTRA_SAMPLES, T_SAMPLE_FORMAT, T_JPEG_TABLES,
];

/// A raw 12-byte directory entry, as read.
#[derive(Clone, Copy)]
struct Entry {
    tag: u16,
    ty: u16,
    count: u32,
    value: [u8; 4],
}

fn type_size(ty: u16) -> Option<u32> {
    Some(match ty {
        1 | 2 | 6 | 7 => 1,  // BYTE, ASCII, SBYTE, UNDEFINED
        3 | 8 => 2,          // SHORT, SSHORT
        4 | 9 | 13 => 4,     // LONG, SLONG, IFD
        5 | 10 => 8,         // RATIONAL, SRATIONAL
        11 => 4,             // FLOAT
        12 => 8,             // DOUBLE
        _ => return None,
    })
}

struct Rd {
    big: bool,
}
impl Rd {
    fn u16(&self, b: &[u8]) -> u16 {
        if self.big { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) }
    }
    fn u32(&self, b: &[u8]) -> u32 {
        if self.big {
            u32::from_be_bytes([b[0], b[1], b[2], b[3]])
        } else {
            u32::from_le_bytes([b[0], b[1], b[2], b[3]])
        }
    }
    fn put16(&self, out: &mut Vec<u8>, v: u16) {
        if self.big { out.extend_from_slice(&v.to_be_bytes()) } else { out.extend_from_slice(&v.to_le_bytes()) }
    }
    fn put32(&self, out: &mut Vec<u8>, v: u32) {
        if self.big { out.extend_from_slice(&v.to_be_bytes()) } else { out.extend_from_slice(&v.to_le_bytes()) }
    }
}

pub(crate) fn sanitize(input: &[u8], policy: &crate::Policy, report: &mut Report) -> crate::Result<Vec<u8>> {
    if input.len() < 8 {
        return Err(Error::malformed(FORMAT, "shorter than a TIFF header"));
    }
    let rd = Rd {
        big: match &input[0..2] {
            b"MM" => true,
            b"II" => false,
            _ => return Err(Error::malformed(FORMAT, "no byte-order mark")),
        },
    };
    if rd.u16(&input[2..4]) != 42 {
        return Err(Error::malformed(FORMAT, "not classic TIFF"));
    }

    // Collect the IFD chain, guarding against loops and runaway length.
    let mut ifd_offsets = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    let mut next = rd.u32(&input[4..8]) as usize;
    while next != 0 {
        if !seen.insert(next) || ifd_offsets.len() >= 256 {
            break; // a directory that points back into the chain, or too many
        }
        ifd_offsets.push(next);
        let Some(entries_end) = ifd_entry_table_end(input, next, &rd) else { break };
        next = rd.u32(input.get(entries_end..entries_end + 4).ok_or_else(|| Error::malformed(FORMAT, "truncated IFD chain"))?) as usize;
    }
    if ifd_offsets.is_empty() {
        return Err(Error::malformed(FORMAT, "no image directory"));
    }

    // A camera raw (DNG, NEF, ARW, ...) is a TIFF whose real, full-resolution
    // image lives in a sub-directory this generic rebuild does not follow, with
    // the top directory holding only a small preview. Rebuilding one would drop
    // the actual photograph and leave a corrupt raw the user cannot re-shoot.
    // These are declined and returned untouched, the honest outcome for a file
    // we cannot safely take apart.
    if let Some(first) = read_entries(input, ifd_offsets[0], &rd) {
        if let Some(reason) = looks_like_camera_raw(&first, &rd) {
            report.assurance = crate::Assurance::None;
            report.warn(format!(
                "this looks like a camera raw ({reason}); its full-resolution image is stored in a \
                 way a generic TIFF rebuild would corrupt, so it was left exactly as it arrived and \
                 nothing was removed. Convert it to a normal image first if you need it scrubbed",
            ));
            return Ok(input.to_vec());
        }
    }

    // Build the output: 8-byte header, then each kept directory in turn.
    let mut out = Vec::with_capacity(input.len());
    out.extend_from_slice(&input[0..2]);
    rd.put16(&mut out, 42);
    rd.put32(&mut out, 0); // first-IFD offset, patched once we know it

    let mut dropped_unknown = 0usize;
    let mut prev_next_ptr_pos: Option<usize> = None;
    let mut first_ifd_pos: Option<usize> = None;

    for (idx, &off) in ifd_offsets.iter().enumerate() {
        let entries = read_entries(input, off, &rd).ok_or_else(|| Error::malformed(FORMAT, "unreadable directory"))?;

        // A reduced-resolution directory after the first is the embedded
        // thumbnail; drop it whole.
        if idx > 0 && is_reduced_resolution(&entries, &rd) {
            report.removed(Kind::Thumbnail, "reduced-resolution page", 0);
            continue;
        }

        // Any KEPT page whose strips are JPEG datastreams (compression 6 or 7)
        // stores each strip as a self-contained JPEG that can carry its own
        // APP1/Exif+GPS, which this rebuild copies opaquely. Checked per page,
        // not just on the first directory: a multi-page TIFF can hide the JPEG
        // page behind a plain first page. Downgrade and disclose once.
        if report.assurance == crate::Assurance::Complete && is_jpeg_compressed(&entries, &rd) {
            report.assurance = crate::Assurance::BestEffort;
            report.warn(
                "this TIFF stores image data as embedded JPEG; the directory metadata was removed, \
                 but a JPEG strip can carry its own EXIF or GPS that a TIFF rebuild copies through \
                 untouched. For a full clean, convert it to a normal JPEG or PNG and scrub that",
            );
        }

        // Pad to an even offset: TIFF requires word alignment.
        if out.len() % 2 == 1 {
            out.push(0);
        }
        let ifd_start = out.len();
        if first_ifd_pos.is_none() {
            first_ifd_pos = Some(ifd_start);
        }
        // Chain the previous directory's next-pointer to this one.
        if let Some(pos) = prev_next_ptr_pos.take() {
            let v = ifd_start as u32;
            out[pos..pos + 4].copy_from_slice(&if rd.big { v.to_be_bytes() } else { v.to_le_bytes() });
        }

        let next_ptr_pos = build_ifd(input, &entries, &rd, policy, &mut out, report, &mut dropped_unknown)?;
        prev_next_ptr_pos = Some(next_ptr_pos);
    }

    // The last kept directory ends the chain.
    if let Some(pos) = prev_next_ptr_pos {
        out[pos..pos + 4].copy_from_slice(&0u32.to_ne_bytes());
    }
    // Patch the header's first-IFD offset.
    let first = first_ifd_pos.ok_or_else(|| Error::malformed(FORMAT, "every directory was a thumbnail"))?;
    let v = first as u32;
    out[4..8].copy_from_slice(&if rd.big { v.to_be_bytes() } else { v.to_le_bytes() });

    if dropped_unknown > 0 {
        report.removed(Kind::UnknownStructure, format!("{dropped_unknown} non-structural tag(s)"), 0);
    }

    Ok(out)
}

/// The byte offset just past an IFD's 12-byte entry table (where its next-IFD
/// pointer sits), or None if the count runs off the end.
fn ifd_entry_table_end(input: &[u8], off: usize, rd: &Rd) -> Option<usize> {
    let count = rd.u16(input.get(off..off + 2)?) as usize;
    let end = off.checked_add(2)?.checked_add(count.checked_mul(12)?)?;
    (end + 4 <= input.len()).then_some(end)
}

fn read_entries(input: &[u8], off: usize, rd: &Rd) -> Option<Vec<Entry>> {
    let count = rd.u16(input.get(off..off + 2)?) as usize;
    let mut entries = Vec::with_capacity(count);
    let mut p = off + 2;
    for _ in 0..count {
        let e = input.get(p..p + 12)?;
        entries.push(Entry {
            tag: rd.u16(&e[0..2]),
            ty: rd.u16(&e[2..4]),
            count: rd.u32(&e[4..8]),
            value: [e[8], e[9], e[10], e[11]],
        });
        p += 12;
    }
    Some(entries)
}

/// Whether IFD0 carries the fingerprints of a camera raw, and which one.
///
/// The check is deliberately eager: a false positive means a plain TIFF is
/// returned untouched (still safe, just not scrubbed), whereas a false negative
/// means a raw is silently mangled. When in doubt, decline.
fn looks_like_camera_raw(entries: &[Entry], rd: &Rd) -> Option<&'static str> {
    const T_SUB_IFDS: u16 = 330;
    const T_CFA_PATTERN_DIM: u16 = 33421;
    const T_CFA_PATTERN: u16 = 33422;
    const T_DNG_VERSION: u16 = 50706;
    const PHOTOMETRIC_CFA: u16 = 32803; // colour filter array = sensor data
    const PHOTOMETRIC_LINEAR_RAW: u16 = 34892;

    for e in entries {
        match e.tag {
            T_DNG_VERSION => return Some("DNG"),
            T_SUB_IFDS => return Some("has raw sub-images"),
            T_CFA_PATTERN | T_CFA_PATTERN_DIM => return Some("colour-filter-array data"),
            T_PHOTOMETRIC => {
                let v = if e.count == 1 { rd_short(e, rd) } else { None };
                if v == Some(PHOTOMETRIC_CFA) || v == Some(PHOTOMETRIC_LINEAR_RAW) {
                    return Some("sensor colour-filter-array image");
                }
            }
            _ => {}
        }
    }
    None
}

/// Read a SHORT (or LONG) value that sits inline in an entry.
///
/// A LONG that does not fit in 16 bits is `None`, not a truncation: `as u16`
/// could drop the high word and make an unrelated photometric value alias to a
/// raw sentinel (32803 / 34892), misclassifying a plain TIFF as a camera raw.
fn rd_short(e: &Entry, rd: &Rd) -> Option<u16> {
    match e.ty {
        3 => Some(rd.u16(&e.value[0..2])),
        4 => u16::try_from(rd.u32(&e.value)).ok(),
        _ => None,
    }
}

/// True if the directory declares JPEG compression (old-style 6 or new-style 7),
/// whose strip/tile data is a self-contained JPEG this rebuild copies opaquely.
///
/// Reads the Compression value across BYTE/SHORT/LONG types rather than through
/// `rd_short` (SHORT/LONG only), so a crafted BYTE-typed compression tag cannot
/// slip a JPEG page past the check.
fn is_jpeg_compressed(entries: &[Entry], rd: &Rd) -> bool {
    entries.iter().any(|e| {
        e.tag == T_COMPRESSION
            && matches!(
                match e.ty {
                    1 => e.value.first().map(|&b| b as u32),
                    3 => Some(rd.u16(&e.value[0..2]) as u32),
                    4 => Some(rd.u32(&e.value)),
                    _ => None,
                },
                Some(6) | Some(7)
            )
    })
}

fn is_reduced_resolution(entries: &[Entry], rd: &Rd) -> bool {
    entries
        .iter()
        .find(|e| e.tag == T_NEW_SUBFILE_TYPE)
        .map(|e| rd.u32(&e.value) & 0x1 != 0)
        .unwrap_or(false)
}

/// Read an entry whose value is an array of unsigned integers (SHORT or LONG),
/// resolving an out-of-line pointer if the value does not fit inline.
fn read_uint_array(input: &[u8], e: &Entry, rd: &Rd) -> Option<Vec<u64>> {
    let size = type_size(e.ty)?;
    let byte_len = (size as u64).checked_mul(e.count as u64)?;
    let raw: Vec<u8> = if byte_len <= 4 {
        e.value[..byte_len as usize].to_vec()
    } else {
        let off = rd.u32(&e.value) as usize;
        input.get(off..off + byte_len as usize)?.to_vec()
    };
    let mut vals = Vec::with_capacity(e.count as usize);
    for chunk in raw.chunks_exact(size as usize) {
        let v = match size {
            2 => rd.u16(chunk) as u64,
            4 => rd.u32(chunk) as u64,
            1 => chunk[0] as u64,
            _ => return None,
        };
        vals.push(v);
    }
    Some(vals)
}

/// The raw value bytes of an entry (inline or followed to its offset).
fn value_bytes(input: &[u8], e: &Entry, rd: &Rd) -> Option<Vec<u8>> {
    let size = type_size(e.ty)?;
    let byte_len = (size as u64).checked_mul(e.count as u64)? as usize;
    if byte_len <= 4 {
        Some(e.value[..byte_len].to_vec())
    } else {
        let off = rd.u32(&e.value) as usize;
        input.get(off..off + byte_len).map(|s| s.to_vec())
    }
}

/// Serialise one kept directory (its entry table, its out-of-line values, and
/// the pixel data it references) onto the end of `out`. Returns the absolute
/// position of the 4-byte next-IFD pointer, for the caller to chain or zero.
fn build_ifd(
    input: &[u8],
    entries: &[Entry],
    rd: &Rd,
    policy: &crate::Policy,
    out: &mut Vec<u8>,
    report: &mut Report,
    dropped_unknown: &mut usize,
) -> crate::Result<usize> {
    const T_ICC_PROFILE: u16 = 34675;
    let ifd_start = out.len();

    // Partition the source entries: structural ones to keep, and note the
    // pixel-locating pair (strips or tiles). Report the rest as removed.
    let mut kept: Vec<Entry> = Vec::new();
    let mut offsets_entry: Option<Entry> = None;
    let mut counts_entry: Option<Entry> = None;
    let mut is_tiled = false;

    for e in entries {
        match e.tag {
            T_STRIP_OFFSETS => offsets_entry = Some(*e),
            T_STRIP_BYTE_COUNTS => counts_entry = Some(*e),
            T_TILE_OFFSETS => {
                offsets_entry = Some(*e);
                is_tiled = true;
            }
            T_TILE_BYTE_COUNTS => counts_entry = Some(*e),
            // Orientation is structural but privacy policy decides its fate, to
            // match how the JPEG path treats it: dropped by default so no tag
            // survives, kept when the caller wants the picture to display upright.
            T_ORIENTATION => {
                if policy.orientation == crate::Orientation::PreserveMinimal {
                    kept.push(*e);
                } else {
                    report.removed(Kind::Exif, "TIFF tag: orientation", 0);
                }
            }
            // ICC follows the colour-profile policy, like WebP and the others.
            T_ICC_PROFILE => {
                if policy.keep_icc() {
                    kept.push(*e);
                } else {
                    report.removed(Kind::ColorProfile, "TIFF tag: ICC profile", 0);
                }
            }
            tag if KEEP.contains(&tag) => kept.push(*e),
            tag => {
                report_dropped(tag, report, dropped_unknown);
            }
        }
    }

    let (Some(off_e), Some(cnt_e)) = (offsets_entry, counts_entry) else {
        return Err(Error::malformed(FORMAT, "directory has no strip or tile data"));
    };
    let orig_offsets = read_uint_array(input, &off_e, rd).ok_or_else(|| Error::malformed(FORMAT, "unreadable pixel offsets"))?;
    let orig_counts = read_uint_array(input, &cnt_e, rd).ok_or_else(|| Error::malformed(FORMAT, "unreadable pixel byte counts"))?;
    if orig_offsets.len() != orig_counts.len() || orig_offsets.is_empty() {
        return Err(Error::malformed(FORMAT, "pixel offset/count mismatch"));
    }

    // Gather the pixel blocks now so we know their sizes; place them later.
    let mut blocks: Vec<&[u8]> = Vec::with_capacity(orig_offsets.len());
    for (o, c) in orig_offsets.iter().zip(&orig_counts) {
        let o = *o as usize;
        let c = *c as usize;
        let block = input.get(o..o.checked_add(c).ok_or_else(|| Error::malformed(FORMAT, "pixel block overflows"))?)
            .ok_or_else(|| Error::malformed(FORMAT, "pixel block out of bounds"))?;
        blocks.push(block);
    }

    // The rebuilt directory holds: every kept entry, plus a regenerated offsets
    // entry and the original counts entry. Regenerate offsets as LONG so a
    // relocated block that no longer fits a SHORT is still representable.
    let n_entries = kept.len() + 2;
    let ifd_table_len = 2 + 12 * n_entries + 4;

    // Lay out out-of-line value regions after the table. An entry's value is
    // inline when it fits in 4 bytes; otherwise it is appended here.
    let data_start = ifd_start + ifd_table_len;
    let mut cursor = data_start;

    // Prepare kept entries' out-of-line blobs.
    struct Emit {
        tag: u16,
        ty: u16,
        count: u32,
        value: [u8; 4],
    }
    let mut emit: Vec<Emit> = Vec::with_capacity(n_entries);
    let mut ool: Vec<(usize, Vec<u8>)> = Vec::new(); // (absolute offset, bytes)

    for e in &kept {
        let bytes = value_bytes(input, e, rd).ok_or_else(|| Error::malformed(FORMAT, "unreadable tag value"))?;
        if bytes.len() <= 4 {
            let mut v = [0u8; 4];
            v[..bytes.len()].copy_from_slice(&bytes);
            emit.push(Emit { tag: e.tag, ty: e.ty, count: e.count, value: v });
        } else {
            if cursor % 2 == 1 {
                cursor += 1;
            }
            let at = cursor;
            cursor += bytes.len();
            let mut vf = [0u8; 4];
            vf.copy_from_slice(&if rd.big { (at as u32).to_be_bytes() } else { (at as u32).to_le_bytes() });
            emit.push(Emit { tag: e.tag, ty: e.ty, count: e.count, value: vf });
            ool.push((at, bytes));
        }
    }

    // The byte-counts entry: reuse original values, re-emitted as LONG.
    let counts_bytes = uint_array_to_bytes(&orig_counts, rd);
    let counts_tag = if is_tiled { T_TILE_BYTE_COUNTS } else { T_STRIP_BYTE_COUNTS };
    let counts_count = orig_counts.len() as u32;
    let counts_value = place_array(&counts_bytes, &mut cursor, &mut ool, rd);

    // The offsets entry: values are the *new* block positions, computed after we
    // know where the out-of-line region ends and pixel data begins.
    let offsets_tag = if is_tiled { T_TILE_OFFSETS } else { T_STRIP_OFFSETS };
    let offsets_count = blocks.len() as u32;
    // Reserve space for the offsets array in the ool region (values filled below).
    let offsets_byte_len = blocks.len() * 4;
    let offsets_at;
    let offsets_value: [u8; 4];
    if offsets_byte_len <= 4 {
        offsets_at = None;
        // single block, inline — computed after pixel placement
        offsets_value = [0; 4];
    } else {
        if cursor % 2 == 1 {
            cursor += 1;
        }
        offsets_at = Some(cursor);
        cursor += offsets_byte_len;
        offsets_value = if rd.big { (offsets_at.unwrap() as u32).to_be_bytes() } else { (offsets_at.unwrap() as u32).to_le_bytes() };
    }

    // Pixel data begins after all out-of-line values, word-aligned.
    if cursor % 2 == 1 {
        cursor += 1;
    }
    let pixel_start = cursor;
    let mut new_offsets = Vec::with_capacity(blocks.len());
    let mut p = pixel_start;
    for b in &blocks {
        if p % 2 == 1 {
            p += 1;
        }
        if p as u64 > u32::MAX as u64 {
            return Err(Error::malformed(FORMAT, "rebuilt file exceeds TIFF's 4 GB limit"));
        }
        new_offsets.push(p as u32);
        p += b.len();
    }

    // Now materialise everything in order: table, ool region, pixel data.
    // 1. entry table.
    let mut all: Vec<Emit> = emit;
    // offsets entry value:
    let offsets_final_value = if offsets_byte_len <= 4 {
        // inline single offset
        let v = new_offsets.first().copied().unwrap_or(0);
        if rd.big { v.to_be_bytes() } else { v.to_le_bytes() }
    } else {
        offsets_value
    };
    all.push(Emit { tag: offsets_tag, ty: 4, count: offsets_count, value: offsets_final_value });
    all.push(Emit { tag: counts_tag, ty: 4, count: counts_count, value: counts_value });
    all.sort_by_key(|e| e.tag); // TIFF requires entries in ascending tag order

    rd.put16(out, all.len() as u16);
    for e in &all {
        rd.put16(out, e.tag);
        rd.put16(out, e.ty);
        rd.put32(out, e.count);
        out.extend_from_slice(&e.value);
    }
    let next_ptr_pos = out.len();
    rd.put32(out, 0); // next-IFD pointer, patched by the caller

    // 2. out-of-line values (including the offsets array, whose bytes we build
    //    now that new_offsets is known).
    // Fill the reserved offsets array into `ool` if it was out-of-line.
    if let Some(at) = offsets_at {
        let mut bytes = Vec::with_capacity(offsets_byte_len);
        for o in &new_offsets {
            rd.put32(&mut bytes, *o);
        }
        ool.push((at, bytes));
    }
    ool.sort_by_key(|(at, _)| *at);
    for (at, bytes) in &ool {
        pad_to(out, *at, ifd_start);
        debug_assert_eq!(out.len(), *at);
        out.extend_from_slice(bytes);
    }

    // 3. pixel data at the offsets we assigned.
    for (b, off) in blocks.iter().zip(&new_offsets) {
        pad_to(out, *off as usize, ifd_start);
        out.extend_from_slice(b);
    }

    Ok(next_ptr_pos)
}

/// Append an out-of-line array, or return its inline bytes if it fits in 4.
fn place_array(bytes: &[u8], cursor: &mut usize, ool: &mut Vec<(usize, Vec<u8>)>, rd: &Rd) -> [u8; 4] {
    if bytes.len() <= 4 {
        let mut v = [0u8; 4];
        v[..bytes.len()].copy_from_slice(bytes);
        v
    } else {
        if *cursor % 2 == 1 {
            *cursor += 1;
        }
        let at = *cursor;
        *cursor += bytes.len();
        ool.push((at, bytes.to_vec()));
        if rd.big { (at as u32).to_be_bytes() } else { (at as u32).to_le_bytes() }
    }
}

fn uint_array_to_bytes(vals: &[u64], rd: &Rd) -> Vec<u8> {
    let mut out = Vec::with_capacity(vals.len() * 4);
    for v in vals {
        rd.put32(&mut out, *v as u32);
    }
    out
}

/// Pad `out` with zero bytes until its length reaches `target`.
fn pad_to(out: &mut Vec<u8>, target: usize, _base: usize) {
    while out.len() < target {
        out.push(0);
    }
}

fn report_dropped(tag: u16, report: &mut Report, dropped_unknown: &mut usize) {
    let (kind, name): (Kind, &str) = match tag {
        270 => (Kind::DocumentInfo, "image description"),
        271 => (Kind::Exif, "camera make"),
        272 => (Kind::Exif, "camera model"),
        305 => (Kind::Exif, "software"),
        306 => (Kind::Timestamp, "date/time"),
        315 => (Kind::Author, "artist"),
        316 => (Kind::Exif, "host computer"),
        33432 => (Kind::Exif, "copyright"),
        700 => (Kind::Xmp, "XMP"),
        33723 => (Kind::Iptc, "IPTC"),
        34675 => (Kind::ColorProfile, "ICC profile"),
        34377 => (Kind::Exif, "Photoshop resources"),
        34665 => (Kind::Exif, "Exif sub-directory"),
        34853 => {
            // A GPS directory is the finding most likely to change a decision,
            // so flag it on its own even though we do not open it to confirm
            // coordinates are inside.
            report.found_location = true;
            report.removed(Kind::Exif, "GPS sub-directory", 0);
            return;
        }
        37724 => (Kind::Exif, "layered image data"),
        _ => {
            *dropped_unknown += 1;
            return;
        }
    };
    report.removed(kind, format!("TIFF tag: {name}"), 0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;
    use crate::Format;

    struct W {
        big: bool,
        buf: Vec<u8>,
    }
    impl W {
        fn new(big: bool) -> Self {
            W { big, buf: Vec::new() }
        }
        fn u16(&mut self, v: u16) {
            if self.big { self.buf.extend_from_slice(&v.to_be_bytes()) } else { self.buf.extend_from_slice(&v.to_le_bytes()) }
        }
        fn u32(&mut self, v: u32) {
            if self.big { self.buf.extend_from_slice(&v.to_be_bytes()) } else { self.buf.extend_from_slice(&v.to_le_bytes()) }
        }
    }

    /// Build a single-strip TIFF: header, IFD0 with the given extra entries plus
    /// the mandatory structural tags, then the strip of pixel bytes at the end.
    /// Returns the file. `extra` entries are (tag, type, count, value) inline.
    fn tiff(big: bool, extra: &[(u16, u16, u32, u32)], pixels: &[u8]) -> Vec<u8> {
        let structural: [(u16, u16, u32, u32); 6] = [
            (T_IMAGE_WIDTH, 4, 1, 2),
            (T_IMAGE_LENGTH, 4, 1, 2),
            (T_BITS_PER_SAMPLE, 3, 1, 8),
            (T_COMPRESSION, 3, 1, 1),
            (T_PHOTOMETRIC, 3, 1, 1),
            (T_ROWS_PER_STRIP, 4, 1, 2),
        ];
        // Entries: structural + strip offset/count + extra. Strip goes right
        // after the IFD; compute its offset.
        let mut entries: Vec<(u16, u16, u32, u32)> = structural.to_vec();
        entries.push((T_STRIP_BYTE_COUNTS, 4, 1, pixels.len() as u32));
        entries.extend_from_slice(extra);
        // placeholder for strip offset; fill after we know layout
        let n = entries.len() + 1; // +1 for strip offsets entry
        let ifd_len = 2 + 12 * n + 4;
        let strip_off = 8 + ifd_len;
        entries.push((T_STRIP_OFFSETS, 4, 1, strip_off as u32));
        entries.sort_by_key(|e| e.0);

        let mut w = W::new(big);
        w.buf.extend_from_slice(if big { b"MM" } else { b"II" });
        w.u16(42);
        w.u32(8);
        w.u16(entries.len() as u16);
        for (tag, ty, count, value) in &entries {
            w.u16(*tag);
            w.u16(*ty);
            w.u32(*count);
            // inline value, left-justified for SHORT
            if *ty == 3 {
                // SHORT: value in the high or low half
                let v = *value as u16;
                if big {
                    w.buf.extend_from_slice(&v.to_be_bytes());
                    w.buf.extend_from_slice(&[0, 0]);
                } else {
                    w.buf.extend_from_slice(&v.to_le_bytes());
                    w.buf.extend_from_slice(&[0, 0]);
                }
            } else {
                w.u32(*value);
            }
        }
        w.u32(0); // next IFD
        assert_eq!(w.buf.len(), strip_off, "layout math");
        w.buf.extend_from_slice(pixels);
        w.buf
    }

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Tiff, input.len());
        let out = sanitize(input, &crate::Policy::default(), &mut report).expect("valid tiff");
        (out, report)
    }

    /// Parse a TIFF far enough to return IFD0's tag set and the concatenated
    /// pixel bytes its strips point at. Used to prove the rebuild preserved them.
    fn read_back(input: &[u8]) -> (Vec<u16>, Vec<u8>) {
        let rd = Rd { big: &input[0..2] == b"MM" };
        let ifd0 = rd.u32(&input[4..8]) as usize;
        let entries = read_entries(input, ifd0, &rd).unwrap();
        let tags: Vec<u16> = entries.iter().map(|e| e.tag).collect();
        let off_e = entries.iter().find(|e| e.tag == T_STRIP_OFFSETS).unwrap();
        let cnt_e = entries.iter().find(|e| e.tag == T_STRIP_BYTE_COUNTS).unwrap();
        let offs = read_uint_array(input, off_e, &rd).unwrap();
        let cnts = read_uint_array(input, cnt_e, &rd).unwrap();
        let mut pixels = Vec::new();
        for (o, c) in offs.iter().zip(&cnts) {
            pixels.extend_from_slice(&input[*o as usize..(*o + *c) as usize]);
        }
        (tags, pixels)
    }

    #[test]
    fn metadata_tags_are_dropped_and_pixels_are_byte_identical() {
        for big in [true, false] {
            let pixels = [0xDE, 0xAD, 0xBE, 0xEF];
            let input = tiff(
                big,
                &[
                    (271, 2, 5, 0), // Make (points somewhere; count small enough to be inline-ish)
                    (306, 2, 4, 0), // DateTime
                    (34853, 4, 1, 8), // GPS IFD pointer
                ],
                &pixels,
            );
            let (out, report) = run(&input);
            let (tags, out_pixels) = read_back(&out);
            assert!(!tags.contains(&271), "Make survived (big={big})");
            assert!(!tags.contains(&306), "DateTime survived");
            assert!(!tags.contains(&34853), "GPS survived");
            assert!(tags.contains(&T_IMAGE_WIDTH), "structural tag was lost");
            assert_eq!(out_pixels, pixels, "pixel data changed");
            assert!(report.removed.iter().any(|r| r.kind == Kind::Exif));
        }
    }

    #[test]
    fn a_clean_tiff_round_trips_with_the_same_pixels() {
        let pixels = [1, 2, 3, 4];
        let input = tiff(false, &[], &pixels);
        let (out, _r) = run(&input);
        let (_tags, out_pixels) = read_back(&out);
        assert_eq!(out_pixels, pixels);
    }

    #[test]
    fn the_output_is_a_valid_parseable_tiff() {
        let input = tiff(true, &[(271, 2, 4, 0)], &[9, 9, 9, 9]);
        let (out, _r) = run(&input);
        assert!(out.len() >= 8);
        assert_eq!(&out[0..2], b"MM");
        assert_eq!(super::Rd { big: true }.u16(&out[2..4]), 42);
        // entries must be in ascending tag order
        let rd = Rd { big: true };
        let ifd0 = rd.u32(&out[4..8]) as usize;
        let entries = read_entries(&out, ifd0, &rd).unwrap();
        let mut prev = 0;
        for e in &entries {
            assert!(e.tag >= prev, "entries out of order");
            prev = e.tag;
        }
    }

    #[test]
    fn a_thumbnail_ifd_is_dropped_but_the_main_image_survives() {
        // Two IFDs: a main image and a reduced-resolution thumbnail chained after.
        let big = false;
        let main = tiff(big, &[], &[10, 20, 30, 40]);
        // Append a thumbnail IFD with NewSubfileType bit 0 set, chained from IFD0.
        // Simplest: build a second single-strip IFD and patch the chain.
        let mut file = main.clone();
        // find IFD0 next-pointer (last 4 bytes of the entry table)
        let rd = Rd { big };
        let ifd0 = rd.u32(&file[4..8]) as usize;
        let end = ifd_entry_table_end(&file, ifd0, &rd).unwrap();
        let thumb_pixels = [0xAA];
        // lay the thumbnail IFD at current end
        if file.len() % 2 == 1 {
            file.push(0);
        }
        let thumb_off = file.len();
        let thumb_entries: Vec<(u16, u16, u32, u32)> = {
            let mut v = vec![
                (T_NEW_SUBFILE_TYPE, 4, 1, 1), // reduced-resolution flag
                (T_IMAGE_WIDTH, 4, 1, 1),
                (T_IMAGE_LENGTH, 4, 1, 1),
                (T_COMPRESSION, 3, 1, 1),
                (T_STRIP_BYTE_COUNTS, 4, 1, thumb_pixels.len() as u32),
            ];
            let n = v.len() + 1;
            let ifd_len = 2 + 12 * n + 4;
            v.push((T_STRIP_OFFSETS, 4, 1, (thumb_off + ifd_len) as u32));
            v.sort_by_key(|e| e.0);
            v
        };
        let mut w = W::new(big);
        w.u16(thumb_entries.len() as u16);
        for (tag, ty, count, value) in &thumb_entries {
            w.u16(*tag);
            w.u16(*ty);
            w.u32(*count);
            if *ty == 3 {
                w.buf.extend_from_slice(&(*value as u16).to_le_bytes());
                w.buf.extend_from_slice(&[0, 0]);
            } else {
                w.u32(*value);
            }
        }
        w.u32(0);
        w.buf.extend_from_slice(&thumb_pixels);
        file.extend_from_slice(&w.buf);
        // patch IFD0's next-pointer to the thumbnail
        file[end..end + 4].copy_from_slice(&(thumb_off as u32).to_le_bytes());

        let (out, report) = run(&file);
        // The rebuilt file must have exactly one IFD (thumbnail dropped) and the
        // main pixels intact.
        let (_tags, out_pixels) = read_back(&out);
        assert_eq!(out_pixels, [10, 20, 30, 40]);
        let rd2 = Rd { big };
        let ifd0b = rd2.u32(&out[4..8]) as usize;
        let end2 = ifd_entry_table_end(&out, ifd0b, &rd2).unwrap();
        assert_eq!(rd2.u32(&out[end2..end2 + 4]), 0, "a second IFD survived");
        assert!(report.removed.iter().any(|r| r.kind == Kind::Thumbnail));
    }

    #[test]
    fn multi_strip_pixel_data_is_reassembled_in_order() {
        // Two strips at non-contiguous offsets; the rebuild must concatenate
        // them correctly and rewrite both offsets.
        let big = true;
        let rd = Rd { big };
        // Build by hand: header, IFD with 2-strip offsets/counts arrays out of line.
        let mut w = W::new(big);
        w.buf.extend_from_slice(b"MM");
        w.u16(42);
        w.u32(8);
        // entries (ascending tag): 256,257,258,259,262,273(offsets),278,279(counts)
        let entries: [(u16, u16, u32); 8] = [
            (T_IMAGE_WIDTH, 4, 1),
            (T_IMAGE_LENGTH, 4, 1),
            (T_BITS_PER_SAMPLE, 3, 1),
            (T_COMPRESSION, 3, 1),
            (T_PHOTOMETRIC, 3, 1),
            (T_STRIP_OFFSETS, 4, 2),
            (T_ROWS_PER_STRIP, 4, 1),
            (T_STRIP_BYTE_COUNTS, 4, 2),
        ];
        let ifd_len = 2 + 12 * entries.len() + 4;
        let arrays_at = 8 + ifd_len;
        let offsets_arr_at = arrays_at;
        let counts_arr_at = arrays_at + 8;
        let strip0_at = arrays_at + 16;
        let strip1_at = strip0_at + 3;
        w.u16(entries.len() as u16);
        for (tag, ty, count) in &entries {
            w.u16(*tag);
            w.u16(*ty);
            w.u32(*count);
            let value: u32 = match *tag {
                T_IMAGE_WIDTH => 4,
                T_IMAGE_LENGTH => 2,
                T_BITS_PER_SAMPLE => 8,
                T_COMPRESSION => 1,
                T_PHOTOMETRIC => 1,
                T_ROWS_PER_STRIP => 1,
                T_STRIP_OFFSETS => offsets_arr_at as u32,
                T_STRIP_BYTE_COUNTS => counts_arr_at as u32,
                _ => 0,
            };
            if *ty == 3 {
                w.buf.extend_from_slice(&(value as u16).to_be_bytes());
                w.buf.extend_from_slice(&[0, 0]);
            } else {
                w.u32(value);
            }
        }
        w.u32(0);
        assert_eq!(w.buf.len(), offsets_arr_at);
        w.u32(strip0_at as u32);
        w.u32(strip1_at as u32);
        w.u32(3); // strip0 len
        w.u32(2); // strip1 len
        assert_eq!(w.buf.len(), strip0_at);
        w.buf.extend_from_slice(&[1, 2, 3]); // strip0
        w.buf.extend_from_slice(&[4, 5]); // strip1
        let input = w.buf;

        let mut report = Report::new(Format::Tiff, input.len());
        let out = sanitize(&input, &crate::Policy::default(), &mut report).unwrap();
        let (_tags, pixels) = read_back(&out);
        assert_eq!(pixels, [1, 2, 3, 4, 5], "strips reassembled wrong");
        let _ = rd;
    }

    #[test]
    fn truncation_and_garbage_never_panic() {
        let input = tiff(true, &[(271, 2, 40, 8)], &[1, 2, 3, 4]);
        for n in 0..input.len() {
            let mut report = Report::new(Format::Tiff, n);
            let _ = sanitize(&input[..n], &crate::Policy::default(), &mut report);
        }
        for junk in [b"MM\x00\x2a".as_slice(), b"II\x2a\x00\x08\x00\x00\x00", b"garbage!"] {
            let mut report = Report::new(Format::Tiff, junk.len());
            let _ = sanitize(junk, &crate::Policy::default(), &mut report);
        }
    }

    #[test]
    fn a_camera_raw_is_left_untouched_rather_than_corrupted() {
        // A TIFF whose IFD0 declares itself a colour-filter-array sensor image
        // must be returned byte-for-byte, with an honest "not scrubbed" result.
        let input = tiff(false, &[(super::T_PHOTOMETRIC, 3, 1, 32803)], &[1, 2, 3, 4]);
        let mut report = Report::new(Format::Tiff, input.len());
        let out = sanitize(&input, &crate::Policy::default(), &mut report).unwrap();
        assert_eq!(out, input, "a raw was modified");
        assert_eq!(report.assurance, crate::Assurance::None);
        assert!(report.warnings.iter().any(|w| w.contains("camera raw")));
    }

    #[test]
    fn a_dng_version_tag_marks_a_raw() {
        let input = tiff(true, &[(50706, 1, 4, 0)], &[9, 9, 9, 9]);
        let mut report = Report::new(Format::Tiff, input.len());
        let out = sanitize(&input, &crate::Policy::default(), &mut report).unwrap();
        assert_eq!(out, input);
        assert_eq!(report.assurance, crate::Assurance::None);
    }

    #[test]
    fn orientation_is_dropped_by_default_and_kept_when_asked() {
        let input = tiff(false, &[(T_ORIENTATION, 3, 1, 6)], &[1, 2, 3, 4]);

        let (out_default, _r) = run(&input);
        let (tags, _p) = read_back(&out_default);
        assert!(!tags.contains(&T_ORIENTATION), "orientation kept under the strict default");

        let mut report = Report::new(Format::Tiff, input.len());
        let out_keep = sanitize(&input, &crate::Policy::preserve_appearance(), &mut report).unwrap();
        let (tags2, _p2) = read_back(&out_keep);
        assert!(tags2.contains(&T_ORIENTATION), "orientation dropped even when asked to keep it");
    }

    #[test]
    fn icc_profile_follows_the_colour_policy() {
        // An out-of-line ICC profile (tag 34675) with a recognisable body.
        let big = false;
        let icc = b"ICCPROFILEDATA_1234567890";
        // Build a TIFF by hand with an ICC tag pointing past the strip.
        let pixels = [7u8, 7, 7, 7];
        let mut input = tiff(big, &[(34675, 7, icc.len() as u32, 0)], &pixels);
        // append the ICC bytes and patch the tag's value offset to point at them
        let icc_at = input.len() as u32;
        input.extend_from_slice(icc);
        // find the 34675 entry and rewrite its value field
        let rd = Rd { big };
        let ifd0 = rd.u32(&input[4..8]) as usize;
        let count = rd.u16(&input[ifd0..ifd0 + 2]) as usize;
        for k in 0..count {
            let p = ifd0 + 2 + k * 12;
            if rd.u16(&input[p..p + 2]) == 34675 {
                input[p + 8..p + 12].copy_from_slice(&icc_at.to_le_bytes());
            }
        }

        // default: dropped
        let (out_drop, _r) = run(&input);
        assert!(!out_drop.windows(icc.len()).any(|w| w == icc), "ICC survived the strict default");

        // keep: preserved
        let mut report = Report::new(Format::Tiff, input.len());
        let out_keep = sanitize(&input, &crate::Policy::preserve_appearance(), &mut report).unwrap();
        assert!(out_keep.windows(icc.len()).any(|w| w == icc), "ICC dropped even when asked to keep it");
    }

    #[test]
    fn a_self_referential_ifd_chain_terminates() {
        // IFD0's next-pointer points back at IFD0.
        let mut input = tiff(false, &[], &[1, 2, 3, 4]);
        let rd = Rd { big: false };
        let ifd0 = rd.u32(&input[4..8]) as usize;
        let end = ifd_entry_table_end(&input, ifd0, &rd).unwrap();
        input[end..end + 4].copy_from_slice(&(ifd0 as u32).to_le_bytes());
        let mut report = Report::new(Format::Tiff, input.len());
        let _ = sanitize(&input, &crate::Policy::default(), &mut report);
    }
}
