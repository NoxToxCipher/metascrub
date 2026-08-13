//! The account of what came out of a file.

use crate::detect::Format;

/// How much the result can be trusted.
///
/// This exists so the interface can never quietly imply more than was done.
/// A user deciding whether to send a photo needs "we rebuilt this from an
/// allowlist" and "we blanked what we could find" to look different.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Assurance {
    /// The container was rebuilt from an explicit keep-list. Nothing outside
    /// that list survives, including structures this crate does not recognise.
    Complete,

    /// Known metadata was located and removed or overwritten, but the container
    /// was not rebuilt from scratch, so an unrecognised structure could remain.
    /// The accompanying warnings say what specifically was not guaranteed.
    BestEffort,

    /// Nothing was removed. The file is returned exactly as it arrived.
    None,
}

impl std::fmt::Display for Assurance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Assurance::Complete => "complete",
            Assurance::BestEffort => "best effort",
            Assurance::None => "none",
        })
    }
}

/// The category of a removed item, for grouping in a user interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// EXIF / TIFF tag block: camera, lens, exposure, timestamps, GPS.
    Exif,
    /// XMP packet (Adobe's RDF/XML metadata), including the extended form.
    Xmp,
    /// IPTC-IIM or a Photoshop image resource block.
    Iptc,
    /// Vendor-private EXIF maker note: serial numbers, shutter counts.
    MakerNote,
    /// An embedded preview or thumbnail image. These survive naive edits, so a
    /// cropped photo can still ship the uncropped original in its thumbnail.
    Thumbnail,
    /// ICC colour profile.
    ColorProfile,
    /// A free-text comment field.
    Comment,
    /// Bytes appended after the container's own end marker.
    Trailer,
    /// A container structure that is not on the keep-list.
    UnknownStructure,
    /// PDF document information dictionary, or an equivalent document property.
    DocumentInfo,
    /// Author, editor or reviewer identity.
    Author,
    /// A creation, modification or editing timestamp.
    Timestamp,
    /// Revision-save identifiers, which link documents edited in one session.
    RevisionIds,
    /// Archive-level metadata: entry timestamps, uids, host filesystem fields.
    ArchiveEntry,
    /// Application-specific custom properties.
    CustomProperty,
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Kind::Exif => "EXIF",
            Kind::Xmp => "XMP",
            Kind::Iptc => "IPTC",
            Kind::MakerNote => "maker note",
            Kind::Thumbnail => "thumbnail",
            Kind::ColorProfile => "colour profile",
            Kind::Comment => "comment",
            Kind::Trailer => "trailing data",
            Kind::UnknownStructure => "unrecognised structure",
            Kind::DocumentInfo => "document info",
            Kind::Author => "author identity",
            Kind::Timestamp => "timestamp",
            Kind::RevisionIds => "revision identifiers",
            Kind::ArchiveEntry => "archive entry metadata",
            Kind::CustomProperty => "custom property",
        })
    }
}

/// The result of checking a clean against itself: the tool's own homework,
/// marked. Present only when a verify pass was run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verification {
    /// A fresh scan of the cleaned output found nothing left that this tool
    /// removes. For a `Complete` clean this should always hold; if it does not,
    /// the output still carries metadata and must not be trusted.
    pub output_reinspected_clean: bool,
    /// Cleaning the same input twice produced byte-identical output, so nothing
    /// varying per run (a stray timestamp, random padding) leaked into the file.
    pub deterministic: bool,
}

impl Verification {
    /// True only if both checks held.
    pub fn passed(&self) -> bool {
        self.output_reinspected_clean && self.deterministic
    }
}

/// One piece of identifying data the tool knowingly left in the file, with a
/// plain statement of what it would reveal to someone examining the file.
///
/// This exists because a best-effort clean that quietly leaves things behind is
/// worse than one that says exactly what it could not do. A user deciding whether
/// to send a file needs the residual risk spelled out, not implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Retained {
    /// What is still in the file, and why it was kept.
    pub what: String,
    /// What that data would tell someone who inspected the file.
    pub reveals: String,
}

/// One thing that was taken out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    /// Category, for grouping.
    pub kind: Kind,
    /// Where it lived, in terms of the container: a JPEG marker, a PNG chunk
    /// type, a path inside an archive.
    pub location: String,
    /// How many bytes went away. Zero when the item was overwritten in place
    /// rather than excised.
    pub bytes: usize,
}

/// What happened to one file.
#[derive(Debug, Clone)]
pub struct Report {
    /// Detected container format.
    pub format: Format,
    /// How much the result can be trusted. See [`Assurance`].
    pub assurance: Assurance,
    /// Every item that was removed, in the order encountered.
    pub removed: Vec<Removed>,
    /// Identifying data knowingly left in the file, each with what it reveals.
    /// Non-empty means the clean was partial by necessity; the interface should
    /// surface this prominently rather than let it read as a full clean.
    pub retained: Vec<Retained>,
    /// Things the user should know that are not removals: what we could not
    /// guarantee, and what remains in the file on purpose.
    pub warnings: Vec<String>,
    /// True when the input carried GPS coordinates.
    ///
    /// Surfaced on its own because it is the finding most likely to change
    /// someone's mind about sending a file.
    pub found_location: bool,
    /// Input size in bytes.
    pub input_len: usize,
    /// Output size in bytes.
    pub output_len: usize,
    /// Filled in when the caller asked the tool to check its own output. See
    /// [`Verification`].
    pub verification: Option<Verification>,
}

impl Report {
    pub(crate) fn new(format: Format, input_len: usize) -> Self {
        Self {
            format,
            assurance: Assurance::Complete,
            removed: Vec::new(),
            retained: Vec::new(),
            warnings: Vec::new(),
            found_location: false,
            input_len,
            output_len: 0,
            verification: None,
        }
    }

    pub(crate) fn removed(&mut self, kind: Kind, location: impl Into<String>, bytes: usize) {
        self.removed.push(Removed { kind, location: location.into(), bytes });
    }

    /// Record identifying data left in the file. Deduplicated on `what`, since
    /// the same residual (a kept maker note, say) is reached from several code
    /// paths but should be told to the user once.
    pub(crate) fn retain(&mut self, what: impl Into<String>, reveals: impl Into<String>) {
        let what = what.into();
        if !self.retained.iter().any(|r| r.what == what) {
            self.retained.push(Retained { what, reveals: reveals.into() });
        }
    }

    pub(crate) fn warn(&mut self, msg: impl Into<String>) {
        self.warnings.push(msg.into());
    }

    /// Merge a nested report (an image inside a document) into this one,
    /// prefixing each location with the archive path it came from.
    // Only the archive backends nest a sanitizer inside another one.
    #[cfg_attr(not(feature = "ooxml"), allow(dead_code))]
    pub(crate) fn absorb(&mut self, prefix: &str, other: Report) {
        for item in other.removed {
            self.removed.push(Removed {
                kind: item.kind,
                location: format!("{prefix} → {}", item.location),
                bytes: item.bytes,
            });
        }
        for warning in other.warnings {
            self.warn(format!("{prefix}: {warning}"));
        }
        for r in other.retained {
            self.retain(r.what, r.reveals);
        }
        self.found_location |= other.found_location;
    }

    /// True when nothing was found to remove.
    pub fn is_clean(&self) -> bool {
        self.removed.is_empty()
    }

    /// Total bytes of metadata removed.
    pub fn bytes_removed(&self) -> usize {
        self.removed.iter().map(|r| r.bytes).sum()
    }

    /// A one-line human summary, in the plain register the project's copy uses
    /// (DESIGN §8: no em dashes, prefer the plain word).
    pub fn summary(&self) -> String {
        match self.assurance {
            Assurance::None => {
                format!("{}: not sanitized, nothing was removed", self.format)
            }
            _ if self.removed.is_empty() => {
                format!("{}: no metadata found", self.format)
            }
            _ => {
                let gps = if self.found_location { ", including GPS coordinates" } else { "" };
                format!(
                    "{}: removed {} item(s), {} bytes{} ({} assurance)",
                    self.format,
                    self.removed.len(),
                    self.bytes_removed(),
                    gps,
                    self.assurance,
                )
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorbing_a_nested_report_keeps_the_path_and_the_gps_flag() {
        let mut outer = Report::new(Format::Ooxml, 100);
        let mut inner = Report::new(Format::Jpeg, 50);
        inner.removed(Kind::Exif, "APP1", 40);
        inner.found_location = true;
        inner.warn("something to pass along");

        outer.absorb("word/media/image1.jpeg", inner);

        assert!(outer.found_location, "GPS in an embedded image is still GPS in the document");
        assert_eq!(outer.removed[0].location, "word/media/image1.jpeg → APP1");
        assert_eq!(outer.warnings[0], "word/media/image1.jpeg: something to pass along");
    }

    #[test]
    fn summary_calls_out_gps_and_never_claims_a_clean_unknown() {
        let mut r = Report::new(Format::Jpeg, 10);
        r.removed(Kind::Exif, "APP1", 40);
        r.found_location = true;
        assert!(r.summary().contains("GPS"));

        let unknown = Report { assurance: Assurance::None, ..Report::new(Format::Unknown, 10) };
        assert!(unknown.summary().contains("not sanitized"));
        assert!(!unknown.summary().contains("no metadata"));
    }
}
