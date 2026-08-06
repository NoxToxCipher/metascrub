//! Office Open XML (.docx, .xlsx, .pptx) and OpenDocument (.odt, .ods, .odp).
//!
//! Both are ZIP archives of XML parts, and both keep the bulk of their
//! metadata in a small number of dedicated property files. Those are replaced
//! wholesale with a canonical empty version rather than edited, so nothing that
//! was in them can survive by being in a field we did not think to clear.
//!
//! Beyond the property files there are four leaks that a "delete docProps"
//! sanitizer misses, and all four are handled here:
//!
//! - **Tracked changes and comments** name their author in the document body,
//!   not in the properties. A document with revisions accepted still carries
//!   the names in `word/comments.xml` and in every `w:ins` element.
//! - **Revision-save identifiers** (`w:rsid*`) are random tokens Word stamps on
//!   every paragraph and run, and shares between all documents edited in the
//!   same session. Two files with overlapping rsids came off the same machine.
//! - **Embedded images** keep their own EXIF. A photo pasted into a report
//!   still has the GPS coordinates it was taken with, sitting in `word/media/`.
//! - **The archive itself** stamps a modification timestamp on every entry, and
//!   the extra-field area often carries the Unix user id that wrote the file.
//!
//! The assurance level is [`Assurance::BestEffort`] rather than complete. Parts
//! are edited, not rebuilt, and both formats let an application store arbitrary
//! private parts inside the archive. Dropping unknown parts would be the
//! allowlist approach, but in these formats an unknown part is as likely to be
//! a chart, a font or an embedded object the document needs, so removing it
//! would routinely break files.
//!
//! [`Assurance::BestEffort`]: crate::Assurance

use crate::policy::Policy;
use crate::report::{Assurance, Kind, Report};
use crate::xmlscrub::{self, DOCUMENT_RULES};
use crate::zip::Archive;

/// Property parts replaced with a canonical empty document.
///
/// `core.xml` holds the creator, last-modified-by, revision number and the
/// create/modify timestamps. `app.xml` holds the application name and version,
/// the company, the manager, the template it came from, and the total editing
/// time. `custom.xml` holds whatever the organisation configured, which in
/// practice is where document management systems put user names and matter
/// numbers.
const CORE: &str = "docProps/core.xml";
const APP: &str = "docProps/app.xml";
const CUSTOM: &str = "docProps/custom.xml";
const THUMBNAIL: &str = "docProps/thumbnail.jpeg";

const EMPTY_CORE: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\r\n",
    r#"<cp:coreProperties"#,
    r#" xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties""#,
    r#" xmlns:dc="http://purl.org/dc/elements/1.1/""#,
    r#" xmlns:dcterms="http://purl.org/dc/terms/""#,
    r#" xmlns:dcmitype="http://purl.org/dc/dcmitype/""#,
    r#" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"/>"#,
);

const EMPTY_APP: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\r\n",
    r#"<Properties"#,
    r#" xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties""#,
    r#" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#,
);

const EMPTY_CUSTOM: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
    "\r\n",
    r#"<Properties"#,
    r#" xmlns="http://schemas.openxmlformats.org/officeDocument/2006/custom-properties""#,
    r#" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"/>"#,
);

const OD_META: &str = "meta.xml";
const EMPTY_OD_META: &str = concat!(
    r#"<?xml version="1.0" encoding="UTF-8"?>"#,
    r#"<office:document-meta"#,
    r#" xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0""#,
    r#" office:version="1.3"><office:meta/></office:document-meta>"#,
);

/// Parts whose presence is worth telling the user about, because we keep them
/// and cannot see inside them.
const OPAQUE: &[(&str, &str)] = &[
    ("vbaProject.bin", "a macro project, which can hold anything including the author's name"),
    ("customXml/", "custom XML data, often bound to a document management system"),
    ("oleObject", "an embedded OLE object, which carries its own separate metadata"),
    ("embeddings/", "an embedded file, which carries its own separate metadata"),
];

pub(crate) fn sanitize(
    input: &[u8],
    policy: &Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    // Parts are edited rather than rebuilt, and unknown parts are kept because
    // in these formats they are usually content.
    report.assurance = Assurance::BestEffort;

    let mut archive = Archive::read(input, report)?;
    let mut warn_opaque: Vec<&str> = Vec::new();

    for entry in &mut archive.entries {
        let path = entry.path().into_owned();

        // Whole-file replacements first: anything matched here never has its
        // original content inspected, let alone carried forward.
        let replacement = match path.as_str() {
            CORE => Some((EMPTY_CORE, Kind::DocumentInfo, "creator, last editor, revision, dates")),
            APP => {
                Some((EMPTY_APP, Kind::DocumentInfo, "application, company, manager, edit time"))
            }
            CUSTOM => Some((EMPTY_CUSTOM, Kind::CustomProperty, "custom properties")),
            OD_META => Some((EMPTY_OD_META, Kind::DocumentInfo, "creator, editing cycles, dates")),
            _ => None,
        };
        if let Some((empty, kind, what)) = replacement {
            let before = entry.size as usize;
            entry.write(empty.as_bytes());
            report.removed(kind, format!("{path} ({what})"), before.saturating_sub(empty.len()));
            continue;
        }

        // The thumbnail is a rendering of the first page. It is a preview of
        // the content rather than a property, so it stays, but it arrives with
        // its own EXIF and gets the same treatment as any other image.
        if path == THUMBNAIL || is_media(&path) {
            if policy.recurse_embedded {
                scrub_embedded(entry, policy, report, &path);
            } else {
                report.warn(format!(
                    "{path} was left alone because embedded images were not being processed; \
                     it may still carry its own EXIF"
                ));
            }
            continue;
        }

        for (needle, what) in OPAQUE {
            if path.contains(needle) {
                warn_opaque.push(what);
            }
        }

        if !path.ends_with(".xml") && !path.ends_with(".rels") {
            continue;
        }
        scrub_xml_part(entry, report, &path)?;
    }

    warn_opaque.sort_unstable();
    warn_opaque.dedup();
    for what in warn_opaque {
        report.warn(format!("this document contains {what}; that part was kept as it is"));
    }

    report.warn(
        "office documents were cleaned by editing the parts we know about, not by rebuilding \
         the file, so an application-specific part could still hold something; the property \
         files, tracked-change authors, revision identifiers and embedded images were handled",
    );

    Ok(archive.write())
}

fn is_media(path: &str) -> bool {
    ["word/media/", "xl/media/", "ppt/media/", "media/", "Pictures/"]
        .iter()
        .any(|prefix| path.starts_with(prefix))
}

/// Run an embedded image back through the top-level sanitizer.
fn scrub_embedded(entry: &mut crate::zip::Entry, policy: &Policy, report: &mut Report, path: &str) {
    let Ok(content) = entry.read() else {
        report.warn(format!("{path} could not be decompressed, so it was left as it is"));
        return;
    };
    match crate::sanitize(&content, policy) {
        Ok(clean) => {
            if !clean.report.removed.is_empty() {
                entry.write(&clean.data);
            }
            report.absorb(path, clean.report);
        }
        Err(e) => {
            report.warn(format!("{path} could not be sanitized ({e}), so it was left as it is"))
        }
    }
}

/// Apply the attribute rules, and drop the revision-identifier block from the
/// settings parts.
fn scrub_xml_part(
    entry: &mut crate::zip::Entry,
    report: &mut Report,
    path: &str,
) -> crate::Result<()> {
    let content = entry.read()?;

    let (content, dropped_elements) = if path.ends_with("settings.xml") {
        xmlscrub::remove_elements(&content, &["rsids"])
    } else {
        (content, 0)
    };
    let (content, counts) = xmlscrub::scrub_attributes(&content, DOCUMENT_RULES);

    if dropped_elements > 0 {
        report.removed(Kind::RevisionIds, format!("{path} (rsids block)"), 0);
    }
    if counts.blanked > 0 {
        report.removed(Kind::Author, format!("{path} ({} names)", counts.blanked), 0);
    }
    if counts.removed > 0 {
        // Both categories come out of the same pass; naming the part is enough
        // for a user to see where the identity data lived.
        report.removed(
            Kind::RevisionIds,
            format!("{path} ({} timestamps and revision ids)", counts.removed),
            0,
        );
    }
    if dropped_elements > 0 || counts.any() {
        entry.write(&content);
    }
    Ok(())
}

/// Exposed so the rule table cannot drift from what the documentation claims.
#[cfg(test)]
fn blanks_to(local_name: &str) -> Option<xmlscrub::Action> {
    DOCUMENT_RULES.iter().find(|r| r.name == local_name).map(|r| r.action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::crc32;
    use crate::xmlscrub::Action;
    use crate::zip::{Archive, Entry};
    use crate::{Format, Kind};

    fn entry(name: &str, body: &[u8]) -> Entry {
        Entry {
            name: name.as_bytes().to_vec(),
            method: 8,
            crc: crc32(body),
            stored: miniz_oxide::deflate::compress_to_vec(body, 6),
            size: body.len() as u32,
            utf8: true,
        }
    }

    fn docx(parts: &[(&str, &[u8])]) -> Vec<u8> {
        let mut entries = vec![entry("[Content_Types].xml", b"<Types/>")];
        entries.extend(parts.iter().map(|(n, b)| entry(n, b)));
        Archive { entries }.write()
    }

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Ooxml, input.len());
        let out = sanitize(input, &Policy::default(), &mut report).expect("valid docx");
        (out, report)
    }

    fn part_bytes(out: &[u8], name: &str) -> Vec<u8> {
        let archive = Archive::read(out, &mut Report::new(Format::Ooxml, 0)).unwrap();
        archive.find(name).unwrap_or_else(|| panic!("{name} is missing")).read().unwrap()
    }

    fn part(out: &[u8], name: &str) -> String {
        String::from_utf8_lossy(&part_bytes(out, name)).into_owned()
    }

    const REAL_CORE: &[u8] = br#"<?xml version="1.0"?><cp:coreProperties xmlns:cp="x" xmlns:dc="y">
<dc:creator>Jane Q. Author</dc:creator><cp:lastModifiedBy>Bob Reviewer</cp:lastModifiedBy>
<cp:revision>17</cp:revision><dcterms:created>2026-01-02T09:15:00Z</dcterms:created>
</cp:coreProperties>"#;

    const REAL_APP: &[u8] = br#"<?xml version="1.0"?><Properties xmlns="z">
<Application>Microsoft Office Word</Application><Company>Acme Legal LLP</Company>
<Manager>Directorate</Manager><TotalTime>438</TotalTime><Template>house-style.dotx</Template>
</Properties>"#;

    #[test]
    fn the_property_parts_are_replaced_rather_than_edited() {
        let (out, report) = run(&docx(&[(CORE, REAL_CORE), (APP, REAL_APP)]));

        for needle in ["Jane Q. Author", "Bob Reviewer", "Acme Legal LLP", "Directorate", "438"] {
            assert!(
                !out.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} survived"
            );
        }
        assert_eq!(part(&out, CORE), EMPTY_CORE, "the part must be the canonical empty one");
        assert_eq!(part(&out, APP), EMPTY_APP);
        assert_eq!(report.removed.iter().filter(|r| r.kind == Kind::DocumentInfo).count(), 2);
    }

    #[test]
    fn custom_properties_are_emptied_but_the_part_stays_so_the_relationship_holds() {
        // Deleting the part would leave a dangling relationship and content
        // type override, which Word reports as a corrupt file.
        let custom = br#"<Properties><property name="MatterNumber"><vt:lpwstr>ACME-2026-0042</vt:lpwstr></property></Properties>"#;
        let (out, report) = run(&docx(&[(CUSTOM, custom)]));

        assert!(!out.windows(14).any(|w| w == b"ACME-2026-0042"));
        assert_eq!(part(&out, CUSTOM), EMPTY_CUSTOM);
        assert!(report.removed.iter().any(|r| r.kind == Kind::CustomProperty));
    }

    #[test]
    fn tracked_change_and_comment_authors_are_removed_from_the_body() {
        let document = br#"<w:document><w:body><w:p w:rsidR="00A12B34" w:rsidRDefault="00A12B34">
<w:ins w:id="1" w:author="Jane Q. Author" w:date="2026-03-04T10:00:00Z"><w:r><w:t>added</w:t></w:r></w:ins>
</w:p></w:body></w:document>"#;
        let comments = br#"<w:comments><w:comment w:id="1" w:author="Bob Reviewer" w:initials="BR" w:date="2026-03-05T11:00:00Z"><w:p><w:t>check this</w:t></w:p></w:comment></w:comments>"#;

        let (out, report) =
            run(&docx(&[("word/document.xml", document), ("word/comments.xml", comments)]));

        for needle in ["Jane Q. Author", "Bob Reviewer", "2026-03-04", "00A12B34"] {
            assert!(
                !out.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "{needle} survived"
            );
        }
        // The content itself is untouched.
        assert!(part(&out, "word/document.xml").contains("added"));
        assert!(part(&out, "word/comments.xml").contains("check this"));

        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Author));
        assert!(kinds.contains(&Kind::RevisionIds));
    }

    #[test]
    fn the_revision_identifier_block_is_dropped_from_settings() {
        let settings = br#"<w:settings><w:zoom w:percent="100"/><w:rsids><w:rsidRoot w:val="00A12B34"/><w:rsid w:val="00C56D78"/></w:rsids></w:settings>"#;
        let (out, report) = run(&docx(&[("word/settings.xml", settings)]));

        assert!(!out.windows(8).any(|w| w == b"00A12B34"));
        assert!(!out.windows(8).any(|w| w == b"00C56D78"));
        assert!(part(&out, "word/settings.xml").contains("w:zoom"), "real settings must survive");
        assert!(report.removed.iter().any(|r| r.kind == Kind::RevisionIds));
    }

    #[test]
    fn an_embedded_photo_is_sanitized_and_its_gps_reported_at_the_document_level() {
        let photo = jpeg_with_gps();
        let (out, report) = run(&docx(&[("word/media/image1.jpeg", &photo)]));

        let cleaned = part_bytes(&out, "word/media/image1.jpeg");
        assert!(!cleaned.windows(16).any(|w| w == b"CameraSerial9988"));
        assert!(!cleaned.windows(6).any(|w| w == b"Exif\0\0"));
        assert!(report.found_location, "GPS in an embedded photo is GPS in the document");
        assert!(
            report.removed.iter().any(|r| r.location.starts_with("word/media/image1.jpeg →")),
            "the finding must say which part it came from"
        );
    }

    #[test]
    fn embedded_images_can_be_left_alone_and_that_is_said_out_loud() {
        let photo = jpeg_with_gps();
        let input = docx(&[("word/media/image1.jpeg", &photo)]);
        let policy = Policy { recurse_embedded: false, ..Policy::default() };

        let mut report = Report::new(Format::Ooxml, input.len());
        let out = sanitize(&input, &policy, &mut report).unwrap();

        // The part is deflated inside the archive, so check the content itself
        // rather than the container bytes.
        let kept = part_bytes(&out, "word/media/image1.jpeg");
        assert_eq!(kept, photo, "the image should be exactly as it arrived");
        assert!(report.warnings.iter().any(|w| w.contains("may still carry its own EXIF")));
    }

    #[test]
    fn macros_and_embedded_objects_are_kept_but_flagged() {
        let (_, report) = run(&docx(&[
            ("word/vbaProject.bin", b"\x00macro blob"),
            ("customXml/item1.xml", b"<root/>"),
        ]));
        assert!(report.warnings.iter().any(|w| w.contains("macro project")));
        assert!(report.warnings.iter().any(|w| w.contains("custom XML data")));
    }

    #[test]
    fn opendocument_metadata_is_replaced() {
        let meta = br#"<office:document-meta><office:meta><meta:initial-creator>Jane Q. Author</meta:initial-creator><meta:editing-cycles>23</meta:editing-cycles><meta:generator>LibreOffice/7.6</meta:generator></office:meta></office:document-meta>"#;
        let odt = {
            let mut entries = vec![Entry {
                name: b"mimetype".to_vec(),
                method: 0,
                crc: crc32(b"application/vnd.oasis.opendocument.text"),
                stored: b"application/vnd.oasis.opendocument.text".to_vec(),
                size: 39,
                utf8: false,
            }];
            entries.push(entry(OD_META, meta));
            Archive { entries }.write()
        };

        let mut report = Report::new(Format::OpenDocument, odt.len());
        let out = sanitize(&odt, &Policy::default(), &mut report).unwrap();

        assert!(!out.windows(14).any(|w| w == b"Jane Q. Author"));
        assert!(!out.windows(11).any(|w| w == b"LibreOffice"));
        assert_eq!(part(&out, OD_META), EMPTY_OD_META);
        // The mimetype entry must stay first and stay uncompressed.
        let archive = Archive::read(&out, &mut Report::new(Format::OpenDocument, 0)).unwrap();
        assert_eq!(archive.entries[0].path(), "mimetype");
        assert_eq!(archive.entries[0].method, 0);
    }

    #[test]
    fn the_result_is_never_advertised_as_a_complete_strip() {
        let (_, report) = run(&docx(&[(CORE, REAL_CORE)]));
        assert_eq!(report.assurance, Assurance::BestEffort);
        assert!(report.warnings.iter().any(|w| w.contains("not by rebuilding")));
    }

    #[test]
    fn archive_level_timestamps_are_normalized() {
        let (_, report) = run(&docx(&[(CORE, REAL_CORE)]));
        assert!(report.removed.iter().any(|r| r.kind == Kind::Timestamp));
    }

    #[test]
    fn a_document_with_nothing_to_remove_still_round_trips() {
        let input = docx(&[("word/document.xml", b"<w:document><w:body/></w:document>")]);
        let (out, _) = run(&input);
        let archive = Archive::read(&out, &mut Report::new(Format::Ooxml, 0)).unwrap();
        assert_eq!(archive.entries.len(), 2);
        assert_eq!(part(&out, "word/document.xml"), "<w:document><w:body/></w:document>");
    }

    #[test]
    fn the_rules_match_what_the_module_documentation_claims() {
        assert_eq!(blanks_to("author"), Some(Action::Blank("author")));
        assert_eq!(blanks_to("date"), Some(Action::Remove));
        assert_eq!(blanks_to("rsid"), Some(Action::Remove));
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let full = docx(&[(CORE, REAL_CORE), ("word/document.xml", b"<w:p w:author=\"A\"/>")]);
        for n in 0..full.len() {
            let mut report = Report::new(Format::Ooxml, n);
            let _ = sanitize(&full[..n], &Policy::default(), &mut report);
        }
    }

    /// A JPEG carrying a GPS sub-IFD and a distinctive maker-note string.
    fn jpeg_with_gps() -> Vec<u8> {
        let mut p = b"Exif\0\0".to_vec();
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
        p.extend_from_slice(b"CameraSerial9988");

        let mut j = vec![0xFF, 0xD8];
        j.push(0xFF);
        j.push(0xE1);
        j.extend_from_slice(&((p.len() + 2) as u16).to_be_bytes());
        j.extend_from_slice(&p);
        j.extend_from_slice(&[0xFF, 0xDA, 0x00, 0x08, 1, 1, 0, 0, 63, 0]);
        j.extend_from_slice(&[0x12, 0x34]);
        j.extend_from_slice(&[0xFF, 0xD9]);
        j
    }
}
