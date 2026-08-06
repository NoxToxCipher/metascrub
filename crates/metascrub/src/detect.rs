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
    if input.len() >= 12 && input.starts_with(b"RIFF") && &input[8..12] == b"WEBP" {
        return Format::WebP;
    }
    if input.starts_with(b"%PDF-") {
        return Format::Pdf;
    }
    if let Some(f) = detect_bmff(input) {
        return f;
    }
    if input.starts_with(b"PK\x03\x04") || input.starts_with(b"PK\x05\x06") {
        return detect_zip_flavour(input);
    }
    Format::Unknown
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

    let mut heif = false;
    for brand in brands {
        match brand {
            b"avif" | b"avis" => return Some(Format::Avif),
            b"heic" | b"heix" | b"heim" | b"heis" | b"hevc" | b"hevx" | b"mif1" | b"msf1" => {
                heif = true;
            }
            _ => {}
        }
    }
    heif.then_some(Format::Heif)
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
    fn riff_that_is_not_webp_is_not_webp() {
        assert_eq!(detect(b"RIFF\x00\x00\x00\x00WAVEfmt "), Format::Unknown);
    }

    #[test]
    fn heif_brand_may_be_in_the_compatible_list_only() {
        let mut f = Vec::new();
        f.extend_from_slice(&24u32.to_be_bytes());
        f.extend_from_slice(b"ftypmif1\x00\x00\x00\x00mif1heic");
        assert_eq!(detect(&f), Format::Heif);
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
        let samples: [&[u8]; 5] = [
            b"",
            b"\xFF",
            b"RIFF",
            b"PK\x03\x04",
            b"\x00\x00\x00\x18ftyp",
        ];
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
