//! Format detection from leading bytes.
//!
//! Never from the filename. A `.txt` extension on a JPEG is a naming mistake;
//! a `.jpg` extension on something else is how a parser gets pointed at input
//! it was not written for.

/// A container format this crate recognises.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Format {
    /// JFIF/EXIF JPEG.
    Jpeg,
    /// PNG, including the APNG animation extension.
    Png,
    /// RIFF WebP, still or animated.
    WebP,
    /// HEIF/HEIC still image (ISO base media file format).
    Heif,
    /// AVIF still image (same container as HEIF, AV1 payload).
    Avif,
    /// GIF, static or animated.
    Gif,
    /// TIFF (Tagged Image File Format), the container EXIF itself is built on.
    Tiff,
    /// SVG (Scalable Vector Graphics), an XML document.
    Svg,
    /// An XMP metadata packet, most often a sidecar file (`.xmp`) that travels
    /// beside a photo carrying its author, GPS, dates and edit history.
    Xmp,
    /// A camera raw (DNG, CR2, CR3, NEF, ARW, RW2, ORF, RAF, …). Cleaned in
    /// place rather than rebuilt; see the `raw` module.
    Raw,
    /// A video file (MP4, MOV, MKV, WebM, AVI). Recognised but not yet cleaned:
    /// reported honestly rather than passed off as an unknown blob.
    Video,
    /// An audio file (MP3, M4A, FLAC, OGG, WAV). Recognised but not yet cleaned.
    Audio,
    /// Portable Document Format.
    Pdf,
    /// Office Open XML: .docx, .xlsx, .pptx.
    Ooxml,
    /// OpenDocument: .odt, .ods, .odp.
    OpenDocument,
    /// A ZIP archive that is not one of the document formats above.
    Zip,
    /// Not recognised.
    Unknown,
}

impl std::fmt::Display for Format {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Format::Jpeg => "JPEG",
            Format::Png => "PNG",
            Format::WebP => "WebP",
            Format::Heif => "HEIF",
            Format::Avif => "AVIF",
            Format::Gif => "GIF",
            Format::Tiff => "TIFF",
            Format::Svg => "SVG",
            Format::Xmp => "XMP sidecar",
            Format::Raw => "camera raw",
            Format::Video => "video",
            Format::Audio => "audio",
            Format::Pdf => "PDF",
            Format::Ooxml => "OOXML",
            Format::OpenDocument => "OpenDocument",
            Format::Zip => "ZIP",
            Format::Unknown => "unknown format",
        })
    }
}

const PNG_MAGIC: &[u8] = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

/// Identify the container from `input`'s leading bytes.
pub fn detect(input: &[u8]) -> Format {
    if input.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Format::Jpeg;
    }
    if input.starts_with(PNG_MAGIC) {
        return Format::Png;
    }
    if input.len() >= 12 && input.starts_with(b"RIFF") {
        match &input[8..12] {
            b"WEBP" => return Format::WebP,
            b"AVI " => return Format::Video,
            b"WAVE" => return Format::Audio,
            _ => {}
        }
    }
    if input.starts_with(b"%PDF-") {
        return Format::Pdf;
    }
    // Video and audio containers that are not ISO-BMFF, by their own magic.
    if input.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return Format::Video; // Matroska / WebM (EBML)
    }
    if input.starts_with(b"OggS") {
        return Format::Audio; // Ogg (Vorbis/Opus/FLAC)
    }
    if input.starts_with(b"fLaC") {
        return Format::Audio; // FLAC
    }
    if looks_like_mp3(input) {
        return Format::Audio;
    }
    if input.starts_with(b"GIF87a") || input.starts_with(b"GIF89a") {
        return Format::Gif;
    }
    // Camera raws must be recognised before the generic TIFF and ISO-BMFF checks
    // below, since most of them wear a TIFF or MP4 header. They are cleaned in
    // place, not rebuilt, so they take a different path entirely.
    if crate::raw::flavour(input).is_some() {
        return Format::Raw;
    }
    if let Some(f) = detect_tiff(input) {
        return f;
    }
    if let Some(f) = detect_bmff(input) {
        return f;
    }
    // XMP before SVG: both are XML, but an XMP packet declares itself with an
    // xpacket processing instruction or an <x:xmpmeta> root.
    if looks_like_xmp(input) {
        return Format::Xmp;
    }
    if looks_like_svg(input) {
        return Format::Svg;
    }
    if input.starts_with(b"PK\x03\x04") || input.starts_with(b"PK\x05\x06") {
        return detect_zip_flavour(input);
    }
    Format::Unknown
}

/// TIFF: a byte-order mark (`II` little-endian or `MM` big-endian) followed by
/// the magic number 42.
///
/// Many camera raw formats are TIFF underneath (CR2, NEF, ARW, DNG, ...). We do
/// **not** want to rebuild one of those as if it were a plain TIFF: their image
/// data lives in vendor sub-directories a generic TIFF rewrite would not
/// understand, so the result could be a corrupt raw. A raw is left to the
/// unknown-format path, which returns it untouched and says so, rather than
/// risk damaging a file the user cannot re-shoot.
fn detect_tiff(input: &[u8]) -> Option<Format> {
    if input.len() < 8 {
        return None;
    }
    let big = match &input[0..2] {
        b"II" => false,
        b"MM" => true,
        _ => return None,
    };
    let magic = if big {
        u16::from_be_bytes([input[2], input[3]])
    } else {
        u16::from_le_bytes([input[2], input[3]])
    };
    if magic != 42 {
        return None;
    }
    // CR2 (Canon) stamps "CR" at offset 8. Other raws are harder to tell from a
    // plain TIFF by header alone; those are caught by a content check in the
    // TIFF parser, which declines a file whose tags name a raw. Here we only
    // filter the one that is unambiguous from the first bytes.
    if input.len() >= 10 && &input[8..10] == b"CR" {
        return None; // Canon raw
    }
    Some(Format::Tiff)
}

/// SVG: an XML document whose root (ignoring a prolog, comments and doctype) is
/// an `<svg` element. Detected by content because SVG has no binary magic.
fn looks_like_svg(input: &[u8]) -> bool {
    // Only scan a bounded prefix; a real SVG declares itself early.
    let head = &input[..input.len().min(1024)];
    // Must look like text/XML, and contain an <svg tag before any binary noise.
    let looks_xml = head.starts_with(b"<?xml")
        || head.starts_with(b"<svg")
        || head.starts_with(b"\xEF\xBB\xBF") // UTF-8 BOM
        || head.starts_with(b"<!--");
    if !looks_xml {
        return false;
    }
    window_contains_ci(head, b"<svg")
}

/// XMP: an `<?xpacket` processing instruction or an `<x:xmpmeta` / `xmpmeta`
/// root. Detected by content; a sidecar has no binary magic and any extension.
fn looks_like_xmp(input: &[u8]) -> bool {
    let head = &input[..input.len().min(4096)];
    let looks_xml = head.starts_with(b"<?xpacket")
        || head.starts_with(b"<?xml")
        || head.starts_with(b"\xEF\xBB\xBF")
        || head.starts_with(b"<x:xmpmeta");
    if !looks_xml || !window_contains_ci(head, b"xmpmeta") {
        return false;
    }
    // An SVG can embed an <x:xmpmeta> metadata block, but it is an SVG, not an
    // XMP sidecar — misclassifying it would replace the whole graphic with an
    // empty packet (data loss, reported as a clean). A real XMP sidecar never
    // contains an <svg element, so its presence settles it in favour of SVG.
    !window_contains_ci(head, b"<svg")
}

/// Case-insensitive search for `needle` within `haystack`.
fn window_contains_ci(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| w.eq_ignore_ascii_case(needle))
}

/// ISO base media file format: `[u32 size]["ftyp"][major brand][minor][compatible brands...]`.
fn detect_bmff(input: &[u8]) -> Option<Format> {
    if input.len() < 16 || &input[4..8] != b"ftyp" {
        return None;
    }
    // The major brand alone is not enough: plenty of HEIF files declare `mif1`
    // as major and put the real brand in the compatible list. Scan both.
    let box_size = u32::from_be_bytes([input[0], input[1], input[2], input[3]]) as usize;
    let end = box_size.clamp(16, input.len());
    let brands = input[8..end].chunks_exact(4);

    let major = &input[8..12];
    let mut heif = false;
    let mut audio = false;
    let mut video = false;
    for brand in brands {
        match brand {
            b"avif" | b"avis" => return Some(Format::Avif),
            b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"mif1" | b"msf1" => {
                heif = true;
            }
            // Apple audio brands.
            b"M4A " | b"M4B " | b"F4A " | b"F4B " => audio = true,
            // MP4 / QuickTime / 3GP video brands.
            b"isom" | b"iso2" | b"iso4" | b"iso5" | b"iso6" | b"mp41" | b"mp42" | b"avc1"
            | b"dash" | b"M4V " | b"M4VH" | b"M4VP" | b"qt  " | b"3gp4" | b"3gp5" | b"3g2a"
            | b"mmp4" | b"f4v " => video = true,
            _ => {}
        }
    }
    // Image brands win (a HEIC is not a video); then the specific audio/video
    // signal; a bare `isom`/`mp42` major with nothing else still reads as video.
    if heif {
        return Some(Format::Heif);
    }
    if audio {
        return Some(Format::Audio);
    }
    if video || matches!(major, b"isom" | b"mp42" | b"mp41" | b"qt  ") {
        return Some(Format::Video);
    }
    None
}

/// MP3: an `ID3` tag, or an MPEG audio frame sync (`0xFF` then `0xE`/`0xF` high
/// nibble). The frame-sync form is loose, so it is only trusted when the layer
/// and bitrate bits are not the reserved all-ones pattern.
fn looks_like_mp3(input: &[u8]) -> bool {
    if input.starts_with(b"ID3") {
        return true;
    }
    matches!(input.get(0..2), Some([0xFF, b]) if b & 0xE0 == 0xE0 && b & 0x18 != 0x08 && b & 0x06 != 0x00)
}

/// Distinguish a .docx from a .odt from a plain .zip.
///
/// OOXML puts `[Content_Types].xml` first; OpenDocument puts an uncompressed
/// `mimetype` entry first, with the media type as its content. Both conventions
/// mean the answer sits in the first local file header, so there is no need to
/// walk the central directory just to classify.
fn detect_zip_flavour(input: &[u8]) -> Format {
    const LOCAL_HEADER: usize = 30;
    if input.len() < LOCAL_HEADER || !input.starts_with(b"PK\x03\x04") {
        return Format::Zip;
    }
    let name_len = u16::from_le_bytes([input[26], input[27]]) as usize;
    let extra_len = u16::from_le_bytes([input[28], input[29]]) as usize;
    let Some(name) = input.get(LOCAL_HEADER..LOCAL_HEADER + name_len) else {
        return Format::Zip;
    };

    if name == b"[Content_Types].xml" {
        return Format::Ooxml;
    }
    if name == b"mimetype" {
        let data = LOCAL_HEADER + name_len + extra_len;
        let media_type = input.get(data..data + 60).unwrap_or(&input[data.min(input.len())..]);
        if media_type.starts_with(b"application/vnd.oasis.opendocument") {
            return Format::OpenDocument;
        }
    }
    Format::Zip
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_magics() {
        assert_eq!(detect(&[0xFF, 0xD8, 0xFF, 0xE0]), Format::Jpeg);
        assert_eq!(detect(PNG_MAGIC), Format::Png);
        assert_eq!(detect(b"RIFF\x00\x00\x00\x00WEBPVP8 "), Format::WebP);
        assert_eq!(detect(b"%PDF-1.7\n"), Format::Pdf);
    }

    #[test]
    fn a_riff_of_an_unrecognised_kind_is_unknown() {
        // WebP, AVI and WAVE are claimed; any other RIFF subtype is not.
        assert_eq!(detect(b"RIFF\x00\x00\x00\x00XXXXjunk"), Format::Unknown);
    }

    #[test]
    fn heif_brand_may_be_in_the_compatible_list_only() {
        let mut f = Vec::new();
        f.extend_from_slice(&24u32.to_be_bytes());
        f.extend_from_slice(b"ftypmif1\x00\x00\x00\x00mif1heic");
        assert_eq!(detect(&f), Format::Heif);
    }

    #[test]
    fn video_and_audio_containers_are_recognised() {
        // ISO-BMFF brands.
        let mp4 = {
            let mut f = 24u32.to_be_bytes().to_vec();
            f.extend_from_slice(b"ftypmp42\x00\x00\x00\x00mp42isom");
            f
        };
        assert_eq!(detect(&mp4), Format::Video);
        let m4a = {
            let mut f = 24u32.to_be_bytes().to_vec();
            f.extend_from_slice(b"ftypM4A \x00\x00\x00\x00M4A isom");
            f
        };
        assert_eq!(detect(&m4a), Format::Audio);
        // Non-BMFF magics.
        assert_eq!(detect(b"\x1a\x45\xdf\xa3\x01\x00\x00\x00"), Format::Video); // Matroska/WebM
        assert_eq!(detect(b"RIFF\x00\x00\x00\x00AVI LIST"), Format::Video);
        assert_eq!(detect(b"RIFF\x00\x00\x00\x00WAVEfmt "), Format::Audio);
        assert_eq!(detect(b"OggS\x00\x02\x00\x00"), Format::Audio);
        assert_eq!(detect(b"fLaC\x00\x00\x00\x22"), Format::Audio);
        assert_eq!(detect(b"ID3\x04\x00\x00\x00\x00\x00\x00"), Format::Audio);
    }

    #[test]
    fn a_heif_is_not_mistaken_for_a_video_after_the_brand_change() {
        // The video/audio branch must not swallow image or raw BMFF files.
        let mut heif = Vec::new();
        heif.extend_from_slice(&24u32.to_be_bytes());
        heif.extend_from_slice(b"ftypheic\x00\x00\x00\x00mif1heic");
        assert_eq!(detect(&heif), Format::Heif);

        let mut cr3 = Vec::new();
        cr3.extend_from_slice(&24u32.to_be_bytes());
        cr3.extend_from_slice(b"ftypcrx \x00\x00\x00\x00crx isom");
        assert_eq!(detect(&cr3), Format::Raw, "a Canon CR3 must still route to raw, not video");
    }

    #[test]
    fn avif_wins_over_the_shared_heif_brand() {
        let mut f = Vec::new();
        f.extend_from_slice(&24u32.to_be_bytes());
        f.extend_from_slice(b"ftypavif\x00\x00\x00\x00mif1avif");
        assert_eq!(detect(&f), Format::Avif);
    }

    #[test]
    fn zip_flavours_are_told_apart_by_their_first_entry() {
        assert_eq!(detect(&local_header(b"[Content_Types].xml", b"")), Format::Ooxml);
        assert_eq!(
            detect(&local_header(b"mimetype", b"application/vnd.oasis.opendocument.text")),
            Format::OpenDocument
        );
        assert_eq!(detect(&local_header(b"random.txt", b"hello")), Format::Zip);
    }

    #[test]
    fn truncated_inputs_do_not_panic() {
        let samples: [&[u8]; 5] = [b"", b"\xFF", b"RIFF", b"PK\x03\x04", b"\x00\x00\x00\x18ftyp"];
        for s in samples {
            let _ = detect(s);
        }
        // Every prefix of a well-formed header must also be survivable.
        let full = local_header(b"[Content_Types].xml", b"x");
        for n in 0..full.len() {
            let _ = detect(&full[..n]);
        }
    }

    fn local_header(name: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(b"PK\x03\x04");
        v.extend_from_slice(&[0u8; 22]); // version..uncompressed size
        v.extend_from_slice(&(name.len() as u16).to_le_bytes());
        v.extend_from_slice(&0u16.to_le_bytes()); // extra len
        v.extend_from_slice(name);
        v.extend_from_slice(body);
        v
    }
}
