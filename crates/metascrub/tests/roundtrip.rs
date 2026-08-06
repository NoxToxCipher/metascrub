//! End-to-end tests against genuinely encoded files.
//!
//! The unit tests inside each module work on hand-built containers, which is
//! the right way to exercise the parsing edge cases but proves nothing about
//! whether the output is still a usable file. These tests encode real images
//! with a real encoder, push metadata into them, sanitize, and then **decode
//! the result and compare pixels**. That is the property that actually matters
//! to someone sending a photograph: the metadata is gone and the picture is
//! not.
//!
//! Fixtures are generated rather than committed. A binary blob in a repository
//! is a thing nobody reviews, and a generated one can be varied, truncated and
//! corrupted by the test itself.

use metascrub::{sanitize, Assurance, Format, Kind, Policy};

/// A distinctive string planted in metadata. If it appears anywhere in the
/// output, something carried it through.
const CANARY: &str = "CANARY-SERIAL-4417";

// ---------------------------------------------------------------------------
// Fixture construction
// ---------------------------------------------------------------------------

/// A small image with enough variation that a re-encode would be detectable.
fn source_pixels() -> image::RgbImage {
    image::RgbImage::from_fn(32, 24, |x, y| {
        image::Rgb([(x * 8) as u8, (y * 10) as u8, ((x * y) % 251) as u8])
    })
}

fn encode(format: image::ImageFormat) -> Vec<u8> {
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(source_pixels())
        .write_to(&mut out, format)
        .expect("the test encoder should succeed");
    out.into_inner()
}

/// An EXIF/TIFF block with an orientation tag, a GPS sub-IFD holding a real
/// latitude, and a maker note carrying the canary.
fn exif_block() -> Vec<u8> {
    let mut t = Vec::new();
    t.extend_from_slice(b"MM");
    t.extend_from_slice(&42u16.to_be_bytes());
    t.extend_from_slice(&8u32.to_be_bytes());

    // IFD0: orientation, GPS pointer, Exif sub-IFD pointer.
    t.extend_from_slice(&3u16.to_be_bytes());
    push_entry(&mut t, 0x0112, 3, 1, 6 << 16); // orientation = rotate 90
    push_entry(&mut t, 0x8825, 4, 1, 0); // GPS pointer, patched below
    push_entry(&mut t, 0x8769, 4, 1, 0); // Exif pointer, patched below
    t.extend_from_slice(&0u32.to_be_bytes()); // no IFD1

    let gps_at = t.len() as u32;
    t.extend_from_slice(&1u16.to_be_bytes());
    push_entry(&mut t, 0x0002, 5, 3, 0); // GPSLatitude
    t.extend_from_slice(&0u32.to_be_bytes());

    let exif_at = t.len() as u32;
    t.extend_from_slice(&1u16.to_be_bytes());
    let note_at = (t.len() + 12 + 4) as u32;
    push_entry(&mut t, 0x927C, 7, CANARY.len() as u32, note_at); // MakerNote
    t.extend_from_slice(&0u32.to_be_bytes());
    t.extend_from_slice(CANARY.as_bytes());

    // Patch the two sub-IFD pointers now that their offsets are known.
    let ifd0_entry = |n: usize| 8 + 2 + n * 12 + 8;
    t[ifd0_entry(1)..ifd0_entry(1) + 4].copy_from_slice(&gps_at.to_be_bytes());
    t[ifd0_entry(2)..ifd0_entry(2) + 4].copy_from_slice(&exif_at.to_be_bytes());
    t
}

fn push_entry(out: &mut Vec<u8>, tag: u16, ty: u16, count: u32, value: u32) {
    out.extend_from_slice(&tag.to_be_bytes());
    out.extend_from_slice(&ty.to_be_bytes());
    out.extend_from_slice(&count.to_be_bytes());
    out.extend_from_slice(&value.to_be_bytes());
}

/// A real JPEG with EXIF, XMP, an IPTC block, a comment, and a trailer.
fn jpeg_with_metadata() -> Vec<u8> {
    let base = encode(image::ImageFormat::Jpeg);
    let mut out = base[..2].to_vec(); // SOI

    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&exif_block());
    push_segment(&mut out, 0xE1, &app1);

    let mut xmp = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    xmp.extend_from_slice(
        format!("<x:xmpmeta><dc:creator>{CANARY}</dc:creator></x:xmpmeta>").as_bytes(),
    );
    push_segment(&mut out, 0xE1, &xmp);

    let mut iptc = b"Photoshop 3.0\0".to_vec();
    iptc.extend_from_slice(b"8BIM\x04\x04");
    iptc.extend_from_slice(CANARY.as_bytes());
    push_segment(&mut out, 0xED, &iptc);

    push_segment(&mut out, 0xFE, format!("comment: {CANARY}").as_bytes());
    push_segment(&mut out, 0xE7, format!("VendorPrivate{CANARY}").as_bytes());

    out.extend_from_slice(&base[2..]);
    out.extend_from_slice(format!("trailer {CANARY}").as_bytes());
    out
}

fn push_segment(out: &mut Vec<u8>, marker: u8, payload: &[u8]) {
    out.push(0xFF);
    out.push(marker);
    out.extend_from_slice(&((payload.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(payload);
}

/// A real PNG with text, EXIF and time chunks inserted after IHDR.
fn png_with_metadata() -> Vec<u8> {
    let base = encode(image::ImageFormat::Png);
    // Signature plus the IHDR chunk: 8 + 4 length + 4 type + 13 body + 4 CRC.
    let after_ihdr = 8 + 4 + 4 + 13 + 4;
    let mut out = base[..after_ihdr].to_vec();

    out.extend_from_slice(&png_chunk(b"tEXt", format!("Author\0{CANARY}").as_bytes()));
    out.extend_from_slice(&png_chunk(
        b"iTXt",
        format!("XML:com.adobe.xmp\0\0\0\0\0<x:xmpmeta>{CANARY}</x:xmpmeta>").as_bytes(),
    ));
    out.extend_from_slice(&png_chunk(b"eXIf", &exif_block()));
    out.extend_from_slice(&png_chunk(b"tIME", &[0x07, 0xEA, 3, 4, 10, 0, 0]));
    out.extend_from_slice(&png_chunk(b"prVt", CANARY.as_bytes()));

    out.extend_from_slice(&base[after_ihdr..]);
    out.extend_from_slice(format!("after IEND {CANARY}").as_bytes());
    out
}

fn png_chunk(ty: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut v = (body.len() as u32).to_be_bytes().to_vec();
    v.extend_from_slice(ty);
    v.extend_from_slice(body);
    v.extend_from_slice(&png_crc(ty, body).to_be_bytes());
    v
}

/// An independent CRC-32, so the test does not validate the library against
/// its own implementation.
fn png_crc(ty: &[u8; 4], body: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in ty.iter().chain(body) {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle.as_bytes())
}

fn decode(bytes: &[u8]) -> image::RgbImage {
    image::load_from_memory(bytes).expect("the sanitized file must still decode").to_rgb8()
}

// ---------------------------------------------------------------------------
// The pixels must survive
// ---------------------------------------------------------------------------

#[test]
fn a_sanitized_jpeg_decodes_to_exactly_the_same_pixels() {
    let dirty = jpeg_with_metadata();
    let clean = sanitize(&dirty, &Policy::default()).unwrap();

    assert_eq!(
        decode(&dirty),
        decode(&clean.data),
        "stripping metadata must not touch a single pixel; if this fails the rebuild has \
         become a re-encode"
    );
    assert!(clean.data.len() < dirty.len(), "the file should get smaller");
}

#[test]
fn a_sanitized_png_decodes_to_exactly_the_same_pixels() {
    let dirty = png_with_metadata();
    let clean = sanitize(&dirty, &Policy::default()).unwrap();
    assert_eq!(decode(&dirty), decode(&clean.data));
}

#[test]
fn a_png_with_no_metadata_comes_back_byte_identical() {
    let pristine = encode(image::ImageFormat::Png);
    let clean = sanitize(&pristine, &Policy::default()).unwrap();
    assert_eq!(clean.data, pristine, "there was nothing to change");
    assert!(clean.report.is_clean());
}

// ---------------------------------------------------------------------------
// The metadata must not survive
// ---------------------------------------------------------------------------

#[test]
fn nothing_carrying_the_canary_survives_a_jpeg() {
    let clean = sanitize(&jpeg_with_metadata(), &Policy::default()).unwrap();

    assert!(!contains(&clean.data, CANARY), "the canary appeared in the output");
    assert!(!contains(&clean.data, "Exif"));
    assert!(!contains(&clean.data, "ns.adobe.com"));
    assert!(!contains(&clean.data, "Photoshop"));

    let report = &clean.report;
    assert_eq!(report.format, Format::Jpeg);
    assert_eq!(report.assurance, Assurance::Complete);
    assert!(report.found_location, "the fixture carries a latitude");

    let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
    for expected in [Kind::Exif, Kind::Xmp, Kind::Iptc, Kind::Comment, Kind::Trailer] {
        assert!(kinds.contains(&expected), "{expected:?} was not reported");
    }
    assert!(kinds.contains(&Kind::MakerNote), "the maker note holds the serial number");
    assert!(kinds.contains(&Kind::UnknownStructure), "APP7 is not a segment we parse");
}

#[test]
fn nothing_carrying_the_canary_survives_a_png() {
    let clean = sanitize(&png_with_metadata(), &Policy::default()).unwrap();

    assert!(!contains(&clean.data, CANARY));
    assert!(!contains(&clean.data, "eXIf"));
    assert!(!contains(&clean.data, "prVt"));
    assert!(clean.report.found_location);
    assert_eq!(clean.report.assurance, Assurance::Complete);
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

#[test]
fn keeping_the_rotation_keeps_only_the_rotation() {
    let dirty = jpeg_with_metadata();
    let clean = sanitize(&dirty, &Policy::preserve_appearance()).unwrap();

    // The picture is unchanged and still decodes.
    assert_eq!(decode(&dirty), decode(&clean.data));
    // The GPS coordinates and the serial number do not ride along in the
    // rebuilt EXIF block.
    assert!(!contains(&clean.data, CANARY));
    assert!(clean.report.found_location, "and we still say the coordinates were there");

    // What is left is the 32-byte synthesized block and nothing more.
    let at = clean.data.windows(6).position(|w| w == b"Exif\0\0").expect("EXIF kept");
    let len = u16::from_be_bytes([clean.data[at - 2], clean.data[at - 1]]);
    assert_eq!(len, 34, "the block must be the minimal one, not a filtered original");
}

#[test]
fn the_strict_policy_warns_when_it_drops_a_real_rotation() {
    let clean = sanitize(&jpeg_with_metadata(), &Policy::strict()).unwrap();
    assert!(
        clean.report.warnings.iter().any(|w| w.contains("sideways")),
        "a photo that will now display rotated should say so"
    );
}

// ---------------------------------------------------------------------------
// Honesty
// ---------------------------------------------------------------------------

#[test]
fn an_unsupported_format_is_returned_untouched_and_never_called_clean() {
    let mp3ish = b"ID3\x04\x00\x00\x00\x00\x00\x00TPE1 artist name".to_vec();
    let out = sanitize(&mp3ish, &Policy::default()).unwrap();

    assert_eq!(out.data, mp3ish);
    assert_eq!(out.report.assurance, Assurance::None);
    assert!(!out.report.summary().contains("no metadata"));
    assert!(out.report.summary().contains("not sanitized"));
}

#[test]
fn inspect_reports_the_same_findings_as_a_real_run() {
    let dirty = jpeg_with_metadata();
    let inspected = metascrub::inspect(&dirty, &Policy::default()).unwrap();
    let sanitized = sanitize(&dirty, &Policy::default()).unwrap().report;

    assert_eq!(inspected.removed, sanitized.removed);
    assert_eq!(inspected.found_location, sanitized.found_location);
}

// ---------------------------------------------------------------------------
// Robustness against files that are not what they claim to be
// ---------------------------------------------------------------------------

#[test]
fn truncated_fixtures_never_panic() {
    for fixture in [jpeg_with_metadata(), png_with_metadata()] {
        for n in 0..fixture.len() {
            let _ = sanitize(&fixture[..n], &Policy::default());
        }
    }
}

#[test]
fn corrupted_fixtures_never_panic() {
    // A deterministic walk over single-byte corruptions. Any panic here is a
    // denial of service on a crate whose input arrives from strangers.
    for fixture in [jpeg_with_metadata(), png_with_metadata()] {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        for _ in 0..2000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mut bad = fixture.clone();
            let at = (state as usize) % bad.len();
            bad[at] ^= ((state >> 32) as u8) | 1;
            let _ = sanitize(&bad, &Policy::default());
        }
    }
}

#[test]
fn a_file_whose_header_lies_about_its_format_is_an_error_not_a_pass() {
    // JPEG magic on PNG content: the parser must refuse rather than return the
    // input and claim it was handled.
    let mut liar = vec![0xFF, 0xD8, 0xFF];
    liar.extend_from_slice(&encode(image::ImageFormat::Png));
    assert!(sanitize(&liar, &Policy::default()).is_err());
}

#[test]
fn random_bytes_of_every_length_never_panic() {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    for len in 0..600 {
        let junk: Vec<u8> = (0..len)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                state as u8
            })
            .collect();
        let _ = sanitize(&junk, &Policy::default());
    }
}
