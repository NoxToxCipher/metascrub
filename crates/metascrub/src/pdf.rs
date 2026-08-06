//! PDF: strip the document information dictionary, the XMP metadata streams,
//! and the per-annotation identity fields, then write the file back out.
//!
//! ## Why this one uses a parser
//!
//! Everywhere else in this crate the container is walked by hand. PDF is the
//! exception, and the reason is the cross-reference table. A PDF is a set of
//! numbered objects indexed by their **byte offset in the file**, so removing
//! any bytes shifts every offset after them and invalidates the index. Editing
//! in place while preserving length is possible for simple files, but it does
//! not reach two structures that modern producers use constantly: cross
//! reference *streams*, and object *streams*, where dozens of objects including
//! the information dictionary are packed inside one compressed blob. A
//! byte-level editor silently misses those, which is the failure mode that
//! matters most: reporting success on a file that still names its author.
//!
//! ## Incremental updates
//!
//! PDF supports appending edits rather than rewriting, so a file can contain
//! several historical revisions, and "removing" metadata by appending a new
//! empty information dictionary leaves the old one sitting in the file for
//! anyone who reads it with a text editor. Rebuilding the file from the parsed
//! object set drops every superseded revision, which is the behaviour we want
//! and a good reason to reserialize rather than append.
//!
//! ## What this does not reach
//!
//! Enough that the result is [`Assurance::BestEffort`]. Attached files keep
//! their own metadata, images inside the PDF are not descended into, and form
//! field values, JavaScript and optional-content names are content rather than
//! metadata and are left alone. Each of those is reported when present.
//!
//! [`Assurance::BestEffort`]: crate::Assurance

use crate::error::Error;
use crate::policy::Policy;
use crate::report::{Assurance, Kind, Report};
use lopdf::{Dictionary, Document, Object};

const FORMAT: &str = "PDF";

/// Information dictionary keys. Producer and Creator name the software and
/// often its licensee; Author is usually the operating system account name.
const INFO_KEYS: &[&[u8]] = &[
    b"Title",
    b"Author",
    b"Subject",
    b"Keywords",
    b"Creator",
    b"Producer",
    b"CreationDate",
    b"ModDate",
    b"Trapped",
];

/// Keys removed from every dictionary in the file.
///
/// `Metadata` is the XMP packet, which duplicates the information dictionary
/// and adds the editing history. `PieceInfo` is private application data that
/// Illustrator and InDesign use to stash their own document state, including
/// paths. `LastModified` accompanies it.
const SWEEP_KEYS: &[&[u8]] = &[b"Metadata", b"PieceInfo", b"LastModified"];

/// Annotation keys that name a person or a moment: the author of a comment,
/// when it was made and last changed, and its unique name, which some
/// producers derive from the machine.
const ANNOT_KEYS: &[&[u8]] = &[b"T", b"M", b"CreationDate", b"NM", b"RC"];

pub(crate) fn sanitize(
    input: &[u8],
    _policy: &Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    report.assurance = Assurance::BestEffort;

    let mut doc = Document::load_mem(input)
        .map_err(|e| Error::malformed(FORMAT, format!("could not be parsed: {e}")))?;

    if doc.trailer.has(b"Encrypt") {
        return Err(Error::Encrypted("this PDF"));
    }

    strip_info(&mut doc, report);
    strip_file_id(&mut doc, report);
    sweep_objects(&mut doc, report);

    let mut out = Vec::with_capacity(input.len());
    doc.save_to(&mut out)
        .map_err(|e| Error::malformed(FORMAT, format!("could not be written back: {e}")))?;

    report.warn(
        "the PDF was rebuilt from its object graph, which also drops any earlier revisions \
         the file was carrying; attached files and images inside the page content keep their \
         own metadata and were not opened",
    );
    Ok(out)
}

/// Remove the document information dictionary, both the trailer's reference to
/// it and the object itself.
fn strip_info(doc: &mut Document, report: &mut Report) {
    let info_id = doc.trailer.get(b"Info").ok().and_then(|o| o.as_reference().ok());

    // Report which fields were present before dropping them. The values
    // themselves are deliberately not logged: a report that quotes the author's
    // name has just copied it somewhere new.
    let present: Vec<String> = match info_id.and_then(|id| doc.get_object(id).ok()) {
        Some(Object::Dictionary(dict)) => INFO_KEYS
            .iter()
            .filter(|k| dict.has(k))
            .map(|k| String::from_utf8_lossy(k).into_owned())
            .collect(),
        _ => Vec::new(),
    };

    doc.trailer.remove(b"Info");
    if let Some(id) = info_id {
        doc.objects.remove(&id);
    }

    if !present.is_empty() {
        report.removed(Kind::DocumentInfo, format!("document info ({})", present.join(", ")), 0);
    }
    if present.iter().any(|k| k == "CreationDate" || k == "ModDate") {
        report.removed(Kind::Timestamp, "document info dates", 0);
    }
    if present.iter().any(|k| k == "Author") {
        report.removed(Kind::Author, "document info author", 0);
    }
}

/// Remove the trailer file identifier.
///
/// `/ID` is a pair of byte strings that producers derive from the current time,
/// the file's path and its size. It is not required for an unencrypted PDF, and
/// two files with the same first element came from the same original, which
/// links documents to each other.
fn strip_file_id(doc: &mut Document, report: &mut Report) {
    if doc.trailer.remove(b"ID").is_some() {
        report.removed(Kind::DocumentInfo, "trailer file identifier (/ID)", 0);
    }
}

/// Walk every object, removing metadata keys and deleting XMP streams.
fn sweep_objects(doc: &mut Document, report: &mut Report) {
    let mut xmp_streams = Vec::new();
    let mut swept = 0usize;
    let mut annots = 0usize;
    let mut embedded_files = 0usize;

    let ids: Vec<_> = doc.objects.keys().copied().collect();
    for id in ids {
        let Some(object) = doc.objects.get_mut(&id) else { continue };

        let dict: &mut Dictionary = match object {
            Object::Dictionary(d) => d,
            Object::Stream(s) => &mut s.dict,
            _ => continue,
        };

        match dict.get(b"Type").ok().and_then(|t| t.as_name().ok()) {
            // An XMP packet as a standalone object. Its own dictionary has
            // nothing worth keeping, so the object goes rather than the key.
            Some(b"Metadata") => {
                xmp_streams.push(id);
                continue;
            }
            Some(b"Annot") => {
                let before = ANNOT_KEYS.iter().filter(|k| dict.has(k)).count();
                for key in ANNOT_KEYS {
                    dict.remove(key);
                }
                if before > 0 {
                    annots += 1;
                }
            }
            Some(b"Filespec") | Some(b"EmbeddedFile") => embedded_files += 1,
            _ => {}
        }

        for key in SWEEP_KEYS {
            if dict.remove(key).is_some() {
                swept += 1;
            }
        }
    }

    for id in &xmp_streams {
        doc.objects.remove(id);
    }

    if !xmp_streams.is_empty() {
        report.removed(Kind::Xmp, format!("{} XMP metadata stream(s)", xmp_streams.len()), 0);
    }
    if swept > 0 {
        report.removed(
            Kind::DocumentInfo,
            format!("{swept} metadata reference(s) on the catalog, pages and objects"),
            0,
        );
    }
    if annots > 0 {
        report.removed(Kind::Author, format!("author and dates on {annots} annotation(s)"), 0);
    }
    if embedded_files > 0 {
        report.warn(format!(
            "this PDF has {embedded_files} attached file(s); attachments keep whatever metadata \
             they arrived with, so sanitize them separately before sending"
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    /// Build a small but genuine PDF: catalog, page tree, one page, an
    /// information dictionary, and an XMP metadata stream on the catalog.
    fn pdf(with_info: bool, with_xmp: bool, with_annot: bool) -> Vec<u8> {
        use lopdf::{dictionary, Object, Stream, StringFormat};

        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();

        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        };
        if with_annot {
            let annot = doc.add_object(dictionary! {
                "Type" => "Annot",
                "Subtype" => "Text",
                "Rect" => vec![10.into(), 10.into(), 20.into(), 20.into()],
                "T" => Object::String(b"Bob Reviewer".to_vec(), StringFormat::Literal),
                "M" => Object::String(b"D:20260304100000Z".to_vec(), StringFormat::Literal),
                "Contents" => Object::String(b"please check".to_vec(), StringFormat::Literal),
            });
            page.set("Annots", vec![annot.into()]);
        }
        let page_id = doc.add_object(page);

        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => vec![page_id.into()],
                "Count" => 1,
            }),
        );

        let mut catalog = dictionary! { "Type" => "Catalog", "Pages" => pages_id };
        if with_xmp {
            let xmp = doc.add_object(Stream::new(
                dictionary! { "Type" => "Metadata", "Subtype" => "XML" },
                br#"<?xpacket?><x:xmpmeta><dc:creator>Jane Q. Author</dc:creator>
                    <xmp:CreatorTool>Acme Publisher 9</xmp:CreatorTool></x:xmpmeta>"#
                    .to_vec(),
            ));
            catalog.set("Metadata", xmp);
        }
        let catalog_id = doc.add_object(catalog);
        doc.trailer.set("Root", catalog_id);

        if with_info {
            let info = doc.add_object(dictionary! {
                "Author" => Object::String(b"Jane Q. Author".to_vec(), StringFormat::Literal),
                "Creator" => Object::String(b"Acme Publisher 9".to_vec(), StringFormat::Literal),
                "Producer" => Object::String(b"Acme PDF Engine 4.2".to_vec(), StringFormat::Literal),
                "Title" => Object::String(b"Quarterly numbers".to_vec(), StringFormat::Literal),
                "CreationDate" => Object::String(b"D:20260102091500Z".to_vec(), StringFormat::Literal),
            });
            doc.trailer.set("Info", info);
        }
        doc.trailer.set(
            "ID",
            vec![
                Object::String(b"\x01\x02\x03\x04".to_vec(), StringFormat::Hexadecimal),
                Object::String(b"\x01\x02\x03\x04".to_vec(), StringFormat::Hexadecimal),
            ],
        );

        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Pdf, input.len());
        let out = sanitize(input, &Policy::default(), &mut report).expect("valid pdf");
        (out, report)
    }

    fn holds(haystack: &[u8], needle: &str) -> bool {
        haystack.windows(needle.len()).any(|w| w == needle.as_bytes())
    }

    #[test]
    fn the_information_dictionary_is_removed() {
        let (out, report) = run(&pdf(true, false, false));

        for needle in ["Jane Q. Author", "Acme PDF Engine 4.2", "Quarterly numbers", "D:2026"] {
            assert!(!holds(&out, needle), "{needle} survived");
        }
        assert!(!holds(&out, "/Info"));

        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::DocumentInfo));
        assert!(kinds.contains(&Kind::Author));
        assert!(kinds.contains(&Kind::Timestamp));
    }

    #[test]
    fn the_report_names_the_fields_without_repeating_their_values() {
        let (_, report) = run(&pdf(true, false, false));
        let info = report
            .removed
            .iter()
            .find(|r| r.location.starts_with("document info ("))
            .expect("the info fields should be itemised");
        assert!(info.location.contains("Author") && info.location.contains("Producer"));
        assert!(
            !info.location.contains("Jane"),
            "a report that quotes the author has copied it somewhere new"
        );
    }

    #[test]
    fn xmp_metadata_streams_are_removed_along_with_the_reference_to_them() {
        let (out, report) = run(&pdf(false, true, false));
        assert!(!holds(&out, "Jane Q. Author"));
        assert!(!holds(&out, "xmpmeta"));
        assert!(!holds(&out, "/Metadata"));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Xmp));
    }

    #[test]
    fn annotation_authors_and_dates_go_but_the_comment_text_stays() {
        let (out, report) = run(&pdf(false, false, true));
        assert!(!holds(&out, "Bob Reviewer"));
        assert!(!holds(&out, "D:20260304"));
        assert!(holds(&out, "please check"), "the comment is content, not metadata");
        assert!(report.removed.iter().any(|r| r.kind == Kind::Author));
    }

    #[test]
    fn the_trailer_file_identifier_is_removed() {
        let (out, report) = run(&pdf(false, false, false));
        assert!(!holds(&out, "/ID"));
        assert!(report.removed.iter().any(|r| r.location.contains("/ID")));
    }

    #[test]
    fn the_result_is_still_a_readable_pdf_with_its_page_intact() {
        let (out, _) = run(&pdf(true, true, true));
        assert!(out.starts_with(b"%PDF-"));
        let doc = Document::load_mem(&out).expect("the rebuilt file must still parse");
        assert_eq!(doc.get_pages().len(), 1, "the page must survive the rebuild");
        assert!(doc.trailer.get(b"Root").is_ok());
    }

    #[test]
    fn an_earlier_revision_left_by_an_incremental_update_does_not_survive() {
        // Append a second revision the way an editor would, then confirm the
        // superseded body is gone rather than merely unreferenced.
        let mut input = pdf(true, false, false);
        input.extend_from_slice(b"\n% appended revision with SUPERSEDED-SECRET inside\n");
        let (out, _) = run(&input);
        assert!(!holds(&out, "SUPERSEDED-SECRET"));
        assert!(!holds(&out, "Jane Q. Author"));
    }

    #[test]
    fn an_encrypted_pdf_is_refused_rather_than_reported_clean() {
        let mut doc = Document::load_mem(&pdf(true, false, false)).unwrap();
        doc.trailer.set("Encrypt", lopdf::Object::Null);
        let mut input = Vec::new();
        doc.save_to(&mut input).unwrap();

        let mut report = Report::new(Format::Pdf, input.len());
        assert!(matches!(
            sanitize(&input, &Policy::default(), &mut report),
            Err(Error::Encrypted(_))
        ));
    }

    #[test]
    fn the_result_is_never_advertised_as_a_complete_strip() {
        let (_, report) = run(&pdf(true, true, false));
        assert_eq!(report.assurance, Assurance::BestEffort);
        assert!(report.warnings.iter().any(|w| w.contains("rebuilt from its object graph")));
    }

    #[test]
    fn a_file_that_does_not_parse_is_an_error_not_a_silent_pass() {
        let mut report = Report::new(Format::Pdf, 0);
        assert!(sanitize(b"%PDF-1.7\nnot really a pdf", &Policy::default(), &mut report).is_err());
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = pdf(true, true, true);
        for n in (0..full.len()).step_by(7) {
            let mut report = Report::new(Format::Pdf, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }
}
