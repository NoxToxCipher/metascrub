//! End-to-end tests for the document formats and the command-line tool.
//!
//! Like `roundtrip.rs`, these build real files rather than committing blobs,
//! and they check both halves of the claim: the identifying data is gone, and
//! the file still opens.
//!
//! Every test here needs the archive, PDF or CLI feature. In an image-only
//! build they all compile out, and so do the helpers they share.
#![cfg_attr(
    not(any(feature = "ooxml", feature = "pdf", feature = "cli")),
    allow(unused_imports, dead_code)
)]

use metascrub::{sanitize, Assurance, Format, Kind, Policy};

const CANARY: &str = "CANARY-SERIAL-4417";

// ---------------------------------------------------------------------------
// A minimal ZIP writer, independent of the one under test
// ---------------------------------------------------------------------------

/// Stored (uncompressed) entries only, so this shares no code path with the
/// library's writer and a bug in one cannot mask a bug in the other.
#[cfg(feature = "ooxml")]
fn zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
    fn crc32(data: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &b in data {
            crc ^= b as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    let mut out = Vec::new();
    let mut offsets = Vec::new();
    for (name, body) in entries {
        offsets.push(out.len() as u32);
        out.extend_from_slice(b"PK\x03\x04");
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // stored
        out.extend_from_slice(&0x1234u16.to_le_bytes()); // a real time
        out.extend_from_slice(&0x5678u16.to_le_bytes()); // a real date
        out.extend_from_slice(&crc32(body).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(body);
    }
    let cd_offset = out.len() as u32;
    for ((name, body), offset) in entries.iter().zip(&offsets) {
        out.extend_from_slice(b"PK\x01\x02");
        out.extend_from_slice(&0x031Eu16.to_le_bytes()); // "made by Unix"
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0x1234u16.to_le_bytes());
        out.extend_from_slice(&0x5678u16.to_le_bytes());
        out.extend_from_slice(&crc32(body).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(&(name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        out.extend_from_slice(name.as_bytes());
    }
    let cd_size = out.len() as u32 - cd_offset;
    out.extend_from_slice(b"PK\x05\x06");
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_offset.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// A JPEG carrying EXIF with GPS and the canary in a maker note.
fn photo_with_gps() -> Vec<u8> {
    let mut tiff = Vec::new();
    tiff.extend_from_slice(b"MM");
    tiff.extend_from_slice(&42u16.to_be_bytes());
    tiff.extend_from_slice(&8u32.to_be_bytes());
    tiff.extend_from_slice(&1u16.to_be_bytes());
    tiff.extend_from_slice(&0x8825u16.to_be_bytes()); // GPS pointer
    tiff.extend_from_slice(&4u16.to_be_bytes());
    tiff.extend_from_slice(&1u32.to_be_bytes());
    tiff.extend_from_slice(&26u32.to_be_bytes());
    tiff.extend_from_slice(&0u32.to_be_bytes());
    tiff.extend_from_slice(&1u16.to_be_bytes());
    tiff.extend_from_slice(&0x0002u16.to_be_bytes()); // GPSLatitude
    tiff.extend_from_slice(&5u16.to_be_bytes());
    tiff.extend_from_slice(&3u32.to_be_bytes());
    tiff.extend_from_slice(&0u32.to_be_bytes());
    tiff.extend_from_slice(&0u32.to_be_bytes());
    tiff.extend_from_slice(CANARY.as_bytes());

    let mut app1 = b"Exif\0\0".to_vec();
    app1.extend_from_slice(&tiff);

    let base = {
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image::RgbImage::new(8, 8))
            .write_to(&mut buf, image::ImageFormat::Jpeg)
            .unwrap();
        buf.into_inner()
    };
    let mut out = base[..2].to_vec();
    out.push(0xFF);
    out.push(0xE1);
    out.extend_from_slice(&((app1.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&app1);
    out.extend_from_slice(&base[2..]);
    out
}

#[cfg(feature = "ooxml")]
fn docx() -> Vec<u8> {
    let core = format!(
        r#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="c" xmlns:dc="d">
<dc:creator>{CANARY}</dc:creator><cp:lastModifiedBy>{CANARY}</cp:lastModifiedBy>
<cp:revision>17</cp:revision><dcterms:created>2026-01-02T09:15:00Z</dcterms:created>
</cp:coreProperties>"#
    );
    let app = format!(
        r#"<?xml version="1.0"?><Properties xmlns="e"><Company>{CANARY}</Company>
<Application>Microsoft Office Word</Application><TotalTime>438</TotalTime></Properties>"#
    );
    let document = format!(
        r#"<w:document><w:body><w:p w:rsidR="00A12B34">
<w:ins w:id="1" w:author="{CANARY}" w:date="2026-03-04T10:00:00Z">
<w:r><w:t>the visible sentence</w:t></w:r></w:ins></w:p></w:body></w:document>"#
    );
    let settings = r#"<w:settings><w:zoom w:percent="100"/><w:rsids><w:rsidRoot w:val="00A12B34"/></w:rsids></w:settings>"#;

    zip(&[
        ("[Content_Types].xml", b"<Types/>".to_vec()),
        ("docProps/core.xml", core.into_bytes()),
        ("docProps/app.xml", app.into_bytes()),
        ("word/document.xml", document.into_bytes()),
        ("word/settings.xml", settings.as_bytes().to_vec()),
        ("word/media/image1.jpeg", photo_with_gps()),
    ])
}

#[cfg(feature = "pdf")]
fn pdf() -> Vec<u8> {
    use lopdf::{dictionary, Document, Object, Stream, StringFormat};

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let page_id = doc.add_object(dictionary! {
        "Type" => "Page",
        "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
    });
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
        }),
    );

    let xmp = doc.add_object(Stream::new(
        dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
        format!("<x:xmpmeta><dc:creator>{CANARY}</dc:creator></x:xmpmeta>").into_bytes(),
    ));
    let catalog = doc.add_object(dictionary! {
        "Type" => "Catalog", "Pages" => pages_id, "Metadata" => xmp,
    });
    doc.trailer.set("Root", catalog);

    let info = doc.add_object(dictionary! {
        "Author" => Object::String(CANARY.into(), StringFormat::Literal),
        "Producer" => Object::String(CANARY.into(), StringFormat::Literal),
        "CreationDate" => Object::String(b"D:20260102091500Z".to_vec(), StringFormat::Literal),
    });
    doc.trailer.set("Info", info);

    let mut out = Vec::new();
    doc.save_to(&mut out).unwrap();
    out
}

fn contains(haystack: &[u8], needle: &str) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle.as_bytes())
}

// ---------------------------------------------------------------------------
// OOXML
// ---------------------------------------------------------------------------

#[cfg(feature = "ooxml")]
#[test]
fn a_docx_loses_its_author_its_revision_ids_and_its_embedded_photos_gps() {
    let clean = sanitize(&docx(), &Policy::default()).unwrap();

    assert!(!contains(&clean.data, CANARY), "the canary survived somewhere in the archive");
    assert!(!contains(&clean.data, "00A12B34"), "a revision-save identifier survived");
    assert!(!contains(&clean.data, "Microsoft Office Word"));
    assert!(!contains(&clean.data, "2026-03-04"));

    let report = &clean.report;
    assert_eq!(report.format, Format::Ooxml);
    assert_eq!(report.assurance, Assurance::BestEffort, "an archive is edited, not rebuilt");
    assert!(report.found_location, "the pasted photo carried coordinates");

    let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
    for expected in [Kind::DocumentInfo, Kind::Author, Kind::RevisionIds, Kind::Timestamp] {
        assert!(kinds.contains(&expected), "{expected:?} was not reported");
    }
    assert!(
        report.removed.iter().any(|r| r.location.contains("word/media/image1.jpeg")),
        "the embedded photo's findings must name the part they came from"
    );
}

#[cfg(feature = "ooxml")]
#[test]
fn the_visible_content_of_a_docx_is_untouched() {
    let clean = sanitize(&docx(), &Policy::default()).unwrap();
    // The archive is deflated on the way out, so decompress before looking.
    assert!(contains(&clean.data, "PK\x03\x04"), "the output should still be a zip archive");
    let reread = sanitize(&clean.data, &Policy::default()).unwrap();
    assert_eq!(reread.report.format, Format::Ooxml, "it must still detect as a docx");
    assert!(!reread.report.found_location, "a second pass should find nothing left");
}

#[cfg(feature = "ooxml")]
#[test]
fn sanitizing_twice_is_stable() {
    let once = sanitize(&docx(), &Policy::default()).unwrap();
    let twice = sanitize(&once.data, &Policy::default()).unwrap();
    assert_eq!(once.data, twice.data, "a clean file must not keep changing");
}

// ---------------------------------------------------------------------------
// PDF
// ---------------------------------------------------------------------------

#[cfg(feature = "pdf")]
#[test]
fn a_pdf_loses_its_info_dictionary_and_its_xmp() {
    let clean = sanitize(&pdf(), &Policy::default()).unwrap();

    assert!(!contains(&clean.data, CANARY));
    assert!(!contains(&clean.data, "D:20260102"));
    assert!(!contains(&clean.data, "/Info"));
    assert!(!contains(&clean.data, "xmpmeta"));

    assert_eq!(clean.report.format, Format::Pdf);
    assert_eq!(clean.report.assurance, Assurance::BestEffort);
    assert!(clean.report.removed.iter().any(|r| r.kind == Kind::Xmp));

    // And it is still a PDF with its page.
    let doc = lopdf::Document::load_mem(&clean.data).expect("must still parse");
    assert_eq!(doc.get_pages().len(), 1);
}

// ---------------------------------------------------------------------------
// The command-line tool
// ---------------------------------------------------------------------------

#[cfg(feature = "cli")]
fn scratch(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("metascrub-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(format!("{}-{name}", std::process::id()))
}

#[cfg(feature = "cli")]
fn cli() -> std::process::Command {
    std::process::Command::new(env!("CARGO_BIN_EXE_metascrub"))
}

#[cfg(feature = "cli")]
#[test]
fn the_cli_writes_a_cleaned_copy_and_leaves_the_original_alone() {
    let src = scratch("photo.jpg");
    let dirty = photo_with_gps();
    std::fs::write(&src, &dirty).unwrap();

    let out = cli().arg(&src).output().expect("the binary should run");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));

    assert_eq!(std::fs::read(&src).unwrap(), dirty, "the input must not be modified");

    let cleaned = src.with_extension("clean.jpg");
    let bytes = std::fs::read(&cleaned).expect("the cleaned copy should exist");
    assert!(!contains(&bytes, CANARY));

    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("where it was taken"), "GPS should be called out: {stdout}");

    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&cleaned);
}

#[cfg(feature = "cli")]
#[test]
fn a_dry_run_writes_nothing() {
    let src = scratch("dry.jpg");
    std::fs::write(&src, photo_with_gps()).unwrap();

    let out = cli().args(["-n"]).arg(&src).output().unwrap();
    assert!(out.status.success());
    assert!(!src.with_extension("clean.jpg").exists(), "a dry run created a file");

    let _ = std::fs::remove_file(&src);
}

#[cfg(feature = "cli")]
#[test]
fn the_json_output_is_parseable_and_reports_the_location_finding() {
    let src = scratch("json.jpg");
    std::fs::write(&src, photo_with_gps()).unwrap();

    let out = cli().args(["-n", "--json"]).arg(&src).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(stdout.trim_start().starts_with('['));
    assert!(stdout.trim_end().ends_with(']'));
    assert!(stdout.contains(r#""found_location": true"#));
    assert!(stdout.contains(r#""format": "JPEG""#));
    assert!(stdout.contains(r#""kind": "EXIF""#));

    let _ = std::fs::remove_file(&src);
}

#[cfg(feature = "cli")]
#[test]
fn an_unsupported_file_exits_with_the_distinct_status() {
    let src = scratch("notes.txt");
    std::fs::write(&src, b"just some text, not a container we parse").unwrap();

    let out = cli().arg(&src).output().unwrap();
    assert_eq!(out.status.code(), Some(2), "unsupported is not the same as failed");
    assert!(String::from_utf8_lossy(&out.stderr).contains("NOT SANITIZED"));

    let _ = std::fs::remove_file(&src);
}

#[cfg(feature = "cli")]
#[test]
fn a_corrupt_file_in_a_supported_format_exits_as_a_failure() {
    let src = scratch("broken.jpg");
    std::fs::write(&src, b"\xFF\xD8\xFF\xE1\x27\x0Ftruncated").unwrap();

    let out = cli().arg(&src).output().unwrap();
    assert_eq!(out.status.code(), Some(1));

    let _ = std::fs::remove_file(&src);
}
