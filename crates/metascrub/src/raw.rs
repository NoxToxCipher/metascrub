//! Camera raw files: remove identifying metadata *in place*, without moving a
//! single byte.
//!
//! A raw is not a picture the way a JPEG is. It is the near-unprocessed readout
//! of the camera's sensor, wrapped in a container (almost always a TIFF variant)
//! alongside the maker's private development data and usually a full-size JPEG
//! preview. The real image lives in vendor-specific sub-directories whose layout
//! is undocumented and different for every manufacturer. That is why raws cannot
//! be *rebuilt* from an allowlist the way the other formats here are: a generic
//! rebuild would not understand where the sensor data is and would hand back a
//! corrupt file the owner can never reshoot.
//!
//! So this module does the opposite of a rebuild. It locates the specific
//! structures known to identify a person, a place, or one particular camera —
//! GPS, serial numbers, the maker note, timestamps, the owner's name, XMP, and
//! the EXIF buried inside the embedded preview — and overwrites exactly those
//! bytes with zeros. Nothing is relocated, no offset changes, the file length is
//! identical, and the sensor data is never touched. The result is always
//! [`Assurance::BestEffort`](crate::Assurance::BestEffort): this removes what it
//! recognises, and an unrecognised private field could remain.
//!
//! What is deliberately kept: the camera **make and model**. They describe a
//! model owned by millions rather than a person, and raw converters use them to
//! choose how to decode the file — blanking them can stop the raw opening at
//! all. What is deliberately removed even at a cost: the **maker note**, which is
//! the main hiding place for serial numbers and shutter counts. Some makers also
//! keep development parameters there, so colour or rendering can shift; the
//! report says so.

use crate::report::{Assurance, Kind, Report};

/// A raw container family. The TIFF-based ones all share one engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Tiff,
    /// Canon CR3 and relatives: an ISO base media (MP4-like) box tree.
    Bmff,
    /// Fujifilm RAF: a bespoke header pointing at an embedded JPEG.
    Raf,
    /// Sigma/Foveon X3F: a bespoke format we can recognise but not yet clean.
    X3f,
}

/// How deep to follow sub-directory and box pointers. Real raws use three or
/// four levels; more is a malformed or hostile file.
const MAX_DEPTH: u8 = 6;

/// Public flavour name for a raw, for the report and UI. Returns `None` when the
/// input is not a raw this module claims.
pub(crate) fn flavour(input: &[u8]) -> Option<&'static str> {
    identify(input).map(|f| f.0)
}

/// Returns the raw's human name and container family, or `None` if not a raw.
fn identify(input: &[u8]) -> Option<(&'static str, Container)> {
    // Bespoke magics first — these are unambiguous.
    if input.starts_with(b"FUJIFILMCCD-RAW") {
        return Some(("Fujifilm RAF", Container::Raf));
    }
    if input.starts_with(b"FOVb") {
        return Some(("Sigma X3F", Container::X3f));
    }
    if input.len() >= 16 && &input[4..8] == b"ftyp" {
        // ISO base media: Canon CR3 declares the brand "crx ".
        let end = u32::from_be_bytes([input[0], input[1], input[2], input[3]]).clamp(16, input.len() as u32) as usize;
        if input[8..end].chunks_exact(4).any(|b| b == b"crx ") {
            return Some(("Canon CR3", Container::Bmff));
        }
    }

    // TIFF-based raws. Byte order first.
    let big = match input.get(0..2) {
        Some(b"II") => false,
        Some(b"MM") => true,
        _ => return None,
    };
    let magic = read_u16(input, 2, big)?;

    // Panasonic RW2/RWL use magic 0x55 where TIFF uses 42.
    if !big && magic == 0x55 {
        return Some(("Panasonic RW2", Container::Tiff));
    }
    // Olympus ORF uses "RO"/"RS"/"OR" as its magic.
    match &input.get(2..4) {
        Some(b"RO") | Some(b"RS") | Some(b"OR") => return Some(("Olympus ORF", Container::Tiff)),
        _ => {}
    }
    if magic != 42 {
        return None;
    }
    // Canon CR2 stamps "CR" at offset 8.
    if input.get(8..10) == Some(b"CR") {
        return Some(("Canon CR2", Container::Tiff));
    }
    // Phase One IIQ stamps "IIII" at offset 8. Its top directory is only a small
    // preview, so the structural checks below would miss it; the marker is the
    // reliable tell.
    if input.get(8..12) == Some(b"IIII") {
        return Some(("Phase One IIQ", Container::Tiff));
    }

    // Otherwise it is a standard-magic TIFF. Decide raw-vs-plain from IFD0.
    let ifd0 = read_u32(input, 4, big)? as usize;
    let (make, indicators) = probe_ifd0(input, ifd0, big);
    if indicators.dng {
        return Some(("Adobe DNG", Container::Tiff));
    }
    // A camera-vendor Make on a TIFF is treated as a raw and cleaned IN PLACE,
    // even without a structural raw marker. This is deliberate and safety-first:
    // some raws (Leaf MOS, medium-format backs) present their top directory as an
    // ordinary image, and rebuilding one as a plain TIFF destroys the sensor data
    // in the vendor sub-sections. Cleaning in place instead can never corrupt it.
    // The cost is that a genuine camera TIFF export gets best-effort rather than
    // a complete rebuild, which is the safe direction to err.
    if let Some(name) = make.as_deref().and_then(vendor_name) {
        return Some((name, Container::Tiff));
    }
    // No usable Make, but the structure alone marks it as a raw.
    if indicators.sub_images || indicators.cfa {
        return Some(("camera raw (TIFF-based)", Container::Tiff));
    }
    // Last resort: some backs (Leaf MOS, some Hasselblad) carry the make only in
    // an XMP packet, not an IFD tag, so the checks above miss them. Scan a
    // bounded prefix for a distinctive vendor marker. A false positive here is
    // harmless (a plain TIFF would just be cleaned in place instead of rebuilt);
    // a false negative would send a real raw to the rebuild path and destroy it.
    if scan_for_raw_marker(input) {
        return Some(("camera raw (TIFF-based)", Container::Tiff));
    }
    None
}

/// Distinctive vendor tokens that mark a TIFF as a raw when no IFD Make says so.
const RAW_MARKERS: &[&[u8]] = &[
    b"Hasselblad", b"Phase One", b"PhaseOne", b"Mamiya", b"Leaf", b"Imacon", b"Sinar",
];

/// True if a bounded prefix of the file contains a raw-vendor marker.
fn scan_for_raw_marker(input: &[u8]) -> bool {
    let head = &input[..input.len().min(256 * 1024)];
    RAW_MARKERS.iter().any(|m| head.windows(m.len()).any(|w| w == *m))
}

struct Ifd0Indicators {
    dng: bool,
    sub_images: bool,
    cfa: bool,
}

/// Read IFD0 shallowly: its Make string and a few raw-tell tags. Bounded and
/// non-recursive, because this runs during format detection.
fn probe_ifd0(input: &[u8], off: usize, big: bool) -> (Option<String>, Ifd0Indicators) {
    let mut ind = Ifd0Indicators { dng: false, sub_images: false, cfa: false };
    let mut make = None;
    let Some(count) = read_u16(input, off, big) else { return (make, ind) };
    for i in 0..count as usize {
        let p = off + 2 + i * 12;
        let (Some(tag), Some(ty), Some(cnt)) =
            (read_u16(input, p, big), read_u16(input, p + 2, big), read_u32(input, p + 4, big))
        else {
            break;
        };
        match tag {
            0x010F => make = read_ascii(input, p, ty, cnt, big),
            0x014A => ind.sub_images = true,      // SubIFDs
            0xC612 => ind.dng = true,             // DNGVersion
            0x00FE => {}                          // NewSubfileType (structural)
            0x0106 => {
                // PhotometricInterpretation == CFA (32803) means sensor mosaic.
                if let Some(v) = inline_short(input, p, ty, big) {
                    if v == 32803 || v == 34892 {
                        ind.cfa = true;
                    }
                }
            }
            0x828E => ind.cfa = true,             // CFAPattern
            _ => {}
        }
    }
    (make, ind)
}

fn vendor_name(make: &str) -> Option<&'static str> {
    let m = make.to_ascii_uppercase();
    // Order matters only for readability; each is a distinct prefix.
    const TABLE: &[(&str, &str)] = &[
        ("NIKON", "Nikon NEF/NRW"),
        ("CANON", "Canon raw"),
        ("SONY", "Sony ARW"),
        ("PENTAX", "Pentax PEF"),
        ("RICOH", "Pentax/Ricoh PEF"),
        ("OLYMPUS", "Olympus ORF"),
        ("OM DIGITAL", "OM System ORF"),
        ("PANASONIC", "Panasonic RW2"),
        ("LEICA", "Leica raw"),
        ("SAMSUNG", "Samsung SRW"),
        ("FUJIFILM", "Fujifilm raw"),
        ("HASSELBLAD", "Hasselblad 3FR/FFF"),
        ("PHASE ONE", "Phase One IIQ"),
        ("MAMIYA", "Mamiya MEF"),
        ("LEAF", "Leaf MOS"),
        ("KODAK", "Kodak DCR/KDC"),
        ("EASTMAN KODAK", "Kodak DCR/KDC"),
        ("EPSON", "Epson ERF"),
        ("SEIKO EPSON", "Epson ERF"),
        ("GOPRO", "GoPro GPR"),
        ("SIGMA", "Sigma raw"),
        ("CASIO", "Casio raw"),
        ("MINOLTA", "Minolta MRW"),
        ("KONICA MINOLTA", "Minolta MRW"),
    ];
    TABLE.iter().find(|(k, _)| m.starts_with(k)).map(|(_, v)| *v)
}

pub(crate) fn sanitize(input: &[u8], _policy: &crate::Policy, report: &mut Report) -> crate::Result<Vec<u8>> {
    report.assurance = Assurance::BestEffort;
    let Some((name, container)) = identify(input) else {
        // Detection said raw but we cannot re-confirm it: refuse to guess.
        report.assurance = Assurance::None;
        report.warn("this did not parse as a raw after all, so nothing was changed");
        return Ok(input.to_vec());
    };

    let mut buf = input.to_vec();
    match container {
        Container::Tiff => {
            let magic = read_u16(&buf, 2, buf.starts_with(b"MM")).unwrap_or(0);
            scrub_tiff_based(&mut buf, report, magic)?;
        }
        Container::Bmff => scrub_bmff(&mut buf, report),
        Container::Raf => scrub_raf(&mut buf, report),
        Container::X3f => {
            report.assurance = Assurance::None;
            report.warn(
                "this is a Sigma X3F raw, whose layout metascrub does not yet parse; it was left \
                 exactly as it arrived and nothing was removed",
            );
            return Ok(buf);
        }
    }

    // A raw is cleaned by editing in place, never rebuilt. The `removed` list
    // says what came out, the `retained` list says what had to stay and what it
    // reveals, and this note ties them together and states the safe path.
    report.warn(format!(
        "{name}: cleaned in place (best effort), not rebuilt. The sensor image and the file's \
         ability to be developed were not touched, nothing was moved, and the length is \
         unchanged. What was removed is listed above; anything that had to stay is listed under \
         what is still in the file, with what it would reveal. For a complete clean with nothing \
         left, develop the raw into a JPEG and clean that instead.",
    ));
    Ok(buf)
}

// ------------------------------- TIFF engine -------------------------------

/// Everything gathered in one pass over the directory tree, to be applied after.
#[derive(Default)]
struct Plan {
    /// Out-of-line value byte ranges to zero, with why.
    zero: Vec<(usize, usize, Kind)>,
    /// Ranges that must never be zeroed: IFD tables, header, sensor pixel data.
    protect: Vec<(usize, usize)>,
    /// Offsets of embedded JPEGs (previews/thumbnails) to scrub in place.
    jpegs: Vec<usize>,
    /// Inline value fields (inside an IFD entry) to zero, 4 bytes each.
    zero_inline: Vec<(usize, Kind)>,
    gps: bool,
    /// A vendor maker note was present and kept; its serial therefore remains.
    maker_note_present: bool,
}

fn scrub_tiff_based(buf: &mut [u8], report: &mut Report, magic: u16) -> crate::Result<()> {
    let big = &buf[0..2] == b"MM";
    let _ = magic; // any of 42 / 0x55 / ORF already validated by identify()
    let Some(ifd0) = read_u32(buf, 4, big).map(|v| v as usize) else {
        return Ok(()); // nothing parseable; leave untouched
    };
    let mut plan = Plan::default();
    plan.protect.push((0, 8)); // the header
    let mut visited = std::collections::BTreeSet::new();
    walk_ifd(buf, ifd0, big, Role::Normal, 0, &mut plan, &mut visited, true);

    apply(buf, &plan, report);
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Normal,
    Gps,
    /// A sub-directory that may hold a full-resolution preview image.
    Sub,
}

#[allow(clippy::too_many_arguments)]
fn walk_ifd(
    buf: &[u8],
    off: usize,
    big: bool,
    role: Role,
    depth: u8,
    plan: &mut Plan,
    visited: &mut std::collections::BTreeSet<usize>,
    follow_chain: bool,
) {
    // Depth guards nesting; the count guard bounds a long chain of distinct
    // directories, which the visited-set alone would follow to the file's end.
    if depth >= MAX_DEPTH || visited.len() >= 8192 || !visited.insert(off) {
        return;
    }
    let Some(count) = read_u16(buf, off, big) else { return };
    let table_end = off + 2 + count as usize * 12;
    if table_end + 4 > buf.len() {
        return;
    }
    plan.protect.push((off, table_end + 4));

    // First pass: note strip/tile geometry so we can both protect the pixels and
    // spot a JPEG-compressed preview.
    let mut strip_offsets: Vec<u64> = Vec::new();
    let mut strip_counts: Vec<u64> = Vec::new();
    let mut compression: Option<u16> = None;
    let mut jpeg_ptr: Option<(u64, u64)> = None; // (offset, length) for tag 0x0201/0x0202

    for i in 0..count as usize {
        let p = off + 2 + i * 12;
        let (Some(tag), Some(ty), Some(cnt)) =
            (read_u16(buf, p, big), read_u16(buf, p + 2, big), read_u32(buf, p + 4, big))
        else {
            return;
        };
        match tag {
            0x0103 => compression = inline_short(buf, p, ty, big),
            0x0111 | 0x0144 => strip_offsets = read_uints(buf, p, ty, cnt, big),
            0x0117 | 0x0145 => strip_counts = read_uints(buf, p, ty, cnt, big),
            0x0201 => {
                if let Some(v) = read_uints(buf, p, ty, cnt, big).first().copied() {
                    jpeg_ptr = Some((v, jpeg_ptr.map(|x| x.1).unwrap_or(0)));
                }
            }
            0x0202 => {
                if let Some(v) = read_uints(buf, p, ty, cnt, big).first().copied() {
                    jpeg_ptr = Some((jpeg_ptr.map(|x| x.0).unwrap_or(0), v));
                }
            }
            _ => {}
        }
        // A tag whose out-of-line value is itself a JPEG (Panasonic's JpgFromRaw
        // at 0x002e, and other vendors' embedded previews stored as a blob rather
        // than an offset pair) is a preview to scrub. Its EXIF carries the same
        // timestamps as the raw. Checking the value's own bytes catches these
        // without hard-coding each vendor's tag number.
        if let Some(voff) = out_of_line_offset(buf, p, ty, cnt, big) {
            if buf.get(voff..voff + 3) == Some(&[0xFF, 0xD8, 0xFF]) {
                plan.jpegs.push(voff);
            }
        }
    }

    // Protect pixel data; if it is a JPEG preview, queue it for in-place scrub.
    for (o, c) in strip_offsets.iter().zip(&strip_counts) {
        let (o, c) = (*o as usize, *c as usize);
        if o.checked_add(c).map(|e| e <= buf.len()).unwrap_or(false) {
            plan.protect.push((o, o + c));
            if buf.get(o..o + 3) == Some(&[0xFF, 0xD8, 0xFF]) {
                plan.jpegs.push(o); // a JPEG-compressed preview/thumbnail
            }
        }
    }
    if let Some((o, len)) = jpeg_ptr {
        let (o, len) = (o as usize, len as usize);
        if o.checked_add(len).map(|e| e <= buf.len() && len > 0).unwrap_or(false)
            && buf.get(o..o + 3) == Some(&[0xFF, 0xD8, 0xFF])
        {
            plan.jpegs.push(o);
        }
    }
    let _ = compression;

    // Second pass: decide each entry's fate.
    for i in 0..count as usize {
        let p = off + 2 + i * 12;
        let (Some(tag), Some(ty), Some(cnt)) =
            (read_u16(buf, p, big), read_u16(buf, p + 2, big), read_u32(buf, p + 4, big))
        else {
            return;
        };

        // In a GPS directory every value is location data.
        if role == Role::Gps {
            if matches!(tag, 0x0002 | 0x0004 | 0x0006 | 0x0012) {
                plan.gps = true; // latitude / longitude / altitude / area name
            }
            queue_value(buf, p, ty, cnt, big, Kind::Exif, plan);
            continue;
        }

        match tag {
            // Sub-directory pointers: recurse, never zero the pointer itself.
            0x8769 => follow(buf, p, ty, cnt, big, Role::Normal, depth, plan, visited),
            0x8825 => follow(buf, p, ty, cnt, big, Role::Gps, depth, plan, visited),
            0xA005 => follow(buf, p, ty, cnt, big, Role::Normal, depth, plan, visited),
            0x014A => follow(buf, p, ty, cnt, big, Role::Sub, depth, plan, visited),
            // The maker note (0x927C) and DNG private data (0xC634) are opaque
            // vendor blobs. They do hold serial numbers, but manufacturers also
            // store the parameters a raw converter needs to decode the file in
            // here (black and white levels, linearisation curves, sensor
            // geometry, white balance). Zeroing them corrupted real files from
            // several brands, so they are KEPT and the report says so. A serial
            // in here therefore survives; the honest fix for that is to develop
            // the raw to a JPEG and clean that. Keeping it means doing nothing
            // except noting that it, and the serial inside it, remain.
            0x927C | 0xC634 => plan.maker_note_present = true,
            _ => {
                if let Some(kind) = identifying(tag) {
                    queue_value(buf, p, ty, cnt, big, kind, plan);
                }
            }
        }
    }

    // Follow the IFD chain (IFD1 is the thumbnail directory, and so on).
    if follow_chain {
        if let Some(next) = read_u32(buf, table_end, big) {
            if next != 0 {
                walk_ifd(buf, next as usize, big, Role::Normal, depth, plan, visited, true);
            }
        }
    }
}

/// Follow a sub-IFD pointer entry (its value is one or more offsets).
#[allow(clippy::too_many_arguments)]
fn follow(
    buf: &[u8],
    p: usize,
    ty: u16,
    cnt: u32,
    big: bool,
    role: Role,
    depth: u8,
    plan: &mut Plan,
    visited: &mut std::collections::BTreeSet<usize>,
) {
    for target in read_uints(buf, p, ty, cnt, big) {
        walk_ifd(buf, target as usize, big, role, depth + 1, plan, visited, false);
    }
}

/// Which tags carry data that identifies a person, place or single device.
/// Make (0x010F) and Model (0x0110) are intentionally absent: they are kept.
fn identifying(tag: u16) -> Option<Kind> {
    Some(match tag {
        0x010D | 0x010E | 0x013C | 0x0131 => Kind::Exif, // DocumentName, ImageDescription, HostComputer, Software
        0x0132 | 0x9003 | 0x9004 | 0x9290 | 0x9291 | 0x9292 => Kind::Timestamp,
        0x013B | 0xA430 => Kind::Author, // Artist, CameraOwnerName
        0x8298 => Kind::Exif,            // Copyright
        0xA431 | 0xA435 | 0xC62F => Kind::MakerNote, // Body/Lens/Camera serial numbers
        0xA420 | 0xC65D => Kind::Exif,   // ImageUniqueID, RawDataUniqueID
        0x9286 => Kind::Comment,         // UserComment
        0x02BC => Kind::Xmp,             // XMP
        0x83BB => Kind::Iptc,            // IPTC-NAA
        0x8649 => Kind::Iptc,            // Photoshop image resources
        0xC68B | 0xC68C => Kind::Exif,   // OriginalRawFileName / OriginalRawFileData
        0x4746 | 0x4749 => Kind::Exif,   // Rating, RatingPercent
        _ => return None,
    })
}

/// Queue an entry's value bytes for zeroing, inline or out-of-line.
fn queue_value(buf: &[u8], p: usize, ty: u16, cnt: u32, big: bool, kind: Kind, plan: &mut Plan) {
    let Some(size) = type_size(ty) else { return };
    let byte_len = size as u64 * cnt as u64;
    if byte_len == 0 {
        return;
    }
    if byte_len <= 4 {
        plan.zero_inline.push((p + 8, kind)); // the 4-byte value field
    } else if let Some(off) = read_u32(buf, p + 8, big) {
        let off = off as usize;
        if off.checked_add(byte_len as usize).map(|e| e <= buf.len()).unwrap_or(false) {
            plan.zero.push((off, off + byte_len as usize, kind));
        }
    }
}

/// Plain-language disclosure for a field that had to be left in place because it
/// sits inside the image data and zeroing it could corrupt the picture.
fn retained_for(kind: Kind) -> (&'static str, &'static str) {
    match kind {
        Kind::Timestamp => (
            "a date-and-time stamp stored inside the image data (removing it could corrupt the picture)",
            "when the photo was taken",
        ),
        Kind::Author => (
            "a name field stored inside the image data",
            "who took or owns the photo",
        ),
        Kind::Xmp | Kind::Iptc => (
            "an XMP or IPTC block stored inside the image data",
            "editing history, keywords, author or licence details",
        ),
        _ => (
            "an identifying field stored inside the image data (removing it could corrupt the picture)",
            "details the camera or software recorded about this file",
        ),
    }
}

/// Apply the plan: zero the metadata regions that do not collide with anything
/// structural, then scrub embedded previews.
fn apply(buf: &mut [u8], plan: &Plan, report: &mut Report) {
    if plan.gps {
        report.found_location = true;
    }
    let mut protect = plan.protect.clone();
    protect.sort_unstable();

    // A maker note was present and kept: its serial remains. Disclose it.
    if plan.maker_note_present {
        report.retain(
            "the manufacturer's maker note (kept because it holds the settings a raw converter \
             needs to develop the file)",
            "the camera's internal serial number and shutter count, which can tie this file to \
             one specific camera body",
        );
    }

    // Out-of-line values.
    let mut counts: std::collections::BTreeMap<Kind, (usize, usize)> = std::collections::BTreeMap::new();
    for &(start, end, kind) in &plan.zero {
        if overlaps(&protect, start, end) {
            // Aliased onto something structural; refuse rather than risk the
            // pixels. Disclosed specifically so the residual risk is not hidden.
            let (what, reveals) = retained_for(kind);
            report.retain(what, reveals);
            continue;
        }
        // A region that is already all-zero holds nothing to remove. Skipping it
        // keeps the clean idempotent, so re-scanning cleaned output reports no
        // further removals (which is what lets verification pass on a raw).
        if buf[start..end].iter().all(|&b| b == 0) {
            continue;
        }
        for b in &mut buf[start..end] {
            *b = 0;
        }
        let e = counts.entry(kind).or_default();
        e.0 += 1;
        e.1 += end - start;
    }
    // Inline values (inside an IFD entry, so never in the protected pixel set).
    for &(at, kind) in &plan.zero_inline {
        if at + 4 <= buf.len() && !buf[at..at + 4].iter().all(|&b| b == 0) {
            buf[at..at + 4].copy_from_slice(&[0; 4]);
            let e = counts.entry(kind).or_default();
            e.0 += 1;
            e.1 += 4;
        }
    }
    for (kind, (n, bytes)) in counts {
        report.removed(kind, format!("{n} raw tag(s)"), bytes);
    }

    // Embedded preview/thumbnail JPEGs, deduplicated.
    let mut seen = std::collections::BTreeSet::new();
    for &at in &plan.jpegs {
        if seen.insert(at) {
            scrub_jpeg_in_place(buf, at, report, "embedded preview");
        }
    }
}

/// Clean the metadata segments of a JPEG that begins at `start`, keeping every
/// marker and length so the file stays valid and the same length.
///
/// The EXIF segment is **not** blanked wholesale: in a raw's embedded JPEG the
/// EXIF's directory tables and their offsets can be load-bearing for the raw
/// decoder (they locate the sensor data), so blanking them corrupts the file.
/// Instead the EXIF is neutralised surgically, the same way as a top-level
/// directory: only identifying tag values and the maker note are zeroed, and
/// the structure is left intact. XMP, IPTC and comment segments carry no such
/// structure, so those are blanked whole.
fn scrub_jpeg_in_place(buf: &mut [u8], start: usize, report: &mut Report, ctx: &str) -> usize {
    let mut i = start + 2; // past SOI
    let mut bytes = 0;
    while i + 4 <= buf.len() {
        if buf[i] != 0xFF {
            break;
        }
        let marker = buf[i + 1];
        if marker == 0xD9 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            i += 2;
            continue;
        }
        if marker == 0xDA {
            break; // start of scan: entropy data follows, stop
        }
        let seg_len = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
        if seg_len < 2 || i + 2 + seg_len > buf.len() {
            break;
        }
        let payload = i + 4;
        let payload_end = i + 2 + seg_len;

        if marker == 0xE1 && buf.get(payload..payload + 6) == Some(b"Exif\0\0") {
            // Surgical: preserve the IFD structure the decoder may need.
            scrub_exif_block(buf, payload + 6, payload_end, report, ctx);
        } else if marker == 0xE1 && buf.get(payload..payload + 4) == Some(b"http") {
            if blank(buf, payload, payload_end) {
                report.removed(Kind::Xmp, format!("{ctx} XMP"), payload_end - payload);
                bytes += payload_end - payload;
            }
        } else if marker == 0xED {
            if blank(buf, payload, payload_end) {
                report.removed(Kind::Iptc, format!("{ctx} IPTC"), payload_end - payload);
                bytes += payload_end - payload;
            }
        } else if marker == 0xFE {
            if blank(buf, payload, payload_end) {
                report.removed(Kind::Comment, format!("{ctx} comment"), payload_end - payload);
                bytes += payload_end - payload;
            }
        } else if matches!(marker, 0xE2..=0xEF) {
            // Other application segments (not JFIF): no structure we rely on.
            if blank(buf, payload, payload_end) {
                report.removed(Kind::UnknownStructure, format!("{ctx} APP{}", marker - 0xE0), payload_end - payload);
                bytes += payload_end - payload;
            }
        }
        i = payload_end;
    }
    bytes
}

/// Zero a range, returning true if it held any non-zero byte (i.e. there was
/// something to remove). Returning false on an already-zero range keeps the
/// clean idempotent, so re-scanning cleaned output reports nothing further.
fn blank(buf: &mut [u8], start: usize, end: usize) -> bool {
    let end = end.min(buf.len());
    if start >= end || buf[start..end].iter().all(|&b| b == 0) {
        return false;
    }
    for b in &mut buf[start..end] {
        *b = 0;
    }
    true
}

/// Neutralise an embedded EXIF/TIFF block in place: walk its directory tree and
/// zero only the identifying values and the maker note, leaving every tag table
/// and offset untouched so a raw decoder that reads this block still works.
/// `start`..`end` spans the TIFF (from its `II`/`MM` byte-order mark).
fn scrub_exif_block(buf: &mut [u8], start: usize, end: usize, report: &mut Report, ctx: &str) {
    if end <= start || end > buf.len() || start + 8 > end {
        return;
    }
    let big = match &buf[start..start + 2] {
        b"MM" => true,
        b"II" => false,
        _ => return,
    };
    let Some(ifd0) = read_u32(buf, start + 4, big).map(|v| v as usize) else {
        return;
    };
    let mut plan = Plan::default();
    plan.protect.push((0, 8));
    let mut visited = std::collections::BTreeSet::new();
    walk_ifd(&buf[start..end], ifd0, big, Role::Normal, 0, &mut plan, &mut visited, true);
    let _ = ctx;
    apply(&mut buf[start..end], &plan, report);
}

// ------------------------------- CR3 (BMFF) -------------------------------

/// Walk the box tree; scrub any TIFF/EXIF block or JPEG preview found inside a
/// box payload. Canon keeps EXIF in `CMT1`..`CMT4` (TIFF blocks) and previews in
/// `PRVW`/`THMB` (JPEG); walking generically catches all of them without hard
/// coding Canon's box names.
fn scrub_bmff(buf: &mut [u8], report: &mut Report) {
    let mut jobs: Vec<Job> = Vec::new();
    let mut budget = 1u32 << 20; // total boxes to visit before giving up
    collect_bmff(buf, 0, buf.len(), 0, &mut jobs, &mut budget);
    run_jobs(buf, jobs, report);
}

enum Job {
    Tiff(usize, usize), // (start, end) of a TIFF block
    Jpeg(usize),        // start of a JPEG
}

fn collect_bmff(buf: &[u8], start: usize, end: usize, depth: u8, jobs: &mut Vec<Job>, budget: &mut u32) {
    if depth >= MAX_DEPTH {
        return;
    }
    let mut i = start;
    while i + 8 <= end {
        if *budget == 0 || jobs.len() >= 65536 {
            return; // a file crafted to be all tiny boxes; stop walking
        }
        *budget -= 1;
        let size32 = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
        let kind = &buf[i + 4..i + 8];
        let (header, box_size) = match size32 {
            0 => (8usize, end - i),           // extends to the end
            1 => {
                // 64-bit size in the next 8 bytes.
                if i + 16 > end {
                    return;
                }
                let s = u64::from_be_bytes(buf[i + 8..i + 16].try_into().unwrap()) as usize;
                (16usize, s)
            }
            s => (8usize, s),
        };
        if box_size < header || i + box_size > end {
            return;
        }
        let body = i + header;
        let body_end = i + box_size;

        // Boxes that carry payloads we care about, or that contain child boxes.
        match kind {
            b"moov" | b"trak" | b"mdia" | b"minf" | b"stbl" | b"moof" | b"traf" | b"dinf" => {
                collect_bmff(buf, body, body_end, depth + 1, jobs, budget);
            }
            b"uuid" => {
                // Canon's metadata lives under a uuid box; its first 16 bytes are
                // the UUID, then child boxes.
                if body + 16 <= body_end {
                    collect_bmff(buf, body + 16, body_end, depth + 1, jobs, budget);
                }
            }
            b"PRVW" | b"THMB" | b"PreviewImage" => {
                // A named preview box wraps its JPEG behind a short header. Find
                // the JPEG start within the first bytes; scanning is safe here
                // because the box type already tells us this is a preview, not
                // sensor data.
                if let Some(rel) = buf.get(body..body_end.min(body + 64)).and_then(find_jpeg_soi) {
                    jobs.push(Job::Jpeg(body + rel));
                }
            }
            _ => {
                // Any other leaf is sniffed only at its very start, so that
                // sensor data in an `mdat` can never be mistaken for a preview.
                if body + 4 <= body_end {
                    let head = &buf[body..body_end.min(body + 4)];
                    if head == b"II\x2a\x00" || head == b"MM\x00\x2a" {
                        jobs.push(Job::Tiff(body, body_end));
                    } else if buf.get(body..body + 3) == Some(&[0xFF, 0xD8, 0xFF]) {
                        jobs.push(Job::Jpeg(body));
                    }
                }
            }
        }
        i += box_size;
    }
}

/// Offset of the first `FF D8 FF` (JPEG start) within a short window.
fn find_jpeg_soi(window: &[u8]) -> Option<usize> {
    window.windows(3).position(|w| w == [0xFF, 0xD8, 0xFF])
}

fn run_jobs(buf: &mut [u8], jobs: Vec<Job>, report: &mut Report) {
    for job in jobs {
        match job {
            Job::Jpeg(at) => {
                scrub_jpeg_in_place(buf, at, report, "CR3 preview");
            }
            Job::Tiff(start, end) => {
                // Scrub the embedded TIFF block relative to its own start.
                let mut sub = buf[start..end].to_vec();
                let big = sub.starts_with(b"MM");
                if let Some(ifd0) = read_u32(&sub, 4, big).map(|v| v as usize) {
                    let mut plan = Plan::default();
                    plan.protect.push((0, 8));
                    let mut visited = std::collections::BTreeSet::new();
                    walk_ifd(&sub, ifd0, big, Role::Normal, 0, &mut plan, &mut visited, true);
                    apply(&mut sub, &plan, report);
                }
                buf[start..end].copy_from_slice(&sub);
            }
        }
    }
}

// ------------------------------- RAF (Fuji) -------------------------------

/// A RAF is a 16-byte magic, a format/camera header, then a table of offsets.
/// The identifying metadata sits in an embedded JPEG whose offset and length are
/// stored as big-endian u32s at fixed positions in the header.
fn scrub_raf(buf: &mut [u8], report: &mut Report) {
    // Offsets per the RAF layout: JPEG image offset at 0x54, length at 0x58.
    let (Some(jpeg_off), Some(jpeg_len)) = (read_u32(buf, 0x54, true), read_u32(buf, 0x58, true)) else {
        report.warn("this RAF's header was too short to locate its preview; nothing was changed");
        return;
    };
    let (o, len) = (jpeg_off as usize, jpeg_len as usize);
    if o.checked_add(len).map(|e| e <= buf.len() && len > 4).unwrap_or(false)
        && buf.get(o..o + 3) == Some(&[0xFF, 0xD8, 0xFF])
    {
        scrub_jpeg_in_place(buf, o, report, "RAF preview");
    } else {
        report.warn("this RAF's preview image was not where its header pointed; nothing was changed");
    }
}

// ------------------------------- byte helpers -------------------------------

fn read_u16(buf: &[u8], at: usize, big: bool) -> Option<u16> {
    let b = buf.get(at..at + 2)?;
    Some(if big { u16::from_be_bytes([b[0], b[1]]) } else { u16::from_le_bytes([b[0], b[1]]) })
}

fn read_u32(buf: &[u8], at: usize, big: bool) -> Option<u32> {
    let b = buf.get(at..at + 4)?;
    Some(if big {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    } else {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    })
}

fn type_size(ty: u16) -> Option<u32> {
    Some(match ty {
        1 | 2 | 6 | 7 => 1,
        3 | 8 => 2,
        4 | 9 | 11 | 13 => 4,
        5 | 10 | 12 => 8,
        _ => return None,
    })
}

/// If an entry's value does not fit inline (more than 4 bytes), return the file
/// offset it points to; otherwise `None`.
fn out_of_line_offset(buf: &[u8], p: usize, ty: u16, cnt: u32, big: bool) -> Option<usize> {
    let size = type_size(ty)?;
    let byte_len = size as u64 * cnt as u64;
    if byte_len <= 4 {
        return None;
    }
    let off = read_u32(buf, p + 8, big)? as usize;
    (off + 3 <= buf.len()).then_some(off)
}

/// Read an entry's value as an array of unsigned integers (BYTE/SHORT/LONG).
fn read_uints(buf: &[u8], p: usize, ty: u16, cnt: u32, big: bool) -> Vec<u64> {
    let Some(size) = type_size(ty) else { return Vec::new() };
    let byte_len = size as usize * cnt as usize;
    let base = if byte_len <= 4 {
        p + 8
    } else {
        match read_u32(buf, p + 8, big) {
            Some(o) => o as usize,
            None => return Vec::new(),
        }
    };
    // A hostile file can claim a billion-element array; no real array can hold
    // more elements than the file has room for, so cap the count there.
    let max_elems = buf.len() / size.max(1) as usize + 1;
    let cnt = (cnt as usize).min(max_elems);
    let mut out = Vec::with_capacity(cnt.min(4096));
    for k in 0..cnt {
        let at = base + k * size as usize;
        let v = match size {
            1 => buf.get(at).map(|b| *b as u64),
            2 => read_u16(buf, at, big).map(|v| v as u64),
            4 => read_u32(buf, at, big).map(|v| v as u64),
            _ => None,
        };
        match v {
            Some(v) => out.push(v),
            None => break,
        }
    }
    out
}

fn inline_short(buf: &[u8], p: usize, ty: u16, big: bool) -> Option<u16> {
    match ty {
        3 => read_u16(buf, p + 8, big),
        4 => read_u32(buf, p + 8, big).map(|v| v as u16),
        _ => None,
    }
}

fn read_ascii(buf: &[u8], p: usize, ty: u16, cnt: u32, big: bool) -> Option<String> {
    if ty != 2 {
        return None;
    }
    let n = cnt as usize;
    let base = if n <= 4 {
        p + 8
    } else {
        read_u32(buf, p + 8, big)? as usize
    };
    let raw = buf.get(base..base + n)?;
    let s: String = raw.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
    Some(s)
}

fn overlaps(sorted: &[(usize, usize)], start: usize, end: usize) -> bool {
    sorted.iter().any(|&(s, e)| start < e && s < end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;
    use crate::Format;

    // ---- a tiny TIFF-raw builder ----

    /// Build a little-endian TIFF-based raw: IFD0 with the given entries plus a
    /// SubIFDs pointer, a CFA marker, a Make, and a strip of "sensor" bytes. Any
    /// out-of-line values are appended and referenced.
    struct RawBuilder {
        entries: Vec<(u16, u16, Vec<u8>)>, // (tag, type, value-bytes)
    }
    impl RawBuilder {
        fn new() -> Self {
            RawBuilder { entries: Vec::new() }
        }
        fn ascii(mut self, tag: u16, s: &str) -> Self {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            self.entries.push((tag, 2, v));
            self
        }
        fn undefined(mut self, tag: u16, bytes: Vec<u8>) -> Self {
            self.entries.push((tag, 7, bytes));
            self
        }
        fn build(mut self) -> Vec<u8> {
            // Always include: Make (Canon), PhotometricInterpretation=CFA, a
            // sensor strip so the file reads as a raw and has protected pixels.
            self = self.ascii(0x010F, "NIKON CORPORATION");
            self.entries.push((0x0106, 3, 32803u16.to_le_bytes().to_vec())); // CFA
            let sensor = b"SENSORDATA-DO-NOT-TOUCH-0123456789".to_vec();

            let n = self.entries.len() + 2; // + StripOffsets + StripByteCounts
            let ifd_start = 8usize;
            let table_len = 2 + n * 12 + 4;
            let mut data_off = ifd_start + table_len;

            // Reserve positions for out-of-line values.
            let mut ool: Vec<(usize, Vec<u8>)> = Vec::new();
            let mut entry_meta: Vec<(u16, u16, u32, [u8; 4])> = Vec::new();
            for (tag, ty, bytes) in &self.entries {
                let cnt = match ty {
                    2 => bytes.len() as u32,
                    3 => (bytes.len() / 2) as u32,
                    _ => bytes.len() as u32,
                };
                let mut value = [0u8; 4];
                if bytes.len() <= 4 {
                    value[..bytes.len()].copy_from_slice(bytes);
                } else {
                    let at = data_off;
                    value.copy_from_slice(&(at as u32).to_le_bytes());
                    ool.push((at, bytes.clone()));
                    data_off += bytes.len();
                    if data_off % 2 == 1 {
                        data_off += 1;
                    }
                }
                entry_meta.push((*tag, *ty, cnt, value));
            }
            // Sensor strip after all ool values.
            let strip_at = data_off;
            data_off += sensor.len();

            // StripOffsets (0x0111) and StripByteCounts (0x0117).
            entry_meta.push((0x0111, 4, 1, (strip_at as u32).to_le_bytes()));
            entry_meta.push((0x0117, 4, 1, (sensor.len() as u32).to_le_bytes()));
            entry_meta.sort_by_key(|e| e.0);

            let mut out = vec![b'I', b'I', 0x2a, 0x00];
            out.extend_from_slice(&8u32.to_le_bytes());
            out.extend_from_slice(&(entry_meta.len() as u16).to_le_bytes());
            for (tag, ty, cnt, value) in &entry_meta {
                out.extend_from_slice(&tag.to_le_bytes());
                out.extend_from_slice(&ty.to_le_bytes());
                out.extend_from_slice(&cnt.to_le_bytes());
                out.extend_from_slice(value);
            }
            out.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
            // Now append ool values and the sensor strip at their reserved spots.
            out.resize(data_off, 0);
            for (at, bytes) in &ool {
                out[*at..*at + bytes.len()].copy_from_slice(bytes);
            }
            out[strip_at..strip_at + sensor.len()].copy_from_slice(&sensor);
            out
        }
    }

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Raw, input.len());
        let out = sanitize(input, &crate::Policy::default(), &mut report).unwrap();
        assert_eq!(out.len(), input.len(), "raw scrubbing must preserve file length");
        (out, report)
    }

    #[test]
    fn identifies_the_common_raw_magics() {
        assert_eq!(flavour(b"FUJIFILMCCD-RAW0201FF..........."), Some("Fujifilm RAF"));
        assert_eq!(flavour(b"FOVb\x00\x00\x00\x00"), Some("Sigma X3F"));
        // RW2: little-endian, magic 0x55.
        assert_eq!(flavour(b"II\x55\x00\x08\x00\x00\x00"), Some("Panasonic RW2"));
        // ORF.
        assert_eq!(flavour(b"IIRO\x08\x00\x00\x00"), Some("Olympus ORF"));
        // CR2.
        assert_eq!(flavour(b"II\x2a\x00\x10\x00\x00\x00CR\x02\x00"), Some("Canon CR2"));
    }

    #[test]
    fn a_plain_tiff_is_not_taken_for_a_raw() {
        // II*, magic 42, no camera Make, no sub-images: not a raw.
        assert_eq!(flavour(b"II\x2a\x00\x08\x00\x00\x00\x00\x00"), None);
    }

    #[test]
    fn standard_identifiers_are_zeroed_the_maker_note_and_sensor_are_kept() {
        // The conservative contract: standard identifying tags (timestamp,
        // standard serial, artist) are removed; the maker note is KEPT because
        // it holds decode parameters (removing it corrupted real files); the
        // sensor data and camera make are untouched.
        let raw = RawBuilder::new()
            .ascii(0x9003, "2026:01:02 03:04:05") // DateTimeOriginal
            .ascii(0xA431, "SERIAL-BODY-987654")  // BodySerialNumber (standard)
            .ascii(0x013B, "Jane Q. Photographer") // Artist
            .undefined(0x927C, b"MAKERNOTE-serial=XYZ-shuttercount=4200".to_vec())
            .build();
        let (out, report) = run(&raw);

        // Removed: the standard identifying tags.
        for needle in [&b"2026:01:02"[..], b"SERIAL-BODY-987654", b"Jane Q. Photographer"] {
            assert!(
                !out.windows(needle.len()).any(|w| w == needle),
                "{:?} survived",
                String::from_utf8_lossy(needle)
            );
        }
        // Kept on purpose: the maker note (may hold a serial, but holds decode data).
        assert!(
            out.windows(16).any(|w| w == b"MAKERNOTE-serial"),
            "the maker note was zeroed, which corrupts real raws"
        );
        // Untouched: sensor data and camera make.
        assert!(
            out.windows(34).any(|w| w == b"SENSORDATA-DO-NOT-TOUCH-0123456789"),
            "sensor pixel data was altered"
        );
        assert!(out.windows(5).any(|w| w == b"NIKON"), "camera make was wrongly removed");
        assert_eq!(report.assurance, Assurance::BestEffort);
        assert_eq!(out.len(), raw.len(), "in-place scrub changed the length");

        // The kept maker note must be DISCLOSED, with what it reveals, so the
        // partial clean is never mistaken for a full one.
        assert!(
            report.retained.iter().any(|r| r.what.contains("maker note")
                && r.reveals.to_lowercase().contains("serial")),
            "a kept maker note was not disclosed to the user"
        );
    }

    #[test]
    fn gps_in_a_sub_directory_is_found_and_reported() {
        // Build a raw whose IFD0 has a GPS pointer to a sub-IFD with a latitude.
        // Construct by hand for precise offsets.
        let mut buf = vec![b'I', b'I', 0x2a, 0x00];
        buf.extend_from_slice(&8u32.to_le_bytes());
        // IFD0: Make, CFA, SubIFDs(not needed), GPS pointer, strip offset/count.
        // We'll place the GPS sub-IFD and strip after IFD0.
        let entries: u16 = 5;
        let ifd0_at = 8;
        let table_len = 2 + entries as usize * 12 + 4;
        let gps_at = ifd0_at + table_len;
        // GPS sub-IFD: 1 entry (latitude, 3 rationals) + next=0.
        let gps_table_len = 2 + 12 + 4;
        let lat_at = gps_at + gps_table_len;
        let strip_at = lat_at + 24;
        let make_at = strip_at + 8;

        buf.extend_from_slice(&entries.to_le_bytes());
        let push_entry = |buf: &mut Vec<u8>, tag: u16, ty: u16, cnt: u32, val: u32| {
            buf.extend_from_slice(&tag.to_le_bytes());
            buf.extend_from_slice(&ty.to_le_bytes());
            buf.extend_from_slice(&cnt.to_le_bytes());
            buf.extend_from_slice(&val.to_le_bytes());
        };
        // ascending tag order
        push_entry(&mut buf, 0x0106, 3, 1, 32803); // CFA
        push_entry(&mut buf, 0x010F, 2, 6, make_at as u32); // Make -> "NIKON\0"
        push_entry(&mut buf, 0x0111, 4, 1, strip_at as u32); // StripOffsets
        push_entry(&mut buf, 0x0117, 4, 1, 8); // StripByteCounts
        push_entry(&mut buf, 0x8825, 4, 1, gps_at as u32); // GPS pointer
        buf.extend_from_slice(&0u32.to_le_bytes()); // next IFD

        // GPS sub-IFD.
        buf.extend_from_slice(&1u16.to_le_bytes());
        push_entry(&mut buf, 0x0002, 5, 3, lat_at as u32); // GPSLatitude, 3 rationals
        buf.extend_from_slice(&0u32.to_le_bytes());
        // latitude data (24 bytes)
        buf.extend_from_slice(&[0x11; 24]);
        // strip (8 bytes)
        buf.extend_from_slice(b"SENSOR!!");
        // make
        buf.extend_from_slice(b"NIKON\0");
        buf.push(0);
        buf.push(0);

        let (out, report) = run(&buf);
        assert!(report.found_location, "GPS in a sub-IFD was not reported");
        assert!(out.windows(8).any(|w| w == b"SENSOR!!"), "sensor data was touched");
        // the latitude bytes were zeroed
        assert!(!out.windows(24).any(|w| w == [0x11; 24]), "GPS latitude survived");
    }

    #[test]
    fn an_embedded_preview_jpegs_exif_is_blanked_in_place() {
        // A preview JPEG whose APP1 EXIF holds a real IFD0 with an Artist tag
        // (0x013B) pointing at a secret string. The surgical scrub must remove
        // the value while leaving the IFD table itself intact — that structure
        // is what a raw decoder relies on.
        let secret = b"SECRET-IN-PREVIEW\0";
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"MM");
        tiff.extend_from_slice(&0x2Au16.to_be_bytes());
        tiff.extend_from_slice(&8u32.to_be_bytes()); // IFD0 at offset 8
        tiff.extend_from_slice(&1u16.to_be_bytes()); // one entry
        tiff.extend_from_slice(&0x013Bu16.to_be_bytes()); // Artist
        tiff.extend_from_slice(&2u16.to_be_bytes()); // ASCII
        tiff.extend_from_slice(&(secret.len() as u32).to_be_bytes());
        tiff.extend_from_slice(&26u32.to_be_bytes()); // value at offset 26
        tiff.extend_from_slice(&0u32.to_be_bytes()); // next IFD
        tiff.extend_from_slice(secret); // sits at offset 26
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend_from_slice(&tiff);

        let mut buf = vec![0xFF, 0xD8];
        buf.extend_from_slice(&[0xFF, 0xE1]);
        buf.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
        buf.extend_from_slice(&exif);
        buf.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 1, 1, 0, 0, 0x3F, 0]); // SOS
        buf.extend_from_slice(&[0x12, 0x34]);
        buf.extend_from_slice(&[0xFF, 0xD9]);

        let before = buf.clone();
        let mut report = Report::new(Format::Raw, buf.len());
        scrub_jpeg_in_place(&mut buf, 0, &mut report, "preview");
        assert!(!buf.windows(6).any(|w| w == b"SECRET"), "preview EXIF value survived");
        assert_eq!(buf.len(), before.len(), "in-place scrub changed the length");
        // Structure preserved: SOI/EOI, and the Artist tag entry (0x013B) is
        // still present in the table even though its value is gone.
        assert_eq!(&buf[0..2], &[0xFF, 0xD8]);
        assert_eq!(&buf[buf.len() - 2..], &[0xFF, 0xD9]);
        assert!(buf.windows(2).any(|w| w == 0x013Bu16.to_be_bytes()), "the IFD structure was destroyed");
    }

    #[test]
    fn truncation_and_garbage_never_panic() {
        let raw = RawBuilder::new()
            .ascii(0x9003, "2026:01:02 03:04:05")
            .undefined(0x927C, vec![0xAB; 40])
            .build();
        for n in 0..raw.len() {
            let mut report = Report::new(Format::Raw, n);
            let _ = sanitize(&raw[..n], &crate::Policy::default(), &mut report);
        }
        for junk in [
            b"II\x2a\x00".as_slice(),
            b"FUJIFILMCCD-RAW",
            b"FOVb",
            b"\x00\x00\x00\x18ftypcrx ",
        ] {
            let mut report = Report::new(Format::Raw, junk.len());
            let _ = sanitize(junk, &crate::Policy::default(), &mut report);
        }
    }

    #[test]
    fn a_tag_claiming_a_billion_elements_does_not_hang() {
        // StripOffsets with count = 4 billion but a tiny file. read_uints must
        // cap to what the file could hold rather than loop four billion times.
        let mut buf = vec![b'I', b'I', 0x2a, 0x00];
        buf.extend_from_slice(&8u32.to_le_bytes());
        buf.extend_from_slice(&2u16.to_le_bytes());
        let e = |buf: &mut Vec<u8>, t: u16, ty: u16, c: u32, v: u32| {
            buf.extend_from_slice(&t.to_le_bytes());
            buf.extend_from_slice(&ty.to_le_bytes());
            buf.extend_from_slice(&c.to_le_bytes());
            buf.extend_from_slice(&v.to_le_bytes());
        };
        e(&mut buf, 0x0106, 3, 1, 32803);
        e(&mut buf, 0x0111, 4, 0xFFFF_FFFF, 8); // StripOffsets, absurd count
        buf.extend_from_slice(&0u32.to_le_bytes());
        let mut report = Report::new(Format::Raw, buf.len());
        let _ = sanitize(&buf, &crate::Policy::default(), &mut report); // must return promptly
    }

    #[test]
    fn a_self_referential_sub_ifd_terminates() {
        // IFD0's Exif pointer points back at IFD0.
        let mut buf = vec![b'I', b'I', 0x2a, 0x00];
        buf.extend_from_slice(&8u32.to_le_bytes());
        buf.extend_from_slice(&3u16.to_le_bytes());
        let e = |buf: &mut Vec<u8>, t: u16, ty: u16, c: u32, v: u32| {
            buf.extend_from_slice(&t.to_le_bytes());
            buf.extend_from_slice(&ty.to_le_bytes());
            buf.extend_from_slice(&c.to_le_bytes());
            buf.extend_from_slice(&v.to_le_bytes());
        };
        e(&mut buf, 0x0106, 3, 1, 32803);
        e(&mut buf, 0x010F, 2, 6, 8); // Make offset points into header-ish; bounded read
        e(&mut buf, 0x8769, 4, 1, 8); // Exif pointer -> IFD0 (loop)
        buf.extend_from_slice(&0u32.to_le_bytes());
        let mut report = Report::new(Format::Raw, buf.len());
        let _ = sanitize(&buf, &crate::Policy::default(), &mut report);
    }
}
