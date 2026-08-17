//! HEIF / HEIC / AVIF: locate the metadata items and overwrite them in place.
//!
//! This is the one image format here that is **not** rebuilt from an allowlist,
//! and the reason is worth stating plainly because it sets the assurance level.
//!
//! HEIF is an ISO base media container. Metadata is not stored inline next to
//! the image the way a JPEG segment is. Instead the `meta` box holds an item
//! table (`iinf`) naming an EXIF item and an XMP item, and a location table
//! (`iloc`) giving the **absolute file offset** of each item's bytes, which
//! generally sit in the big `mdat` blob alongside the image tiles. Excising
//! those bytes shifts everything after them, which invalidates every other
//! offset in `iloc`, so a real removal means rewriting the location table, the
//! item table, and any `iref` that points at them.
//!
//! That rewrite is where a metadata remover gets dangerous: a mistake in the
//! offset arithmetic produces a file that still opens (the tiles it needs are
//! usually early) while quietly holding onto some of what we claimed to
//! remove. So this module takes the conservative route instead. It resolves
//! each metadata item's byte range and **overwrites that range in place**, with
//! an empty-but-valid EXIF block or an empty XMP packet and zero padding. The
//! file length never changes, so every offset in the container stays correct,
//! and the secrets are gone because their bytes are gone.
//!
//! What that costs, and why the result is [`Assurance::BestEffort`]:
//!
//! - The item table still says an EXIF item exists. It now points at an empty
//!   one. Nothing is disclosed, but a tool that reports "this file has EXIF"
//!   will still say so.
//! - The container is not rebuilt, so a box type we do not know about is
//!   carried through rather than dropped. That is the opposite of the guarantee
//!   the JPEG and PNG paths give.
//!
//! [`Assurance::BestEffort`]: crate::Assurance

use crate::error::Error;
use crate::exif;
use crate::policy::Policy;
use crate::report::{Assurance, Kind, Report};
use crate::util::Reader;

const FORMAT: &str = "HEIF";

/// Ceiling on boxes walked and items parsed. Real files are nowhere near this;
/// the limit exists so a crafted file cannot make us spin.
const MAX_BOXES: usize = 4096;
const MAX_ITEMS: usize = 4096;

/// The UUID that marks a top-level XMP box, as written by Adobe tools.
const XMP_UUID: [u8; 16] = [
    0xBE, 0x7A, 0xCF, 0xCB, 0x97, 0xA9, 0x42, 0xE8, 0x9C, 0x71, 0x99, 0x94, 0x91, 0xE3, 0xAF, 0xAC,
];

/// One box's position in the file.
#[derive(Debug, Clone, Copy)]
struct BoxSpan {
    ty: [u8; 4],
    /// First byte of the payload, after the size/type header.
    body: usize,
    /// One past the last byte of the payload.
    end: usize,
}

/// What kind of metadata an item holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Exif,
    Xmp,
}

pub(crate) fn sanitize(
    input: &[u8],
    _policy: &Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    // The container is overwritten in place, never rebuilt, so we cannot claim
    // the allowlist guarantee the other image formats get.
    report.assurance = Assurance::BestEffort;

    let top = walk(input, 0, input.len())?;
    if !top.iter().any(|b| &b.ty == b"ftyp") {
        return Err(Error::malformed(FORMAT, "no ftyp box"));
    }

    let mut out = input.to_vec();

    let Some(meta) = top.iter().find(|b| &b.ty == b"meta") else {
        report.warn("this file has no metadata box, so there was nothing to remove");
        scrub_uuid_boxes(&mut out, &top, report);
        return Ok(out);
    };

    // `meta` is a full box: one version byte and three flag bytes precede the
    // children.
    let children_start = meta.body.saturating_add(4);
    let children = walk(input, children_start.min(meta.end), meta.end)?;

    let items = parse_iinf(input, &children, report);
    let locations = parse_iloc(input, &children, report);
    let idat = children.iter().find(|b| &b.ty == b"idat").map(|b| b.body);

    // The container is not rebuilt, so an embedded ICC colour profile is carried
    // through. Disclose it the way the other formats do rather than letting it
    // stay silent.
    disclose_kept_icc(input, &children, report);

    let mut touched = 0usize;
    for (item_id, kind) in &items {
        let Some(extents) = locations.get(item_id) else {
            report.warn(format!(
                "item {item_id} is declared as {} but has no entry in the location table, \
                 so its bytes could not be found",
                match kind {
                    ItemKind::Exif => "EXIF",
                    ItemKind::Xmp => "XMP",
                }
            ));
            continue;
        };

        for extent in extents {
            let base = match extent.construction {
                Construction::File => 0,
                Construction::Idat => match idat {
                    Some(offset) => offset,
                    None => {
                        report.warn(format!(
                            "item {item_id} points into an idat box that is not present"
                        ));
                        continue;
                    }
                },
                Construction::Item => {
                    report.warn(format!(
                        "item {item_id} is stored inside another item, which this version does \
                         not follow; its metadata has NOT been removed"
                    ));
                    continue;
                }
            };

            let Some(range) = resolve(base, extent, out.len()) else {
                report.warn(format!("item {item_id} points outside the file; ignored"));
                continue;
            };

            match kind {
                ItemKind::Exif => {
                    inspect_heif_exif(&out[range.clone()], report);
                    report.removed(Kind::Exif, format!("meta item {item_id}"), range.len());
                }
                ItemKind::Xmp => {
                    report.removed(Kind::Xmp, format!("meta item {item_id}"), range.len());
                }
            }
            overwrite(&mut out[range], *kind);
            touched += 1;
        }
    }

    scrub_uuid_boxes(&mut out, &top, report);

    if touched > 0 {
        report.warn(
            "the metadata in this file was overwritten where it sat rather than cut out, \
             because removing it outright would break every stored offset in the container; \
             the values are gone but the empty entries that held them remain",
        );
    }
    Ok(out)
}

/// Overwrite one metadata extent with an empty but structurally valid stand-in.
///
/// Zero-filling alone would work for privacy, but a decoder that follows the
/// item table into an all-zero EXIF block can error out on the whole image.
/// Writing a well-formed empty block keeps the file openable.
fn overwrite(dst: &mut [u8], kind: ItemKind) {
    dst.fill(0);
    match kind {
        ItemKind::Exif => {
            // A HEIF EXIF item begins with a 4-byte offset to the TIFF header.
            let empty = exif::empty_tiff();
            if dst.len() >= 4 + empty.len() {
                dst[..4].copy_from_slice(&0u32.to_be_bytes());
                dst[4..4 + empty.len()].copy_from_slice(&empty);
            }
        }
        ItemKind::Xmp => {
            const EMPTY: &[u8] = b"<?xpacket begin=\"\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\
                                   <x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/><?xpacket end=\"w\"?>";
            if dst.len() >= EMPTY.len() {
                dst[..EMPTY.len()].copy_from_slice(EMPTY);
                // XMP packets are conventionally whitespace-padded, so the tail
                // stays valid rather than becoming trailing NULs inside XML.
                dst[EMPTY.len()..].fill(b' ');
            }
        }
    }
}

/// Read a HEIF EXIF item for reporting: skip the 4-byte TIFF header offset,
/// then hand the TIFF block to the shared inspector.
fn inspect_heif_exif(item: &[u8], report: &mut Report) {
    let Some(offset) = item.get(..4).and_then(|b| b.try_into().ok()).map(u32::from_be_bytes) else {
        return;
    };
    let start = 4usize.saturating_add(offset as usize);
    let found = exif::inspect_tiff(item.get(start..).unwrap_or_default());
    report.found_location |= found.gps;
    if found.maker_note {
        report.removed(Kind::MakerNote, "meta EXIF maker note", 0);
    }
    if found.thumbnail {
        report.removed(Kind::Thumbnail, "meta EXIF IFD1", 0);
    }
}

/// Disclose a kept ICC colour profile. HEIF stores it as a `colr` box of type
/// `prof` (an ICC profile) or `rICC` (a restricted ICC profile) inside
/// `meta > iprp > ipco`. Because this format is overwritten in place rather than
/// rebuilt, the profile is carried through; naming it gives the same honest
/// disclosure the JPEG, PNG, WebP and TIFF paths provide, rather than letting an
/// iPhone HEIC keep its profile silently. A `colr` of type `nclx` is only colour
/// parameters, not an embeddable profile, so it is left alone.
fn disclose_kept_icc(input: &[u8], meta_children: &[BoxSpan], report: &mut Report) {
    let Some(iprp) = meta_children.iter().find(|b| &b.ty == b"iprp") else { return };
    let Ok(iprp_children) = walk(input, iprp.body, iprp.end) else { return };
    let Some(ipco) = iprp_children.iter().find(|b| &b.ty == b"ipco") else { return };
    let Ok(props) = walk(input, ipco.body, ipco.end) else { return };
    for colr in props.iter().filter(|b| &b.ty == b"colr") {
        if let Some(kind) = input.get(colr.body..colr.body + 4) {
            if kind == b"prof" || kind == b"rICC" {
                report.retain_icc();
                return;
            }
        }
    }
}

/// Blank the payload of any top-level `uuid` box carrying XMP.
fn scrub_uuid_boxes(out: &mut [u8], boxes: &[BoxSpan], report: &mut Report) {
    for b in boxes {
        if &b.ty != b"uuid" {
            continue;
        }
        let Some(usertype) = out.get(b.body..b.body + 16) else { continue };
        if usertype != XMP_UUID {
            continue;
        }
        let payload = b.body + 16;
        if payload < b.end {
            report.removed(Kind::Xmp, "uuid XMP box", b.end - payload);
            out[payload..b.end].fill(b' ');
        }
    }
}

/// Walk the boxes in `[start, end)`, one level deep.
fn walk(buf: &[u8], start: usize, end: usize) -> crate::Result<Vec<BoxSpan>> {
    let mut boxes = Vec::new();
    let mut pos = start;
    let end = end.min(buf.len());

    while pos + 8 <= end {
        if boxes.len() >= MAX_BOXES {
            return Err(Error::malformed(FORMAT, "unreasonable number of boxes"));
        }
        let size32 = u32::from_be_bytes([buf[pos], buf[pos + 1], buf[pos + 2], buf[pos + 3]]);
        let ty: [u8; 4] = [buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]];

        let (header, size) = match size32 {
            // A 64-bit size follows the type.
            1 => {
                let Some(bytes) = buf.get(pos + 8..pos + 16) else { break };
                let large = u64::from_be_bytes(bytes.try_into().unwrap_or([0; 8]));
                // `large as usize` would truncate on a 32-bit target (the crate
                // cross-compiles to 32-bit Android ABIs), and a box mis-sized by
                // a dropped high word could make the metadata box be walked wrong
                // or missed — a false clean. The input is capped at 2 GB, so any
                // size past usize is malformed regardless of platform.
                let Ok(size) = usize::try_from(large) else {
                    return Err(Error::malformed(
                        FORMAT,
                        "64-bit box size exceeds addressable range",
                    ));
                };
                (16usize, size)
            }
            // Runs to the end of the enclosing range.
            0 => (8usize, end - pos),
            n => (8usize, n as usize),
        };

        if size < header {
            return Err(Error::malformed(FORMAT, format!("box {ty:?} declares size {size}")));
        }
        let box_end = pos.saturating_add(size).min(end);
        boxes.push(BoxSpan { ty, body: pos + header, end: box_end });

        // A zero-length advance would loop forever.
        let next = pos.saturating_add(size);
        if next <= pos {
            return Err(Error::malformed(FORMAT, "zero-length box"));
        }
        pos = next;
    }
    Ok(boxes)
}

/// Parse `iinf` into a list of `(item_id, kind)` for the items we care about.
fn parse_iinf(buf: &[u8], children: &[BoxSpan], report: &mut Report) -> Vec<(u32, ItemKind)> {
    let Some(iinf) = children.iter().find(|b| &b.ty == b"iinf") else { return Vec::new() };
    let mut r = Reader::new(&buf[..iinf.end.min(buf.len())]);
    if r.seek(iinf.body).is_none() {
        return Vec::new();
    }
    let Some(version) = r.u8() else { return Vec::new() };
    let _flags = r.take(3);

    let count = if version == 0 { r.u16_be().map(u32::from) } else { r.u32_be() };
    let Some(count) = count else { return Vec::new() };
    if count as usize > MAX_ITEMS {
        report.warn("the item table declares an implausible number of entries; it was ignored");
        return Vec::new();
    }

    let entries = match walk(buf, r.pos(), iinf.end) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };

    let mut items = Vec::new();
    for entry in entries.iter().filter(|b| &b.ty == b"infe") {
        if let Some(found) = parse_infe(buf, entry) {
            items.push(found);
        }
    }
    items
}

/// Parse one `infe` entry. Only versions 2 and 3 carry an item type, and only
/// those versions are used by HEIF and AVIF.
fn parse_infe(buf: &[u8], span: &BoxSpan) -> Option<(u32, ItemKind)> {
    let mut r = Reader::new(buf.get(..span.end.min(buf.len()))?);
    r.seek(span.body)?;
    let version = r.u8()?;
    r.take(3)?; // flags

    let item_id = match version {
        2 => r.u16_be()? as u32,
        3 => r.u32_be()?,
        _ => return None, // versions 0 and 1 predate item types
    };
    r.u16_be()?; // item_protection_index
    let item_type = r.take(4)?;

    match item_type {
        b"Exif" => Some((item_id, ItemKind::Exif)),
        b"mime" => {
            // item_name, then content_type. XMP declares application/rdf+xml.
            let rest = buf.get(r.pos()..span.end.min(buf.len()))?;
            let mut fields = rest.split(|&b| b == 0).skip(1);
            let content_type = fields.next()?;
            content_type.starts_with(b"application/rdf+xml").then_some((item_id, ItemKind::Xmp))
        }
        _ => None,
    }
}

/// Where an extent's offset is measured from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Construction {
    File,
    Idat,
    Item,
}

#[derive(Debug, Clone, Copy)]
struct Extent {
    construction: Construction,
    offset: u64,
    length: u64,
}

fn resolve(base: usize, extent: &Extent, file_len: usize) -> Option<std::ops::Range<usize>> {
    let start = (base as u64).checked_add(extent.offset)?;
    let end = start.checked_add(extent.length)?;
    let (start, end) = (usize::try_from(start).ok()?, usize::try_from(end).ok()?);
    (end <= file_len && start < end).then_some(start..end)
}

/// Parse `iloc` into `item_id -> extents`.
fn parse_iloc(
    buf: &[u8],
    children: &[BoxSpan],
    report: &mut Report,
) -> std::collections::BTreeMap<u32, Vec<Extent>> {
    let mut map = std::collections::BTreeMap::new();
    let Some(iloc) = children.iter().find(|b| &b.ty == b"iloc") else { return map };

    let mut r = Reader::new(&buf[..iloc.end.min(buf.len())]);
    if r.seek(iloc.body).is_none() {
        return map;
    }
    let (Some(version), Some(_flags)) = (r.u8(), r.take(3)) else { return map };

    let Some(sizes) = r.u8() else { return map };
    let (offset_size, length_size) = (sizes >> 4, sizes & 0x0F);
    let Some(bases) = r.u8() else { return map };
    let (base_offset_size, index_size) = (bases >> 4, bases & 0x0F);

    let count = if version < 2 { r.u16_be().map(u32::from) } else { r.u32_be() };
    let Some(count) = count else { return map };
    if count as usize > MAX_ITEMS {
        report.warn("the location table declares an implausible number of entries; it was ignored");
        return map;
    }

    for _ in 0..count {
        let Some(item_id) = (if version < 2 { r.u16_be().map(u32::from) } else { r.u32_be() })
        else {
            return map;
        };

        let construction = if version == 1 || version == 2 {
            match r.u16_be().map(|v| v & 0x0F) {
                Some(0) => Construction::File,
                Some(1) => Construction::Idat,
                Some(_) => Construction::Item,
                None => return map,
            }
        } else {
            Construction::File
        };

        if r.u16_be().is_none() {
            return map; // data_reference_index
        }
        let Some(base_offset) = read_sized(&mut r, base_offset_size) else { return map };
        let Some(extent_count) = r.u16_be() else { return map };

        let mut extents = Vec::new();
        for _ in 0..extent_count {
            if (version == 1 || version == 2)
                && index_size > 0
                && read_sized(&mut r, index_size).is_none()
            {
                return map;
            }
            let (Some(offset), Some(length)) =
                (read_sized(&mut r, offset_size), read_sized(&mut r, length_size))
            else {
                return map;
            };
            extents.push(Extent {
                construction,
                offset: base_offset.saturating_add(offset),
                length,
            });
        }
        map.insert(item_id, extents);
    }
    map
}

/// Read a field whose width is declared in the header: 0, 4 or 8 bytes.
/// A width of 0 means the field is absent and its value is zero.
fn read_sized(r: &mut Reader<'_>, width: u8) -> Option<u64> {
    match width {
        0 => Some(0),
        4 => r.u32_be().map(u64::from),
        8 => r.u64_be(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    fn bx(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(ty);
        v.extend_from_slice(body);
        v
    }

    fn ftyp() -> Vec<u8> {
        bx(b"ftyp", b"heic\0\0\0\0mif1heic")
    }

    /// infe version 2 for an item of the given type.
    fn infe(id: u16, item_type: &[u8; 4], name: &[u8], content_type: Option<&[u8]>) -> Vec<u8> {
        let mut b = vec![2, 0, 0, 0];
        b.extend_from_slice(&id.to_be_bytes());
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(item_type);
        b.extend_from_slice(name);
        b.push(0);
        if let Some(ct) = content_type {
            b.extend_from_slice(ct);
            b.push(0);
        }
        bx(b"infe", &b)
    }

    fn iinf(entries: &[Vec<u8>]) -> Vec<u8> {
        let mut b = vec![0, 0, 0, 0];
        b.extend_from_slice(&(entries.len() as u16).to_be_bytes());
        for e in entries {
            b.extend_from_slice(e);
        }
        bx(b"iinf", &b)
    }

    /// iloc version 1, 4-byte offsets and lengths, no base offsets or indices.
    fn iloc(items: &[(u16, u32, u32)]) -> Vec<u8> {
        let mut b = vec![1, 0, 0, 0];
        b.push(0x44); // offset_size = 4, length_size = 4
        b.push(0x00); // base_offset_size = 0, index_size = 0
        b.extend_from_slice(&(items.len() as u16).to_be_bytes());
        for &(id, offset, length) in items {
            b.extend_from_slice(&id.to_be_bytes());
            b.extend_from_slice(&0u16.to_be_bytes()); // construction_method 0
            b.extend_from_slice(&0u16.to_be_bytes()); // data_reference_index
            b.extend_from_slice(&1u16.to_be_bytes()); // one extent
            b.extend_from_slice(&offset.to_be_bytes());
            b.extend_from_slice(&length.to_be_bytes());
        }
        bx(b"iloc", &b)
    }

    /// Assemble a HEIF file, patching the iloc offsets once mdat's real
    /// position is known. `payloads` are laid out in order inside mdat.
    fn heif(payloads: &[(u16, &[u8], bool)]) -> Vec<u8> {
        let entries: Vec<Vec<u8>> = payloads
            .iter()
            .map(|&(id, _, is_exif)| {
                if is_exif {
                    infe(id, b"Exif", b"", None)
                } else {
                    infe(id, b"mime", b"", Some(b"application/rdf+xml"))
                }
            })
            .collect();

        // Build with placeholder offsets first, to learn the layout.
        let placeholder: Vec<(u16, u32, u32)> =
            payloads.iter().map(|&(id, p, _)| (id, 0, p.len() as u32)).collect();
        let meta_body = {
            let mut b = vec![0, 0, 0, 0]; // full box header
            b.extend_from_slice(&bx(b"hdlr", &[0u8; 20]));
            b.extend_from_slice(&iinf(&entries));
            b.extend_from_slice(&iloc(&placeholder));
            b
        };
        let head_len = ftyp().len() + bx(b"meta", &meta_body).len();
        let mdat_payload_start = head_len + 8;

        let mut cursor = mdat_payload_start as u32;
        let real: Vec<(u16, u32, u32)> = payloads
            .iter()
            .map(|&(id, p, _)| {
                let at = cursor;
                cursor += p.len() as u32;
                (id, at, p.len() as u32)
            })
            .collect();

        let meta_body = {
            let mut b = vec![0, 0, 0, 0];
            b.extend_from_slice(&bx(b"hdlr", &[0u8; 20]));
            b.extend_from_slice(&iinf(&entries));
            b.extend_from_slice(&iloc(&real));
            b
        };

        let mut mdat = Vec::new();
        for &(_, p, _) in payloads {
            mdat.extend_from_slice(p);
        }

        let mut file = ftyp();
        file.extend_from_slice(&bx(b"meta", &meta_body));
        file.extend_from_slice(&bx(b"mdat", &mdat));
        file
    }

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Heif, input.len());
        let out = sanitize(input, &Policy::default(), &mut report).expect("valid heif");
        (out, report)
    }

    fn exif_payload() -> Vec<u8> {
        // 4-byte TIFF header offset, then a TIFF block with a GPS sub-IFD.
        let mut p = 0u32.to_be_bytes().to_vec();
        p.extend_from_slice(b"MM");
        p.extend_from_slice(&42u16.to_be_bytes());
        p.extend_from_slice(&8u32.to_be_bytes());
        p.extend_from_slice(&1u16.to_be_bytes());
        p.extend_from_slice(&0x8825u16.to_be_bytes());
        p.extend_from_slice(&4u16.to_be_bytes());
        p.extend_from_slice(&1u32.to_be_bytes());
        p.extend_from_slice(&26u32.to_be_bytes());
        p.extend_from_slice(&0u32.to_be_bytes());
        p.extend_from_slice(&1u16.to_be_bytes());
        p.extend_from_slice(&0x0002u16.to_be_bytes());
        p.extend_from_slice(&5u16.to_be_bytes());
        p.extend_from_slice(&3u32.to_be_bytes());
        p.extend_from_slice(&0u32.to_be_bytes());
        p.extend_from_slice(&0u32.to_be_bytes());
        p.extend_from_slice(b"CameraSerial9988 and other private notes");
        p
    }

    #[test]
    fn exif_and_xmp_item_payloads_are_overwritten() {
        let xmp = b"<x:xmpmeta><dc:creator>Jane Q. Photographer</dc:creator></x:xmpmeta>";
        let input = heif(&[(1, &exif_payload(), true), (2, xmp, false)]);
        let (out, report) = run(&input);

        assert_eq!(out.len(), input.len(), "in-place overwriting must not resize the file");
        assert!(!out.windows(16).any(|w| w == b"CameraSerial9988"));
        assert!(!out.windows(20).any(|w| w == b"Jane Q. Photographer"));

        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Exif));
        assert!(kinds.contains(&Kind::Xmp));
        assert!(report.found_location, "the GPS sub-IFD must be reported before it is blanked");
    }

    #[test]
    fn the_result_is_never_advertised_as_a_complete_strip() {
        let (_, report) = run(&heif(&[(1, &exif_payload(), true)]));
        assert_eq!(report.assurance, Assurance::BestEffort);
        assert!(
            report.warnings.iter().any(|w| w.contains("overwritten where it sat")),
            "the user has to be told this was not a rebuild"
        );
    }

    #[test]
    fn what_replaces_the_exif_is_a_valid_empty_block() {
        let (out, _) = run(&heif(&[(1, &exif_payload(), true)]));
        let at = out.windows(2).rposition(|w| w == b"MM").expect("empty TIFF written");
        let found = exif::inspect_tiff(&out[at..]);
        assert_eq!(found, exif::ExifFindings::default(), "the stand-in must hold nothing");
    }

    #[test]
    fn image_data_outside_the_metadata_extents_is_untouched() {
        let tiles: &[u8] = b"\x00\x00\x00\x00PRETEND-IMAGE-TILE-DATA";
        let mut input = heif(&[(1, &exif_payload(), true)]);
        input.extend_from_slice(tiles);
        let (out, _) = run(&input);
        assert!(out.windows(tiles.len()).any(|w| w == tiles));
    }

    #[test]
    fn a_uuid_xmp_box_is_blanked() {
        let mut body = XMP_UUID.to_vec();
        body.extend_from_slice(b"<x:xmpmeta>GPS 51.5074 -0.1278</x:xmpmeta>");
        let mut input = ftyp();
        input.extend_from_slice(&bx(b"uuid", &body));

        let (out, report) = run(&input);
        assert!(!out.windows(7).any(|w| w == b"51.5074"));
        assert!(report.removed.iter().any(|r| r.location == "uuid XMP box"));
    }

    /// meta > iprp > ipco > colr('prof' ...) holds an ICC profile that this
    /// format keeps; it must be disclosed like the other formats do.
    fn heif_with_colr(kind: &[u8]) -> Vec<u8> {
        let colr = bx(b"colr", kind);
        let ipco = bx(b"ipco", &colr);
        let iprp = bx(b"iprp", &ipco);
        let mut meta_body = vec![0, 0, 0, 0];
        meta_body.extend_from_slice(&iprp);
        let mut input = ftyp();
        input.extend_from_slice(&bx(b"meta", &meta_body));
        input
    }

    #[test]
    fn a_kept_icc_colour_profile_is_disclosed() {
        let (_, report) = run(&heif_with_colr(b"prof\x00\x00\x00\x0cacsp"));
        assert!(
            report.retained.iter().any(|r| r.what == "ICC colour profile"),
            "a HEIF ICC colour profile must be disclosed"
        );
    }

    #[test]
    fn an_nclx_colour_box_is_not_treated_as_an_icc_profile() {
        // nclx is colour parameters, not an embeddable profile: nothing to disclose.
        let (_, report) = run(&heif_with_colr(b"nclx\x00\x01\x00\x01\x00\x01\x80"));
        assert!(report.retained.iter().all(|r| r.what != "ICC colour profile"));
    }

    #[test]
    fn a_file_with_no_metadata_box_is_reported_as_such_and_returned_intact() {
        let mut input = ftyp();
        input.extend_from_slice(&bx(b"mdat", b"just image data"));
        let (out, report) = run(&input);
        assert_eq!(out, input);
        assert!(report.warnings.iter().any(|w| w.contains("no metadata box")));
        assert!(report.is_clean());
    }

    #[test]
    fn an_item_stored_inside_another_item_is_declared_not_removed() {
        // construction_method 2 is a case we decline rather than guess at, and
        // declining loudly is the point.
        let mut b = vec![1, 0, 0, 0, 0x44, 0x00];
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&1u16.to_be_bytes()); // item 1
        b.extend_from_slice(&2u16.to_be_bytes()); // construction_method 2
        b.extend_from_slice(&0u16.to_be_bytes());
        b.extend_from_slice(&1u16.to_be_bytes());
        b.extend_from_slice(&0u32.to_be_bytes());
        b.extend_from_slice(&4u32.to_be_bytes());

        let mut meta_body = vec![0, 0, 0, 0];
        meta_body.extend_from_slice(&iinf(&[infe(1, b"Exif", b"", None)]));
        meta_body.extend_from_slice(&bx(b"iloc", &b));

        let mut input = ftyp();
        input.extend_from_slice(&bx(b"meta", &meta_body));

        let (_, report) = run(&input);
        assert!(report.warnings.iter().any(|w| w.contains("has NOT been removed")));
    }

    #[test]
    fn an_extent_pointing_outside_the_file_is_ignored_not_followed() {
        let mut meta_body = vec![0, 0, 0, 0];
        meta_body.extend_from_slice(&iinf(&[infe(1, b"Exif", b"", None)]));
        meta_body.extend_from_slice(&iloc(&[(1, 0xFFFF_0000, 0xFFFF)]));

        let mut input = ftyp();
        input.extend_from_slice(&bx(b"meta", &meta_body));

        let (out, report) = run(&input);
        assert_eq!(out, input);
        assert!(report.warnings.iter().any(|w| w.contains("outside the file")));
    }

    #[test]
    fn a_box_declaring_a_size_smaller_than_its_header_is_rejected() {
        let mut input = ftyp();
        input.extend_from_slice(&2u32.to_be_bytes());
        input.extend_from_slice(b"junk");
        let mut report = Report::new(Format::Heif, input.len());
        assert!(sanitize(&input, &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = heif(&[(1, &exif_payload(), true), (2, b"<x:xmpmeta/>", false)]);
        for n in 0..full.len() {
            let mut report = Report::new(Format::Heif, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }

    #[test]
    fn read_sized_rejects_widths_the_format_does_not_define() {
        let data = [0u8; 8];
        assert_eq!(read_sized(&mut Reader::new(&data), 0), Some(0));
        assert_eq!(read_sized(&mut Reader::new(&data), 4), Some(0));
        assert_eq!(read_sized(&mut Reader::new(&data), 8), Some(0));
        assert_eq!(read_sized(&mut Reader::new(&data), 2), None);
    }
}
