//! XMP: rebuild the packet empty.
//!
//! An XMP file is a sidecar — it travels beside a photo and holds nothing *but*
//! metadata: the author, copyright, GPS, the dates the photo was taken and
//! edited, the editing history (which links a file to a session on a machine),
//! catalogue identifiers, and the camera serial number. There is no image data
//! in it to preserve.
//!
//! So there is nothing to rebuild an allowlist *from*: the safe output is an
//! empty, valid XMP packet. Nothing from the input can survive, because none of
//! it is copied across. That makes this a [`Complete`](crate::Assurance::Complete)
//! result. What was found is reported first, so the user knows what the sidecar
//! had been carrying.

use crate::report::{Kind, Report};
use crate::util::starts_with_ignore_ascii_case;

/// A minimal, valid, empty XMP packet.
const EMPTY_XMP: &[u8] = b"<?xpacket begin=\"\xEF\xBB\xBF\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n \
<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n  \
<rdf:Description rdf:about=\"\"/>\n \
</rdf:RDF>\n\
</x:xmpmeta>\n\
<?xpacket end=\"w\"?>\n";

/// Identifying properties we look for, only to report them. The output is empty
/// regardless of what is found, so this list does not have to be exhaustive to
/// be safe; it exists to tell the user what the sidecar held.
const MARKERS: &[(&[u8], Kind)] = &[
    (b"dc:creator", Kind::Author),
    (b"dc:rights", Kind::Author),
    (b"photoshop:Credit", Kind::Author),
    (b"photoshop:AuthorsPosition", Kind::Author),
    (b"aux:SerialNumber", Kind::MakerNote),
    (b"aux:LensSerialNumber", Kind::MakerNote),
    (b"xmp:CreateDate", Kind::Timestamp),
    (b"xmp:ModifyDate", Kind::Timestamp),
    (b"xmp:MetadataDate", Kind::Timestamp),
    (b"photoshop:DateCreated", Kind::Timestamp),
    (b"exif:DateTimeOriginal", Kind::Timestamp),
    (b"exif:GPS", Kind::Exif),
    (b"xmpMM:History", Kind::RevisionIds),
    (b"xmpMM:DocumentID", Kind::RevisionIds),
    (b"xmpMM:InstanceID", Kind::RevisionIds),
    (b"xmpMM:OriginalDocumentID", Kind::RevisionIds),
    (b"dc:subject", Kind::CustomProperty),
    (b"lr:hierarchicalSubject", Kind::CustomProperty),
];

pub(crate) fn sanitize(
    input: &[u8],
    _policy: &crate::Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    // Report what the sidecar carried, so the outcome is not just "here is an
    // empty file". GPS gets the same special surfacing it does everywhere.
    for (marker, kind) in MARKERS {
        if window_contains(input, marker) {
            report.removed(*kind, format!("XMP property {}", String::from_utf8_lossy(marker)), 0);
            if marker.starts_with(b"exif:GPS") {
                report.found_location = true;
            }
        }
    }
    report.warn(
        "this is an XMP sidecar: a file that is entirely metadata, carried beside a photo. It was \
         replaced with an empty packet, so nothing it held survives. The photo it describes is a \
         separate file and must be cleaned on its own.",
    );
    Ok(EMPTY_XMP.to_vec())
}

fn window_contains(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || haystack.len() < needle.len() {
        return false;
    }
    haystack.windows(needle.len()).any(|w| starts_with_ignore_ascii_case(w, needle))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Assurance, Report};
    use crate::Format;

    fn run(input: &[u8]) -> (Vec<u8>, Report) {
        let mut report = Report::new(Format::Xmp, input.len());
        let out = sanitize(input, &crate::Policy::default(), &mut report).unwrap();
        (out, report)
    }

    #[test]
    fn a_sidecar_is_emptied_and_its_contents_reported() {
        let xmp = br#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="r">
<rdf:Description xmp:CreateDate="2026-01-02T03:04:05" aux:SerialNumber="BODY-99887766"
 exif:GPSLatitude="48,51.5N">
<dc:creator><rdf:Seq><rdf:li>Jane Photographer</rdf:li></rdf:Seq></dc:creator>
<xmpMM:History>edited on jane-laptop</xmpMM:History>
</rdf:Description></rdf:RDF></x:xmpmeta><?xpacket end="w"?>"#;
        let (out, report) = run(xmp);

        // Nothing identifying survives.
        for needle in [
            &b"Jane Photographer"[..],
            b"BODY-99887766",
            b"2026-01-02",
            b"48,51.5N",
            b"jane-laptop",
        ] {
            assert!(
                !out.windows(needle.len()).any(|w| w == needle),
                "{:?} survived",
                String::from_utf8_lossy(needle)
            );
        }
        // The output is still a valid, empty XMP packet.
        assert!(out.windows(9).any(|w| w == b"<?xpacket"));
        assert!(out.windows(9).any(|w| w == b"x:xmpmeta"));
        // It was found to carry a serial, a date, GPS and an author.
        let kinds: Vec<_> = report.removed.iter().map(|r| r.kind).collect();
        assert!(kinds.contains(&Kind::Author));
        assert!(kinds.contains(&Kind::Timestamp));
        assert!(kinds.contains(&Kind::MakerNote));
        assert!(report.found_location, "GPS in a sidecar is still a location leak");
        assert_eq!(report.assurance, Assurance::Complete);
    }

    #[test]
    fn a_sidecar_with_nothing_we_flag_still_becomes_empty() {
        let (out, report) = run(b"<x:xmpmeta><rdf:RDF/></x:xmpmeta>");
        assert!(out.windows(9).any(|w| w == b"<?xpacket"));
        // No identifying markers, but the emptying warning is still present so
        // the outcome is never silent.
        assert!(!report.warnings.is_empty());
    }
}
