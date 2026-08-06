//! A minimal ZIP reader and writer, for the document formats that are really
//! archives.
//!
//! There is a perfectly good ZIP crate. This is hand-rolled anyway, for two
//! reasons that both come back to the job at hand.
//!
//! First, **the archive itself is metadata**. Every entry carries a
//! last-modified timestamp, and the extra-field area routinely carries a second
//! set of higher-resolution timestamps, the Unix user and group that owned the
//! file, and the NTFS creation time. A general-purpose library preserves those
//! faithfully, which is the opposite of what is wanted here. Writing the
//! headers means every field is chosen on purpose: entries come out dated
//! 1980-01-01, extra fields and comments are dropped, and the host-system byte
//! stops announcing which operating system produced the document.
//!
//! Second, entries that need no editing are copied across **still compressed**.
//! Their bytes never go through inflate and deflate, so the output is
//! bit-identical for everything except the parts we deliberately rewrote, and
//! a large presentation is not re-compressed just to change one property file.

use crate::error::Error;
use crate::report::{Kind, Report};
use crate::util::{crc32, Reader};

const FORMAT: &str = "ZIP";

const LOCAL_SIG: &[u8; 4] = b"PK\x03\x04";
const CENTRAL_SIG: &[u8; 4] = b"PK\x01\x02";
const EOCD_SIG: &[u8; 4] = b"PK\x05\x06";
const ZIP64_LOCATOR_SIG: &[u8; 4] = b"PK\x06\x07";

const METHOD_STORE: u16 = 0;
const METHOD_DEFLATE: u16 = 8;

/// Flag bit 0: the entry is encrypted.
const FLAG_ENCRYPTED: u16 = 1 << 0;
/// Flag bit 11: the name is UTF-8 rather than the legacy code page.
const FLAG_UTF8: u16 = 1 << 11;

/// A fixed timestamp for every entry: 1980-01-01 00:00:00, the earliest the
/// format can express.
const DOS_DATE: u16 = 0x0021;
const DOS_TIME: u16 = 0x0000;

/// Cap on what one entry may inflate to. Office parts are small; a part that
/// expands past this is a compression bomb, not a document.
const MAX_INFLATED: usize = 128 * 1024 * 1024;

/// One archive member.
#[derive(Debug, Clone)]
pub(crate) struct Entry {
    /// Path inside the archive, as raw bytes.
    pub name: Vec<u8>,
    /// Compression method, preserved so an OpenDocument `mimetype` entry stays
    /// stored rather than becoming deflated, which would break the format's
    /// magic-byte convention.
    pub method: u16,
    /// CRC-32 of the uncompressed content.
    pub crc: u32,
    /// The content exactly as stored, still compressed.
    pub stored: Vec<u8>,
    /// Uncompressed length.
    pub size: u32,
    /// Whether the name is UTF-8.
    pub utf8: bool,
}

impl Entry {
    /// The entry's name as text, for matching and reporting.
    pub(crate) fn path(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.name)
    }

    /// Decompress the content.
    pub(crate) fn read(&self) -> crate::Result<Vec<u8>> {
        match self.method {
            METHOD_STORE => Ok(self.stored.clone()),
            METHOD_DEFLATE => {
                miniz_oxide::inflate::decompress_to_vec_with_limit(&self.stored, MAX_INFLATED)
                    .map_err(|e| {
                        Error::malformed(FORMAT, format!("{} did not inflate: {e:?}", self.path()))
                    })
            }
            other => Err(Error::unsupported(
                FORMAT,
                format!("{} uses compression method {other}", self.path()),
            )),
        }
    }

    /// Replace the content, recompressing with the entry's existing method.
    pub(crate) fn write(&mut self, data: &[u8]) {
        self.crc = crc32(data);
        self.size = data.len() as u32;
        self.stored = match self.method {
            METHOD_STORE => data.to_vec(),
            _ => {
                self.method = METHOD_DEFLATE;
                miniz_oxide::deflate::compress_to_vec(data, 6)
            }
        };
    }
}

/// A parsed archive.
#[derive(Debug, Clone)]
pub(crate) struct Archive {
    pub entries: Vec<Entry>,
}

impl Archive {
    /// Parse an archive from the central directory, which is the authoritative
    /// index. Local headers are consulted only to find where each entry's data
    /// begins, because their name and extra-field lengths can differ from the
    /// central copy and their sizes are allowed to be zero.
    pub(crate) fn read(input: &[u8], report: &mut Report) -> crate::Result<Self> {
        let eocd = find_eocd(input)
            .ok_or_else(|| Error::malformed(FORMAT, "no end-of-central-directory record"))?;

        let mut r = Reader::new(input);
        r.seek(eocd + 4).ok_or_else(|| Error::malformed(FORMAT, "truncated EOCD"))?;
        let (_disk, _cd_disk, _here, total) =
            (r.u16_le(), r.u16_le(), r.u16_le(), r.u16_le().unwrap_or(0));
        let cd_size = r.u32_le().unwrap_or(0);
        let cd_offset = r.u32_le().unwrap_or(0);
        let comment_len = r.u16_le().unwrap_or(0);

        if total == u16::MAX || cd_size == u32::MAX || cd_offset == u32::MAX {
            return Err(Error::unsupported(FORMAT, "ZIP64 archives are not handled"));
        }
        if input.len() >= 4 && find_zip64_locator(input, eocd) {
            return Err(Error::unsupported(FORMAT, "ZIP64 archives are not handled"));
        }
        if comment_len > 0 {
            report.removed(Kind::ArchiveEntry, "archive comment", comment_len as usize);
        }

        let mut r = Reader::new(input);
        r.seek(cd_offset as usize)
            .ok_or_else(|| Error::malformed(FORMAT, "central directory offset is out of range"))?;

        let mut entries = Vec::with_capacity(total as usize);
        let mut dropped_extra = 0usize;

        for _ in 0..total {
            if r.take(4) != Some(CENTRAL_SIG) {
                return Err(Error::malformed(FORMAT, "bad central directory signature"));
            }
            let mut field = || r.u16_le().unwrap_or(0);
            let (_made_by, _needed, flags, method, _time, _date) =
                (field(), field(), field(), field(), field(), field());
            let crc = r.u32_le().unwrap_or(0);
            let comp_size = r.u32_le().unwrap_or(0);
            let size = r.u32_le().unwrap_or(0);
            let name_len = r.u16_le().unwrap_or(0) as usize;
            let extra_len = r.u16_le().unwrap_or(0) as usize;
            let comment_len = r.u16_le().unwrap_or(0) as usize;
            let (_disk, _internal) = (r.u16_le(), r.u16_le());
            let _external = r.u32_le();
            let local_offset = r
                .u32_le()
                .ok_or_else(|| Error::malformed(FORMAT, "truncated central directory entry"))?;

            let name = r
                .take(name_len)
                .ok_or_else(|| Error::malformed(FORMAT, "truncated entry name"))?
                .to_vec();
            let _ = r.take(extra_len);
            let _ = r.take(comment_len);
            dropped_extra += extra_len + comment_len;

            if flags & FLAG_ENCRYPTED != 0 {
                return Err(Error::Encrypted("this archive"));
            }

            let stored = locate_data(input, local_offset as usize, comp_size as usize)?;
            entries.push(Entry { name, method, crc, stored, size, utf8: flags & FLAG_UTF8 != 0 });
        }

        if dropped_extra > 0 {
            // Extra fields are where the high-resolution timestamps and the
            // Unix uid/gid live, so this is a real finding rather than noise.
            report.removed(Kind::ArchiveEntry, "entry extra fields and comments", dropped_extra);
        }
        report.removed(Kind::Timestamp, "archive entry timestamps", 0);

        Ok(Archive { entries })
    }

    /// Serialize the archive with every non-content field normalized.
    pub(crate) fn write(&self) -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::with_capacity(self.entries.len());

        for entry in &self.entries {
            offsets.push(out.len() as u32);
            out.extend_from_slice(LOCAL_SIG);
            out.extend_from_slice(&20u16.to_le_bytes()); // version needed: 2.0
            out.extend_from_slice(&flags(entry).to_le_bytes());
            out.extend_from_slice(&entry.method.to_le_bytes());
            out.extend_from_slice(&DOS_TIME.to_le_bytes());
            out.extend_from_slice(&DOS_DATE.to_le_bytes());
            out.extend_from_slice(&entry.crc.to_le_bytes());
            out.extend_from_slice(&(entry.stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&entry.size.to_le_bytes());
            out.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // no extra field
            out.extend_from_slice(&entry.name);
            out.extend_from_slice(&entry.stored);
        }

        let cd_offset = out.len() as u32;
        for (entry, offset) in self.entries.iter().zip(&offsets) {
            out.extend_from_slice(CENTRAL_SIG);
            // "Made by" MS-DOS 2.0. The host-system byte otherwise announces
            // which operating system wrote the document.
            out.extend_from_slice(&0x0014u16.to_le_bytes());
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&flags(entry).to_le_bytes());
            out.extend_from_slice(&entry.method.to_le_bytes());
            out.extend_from_slice(&DOS_TIME.to_le_bytes());
            out.extend_from_slice(&DOS_DATE.to_le_bytes());
            out.extend_from_slice(&entry.crc.to_le_bytes());
            out.extend_from_slice(&(entry.stored.len() as u32).to_le_bytes());
            out.extend_from_slice(&entry.size.to_le_bytes());
            out.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes()); // extra
            out.extend_from_slice(&0u16.to_le_bytes()); // comment
            out.extend_from_slice(&0u16.to_le_bytes()); // disk number
            out.extend_from_slice(&0u16.to_le_bytes()); // internal attributes
            out.extend_from_slice(&0u32.to_le_bytes()); // external attributes
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(&entry.name);
        }
        let cd_size = out.len() as u32 - cd_offset;

        out.extend_from_slice(EOCD_SIG);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        let count = self.entries.len() as u16;
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&count.to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // no archive comment
        out
    }

    /// Find an entry by exact path. Used by tests to assert on the result of a
    /// round trip; the sanitizer itself walks every entry in order.
    #[cfg(test)]
    pub(crate) fn find(&self, path: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.path() == path)
    }
}

/// The only general-purpose flag worth carrying forward. Everything else is
/// either about how the sizes were written, which we now control, or about
/// encryption, which we refused.
fn flags(entry: &Entry) -> u16 {
    if entry.utf8 {
        FLAG_UTF8
    } else {
        0
    }
}

/// Read a local file header and return the entry's stored bytes.
fn locate_data(input: &[u8], offset: usize, comp_size: usize) -> crate::Result<Vec<u8>> {
    let mut r = Reader::new(input);
    r.seek(offset).ok_or_else(|| Error::malformed(FORMAT, "local header offset out of range"))?;
    if r.take(4) != Some(LOCAL_SIG) {
        return Err(Error::malformed(FORMAT, "bad local header signature"));
    }
    // Skip to the two length fields, which is all we need from here: the
    // central directory is authoritative for everything else.
    r.take(22).ok_or_else(|| Error::malformed(FORMAT, "truncated local header"))?;
    let name_len = r.u16_le().unwrap_or(0) as usize;
    let extra_len = r.u16_le().unwrap_or(0) as usize;
    r.take(name_len + extra_len)
        .ok_or_else(|| Error::malformed(FORMAT, "truncated local header fields"))?;
    Ok(r.take(comp_size)
        .ok_or_else(|| Error::malformed(FORMAT, "entry data runs past the end of the file"))?
        .to_vec())
}

/// Locate the end-of-central-directory record.
///
/// It is at the end of the file unless there is an archive comment, and the
/// comment may itself contain the signature, so scan backwards and take the
/// last position whose declared comment length reaches exactly the end.
fn find_eocd(input: &[u8]) -> Option<usize> {
    const EOCD_LEN: usize = 22;
    if input.len() < EOCD_LEN {
        return None;
    }
    let earliest = input.len().saturating_sub(EOCD_LEN + u16::MAX as usize);
    for pos in (earliest..=input.len() - EOCD_LEN).rev() {
        if &input[pos..pos + 4] != EOCD_SIG {
            continue;
        }
        let comment_len = u16::from_le_bytes([input[pos + 20], input[pos + 21]]) as usize;
        if pos + EOCD_LEN + comment_len == input.len() {
            return Some(pos);
        }
    }
    None
}

/// A ZIP64 end-of-central-directory locator sits immediately before the EOCD.
fn find_zip64_locator(input: &[u8], eocd: usize) -> bool {
    eocd >= 20 && &input[eocd - 20..eocd - 16] == ZIP64_LOCATOR_SIG
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Format;

    /// Build an archive with stored (uncompressed) entries, independent of the
    /// writer under test, plus optional per-entry extra fields and timestamps.
    fn build(entries: &[(&str, &[u8])], extra: &[u8], comment: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut offsets = Vec::new();
        for (name, body) in entries {
            offsets.push(out.len() as u32);
            out.extend_from_slice(LOCAL_SIG);
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&METHOD_STORE.to_le_bytes());
            out.extend_from_slice(&0x1234u16.to_le_bytes()); // a real timestamp
            out.extend_from_slice(&0x5678u16.to_le_bytes());
            out.extend_from_slice(&crc32(body).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(extra);
            out.extend_from_slice(body);
        }
        let cd_offset = out.len() as u32;
        for ((name, body), offset) in entries.iter().zip(&offsets) {
            out.extend_from_slice(CENTRAL_SIG);
            out.extend_from_slice(&0x031Eu16.to_le_bytes()); // "made by Unix"
            out.extend_from_slice(&20u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&METHOD_STORE.to_le_bytes());
            out.extend_from_slice(&0x1234u16.to_le_bytes());
            out.extend_from_slice(&0x5678u16.to_le_bytes());
            out.extend_from_slice(&crc32(body).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(body.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u16).to_le_bytes());
            out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            out.extend_from_slice(&(comment.len() as u16).to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0u16.to_le_bytes());
            out.extend_from_slice(&0x81A4_0000u32.to_le_bytes()); // Unix mode
            out.extend_from_slice(&offset.to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(extra);
            out.extend_from_slice(comment);
        }
        let cd_size = out.len() as u32 - cd_offset;
        out.extend_from_slice(EOCD_SIG);
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&cd_size.to_le_bytes());
        out.extend_from_slice(&cd_offset.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out
    }

    fn report() -> Report {
        Report::new(Format::Ooxml, 0)
    }

    #[test]
    fn entries_round_trip_through_read_and_write() {
        let zip = build(&[("a.txt", b"hello"), ("dir/b.bin", &[0xFFu8; 300])], b"", b"");
        let archive = Archive::read(&zip, &mut report()).unwrap();
        assert_eq!(archive.entries.len(), 2);
        assert_eq!(archive.find("a.txt").unwrap().read().unwrap(), b"hello");

        let rewritten = Archive::read(&archive.write(), &mut report()).unwrap();
        assert_eq!(rewritten.find("dir/b.bin").unwrap().read().unwrap(), vec![0xFF; 300]);
    }

    #[test]
    fn timestamps_extra_fields_and_comments_do_not_survive() {
        // A Unix "extended timestamp" extra field, the sort a zip tool adds.
        let extra = b"UT\x05\x00\x01\x9A\x78\x56\x34";
        let zip = build(&[("a.txt", b"hi")], extra, b"an entry comment");

        let mut rep = report();
        let archive = Archive::read(&zip, &mut rep).unwrap();
        let out = archive.write();

        assert!(!out.windows(16).any(|w| w == b"an entry comment"));
        assert!(!out.windows(2).any(|w| w == b"UT"), "the extra field survived");
        assert!(!out.windows(2).any(|w| w == 0x1234u16.to_le_bytes()), "timestamp survived");
        assert!(out.windows(2).any(|w| w == DOS_DATE.to_le_bytes()));
        assert!(rep.removed.iter().any(|r| r.kind == Kind::ArchiveEntry));
        assert!(rep.removed.iter().any(|r| r.kind == Kind::Timestamp));
    }

    #[test]
    fn the_host_system_byte_is_normalized() {
        // "Made by Unix" tells a recipient what platform produced the file.
        let zip = build(&[("a.txt", b"hi")], b"", b"");
        let out = Archive::read(&zip, &mut report()).unwrap().write();

        let cd = out.windows(4).position(|w| w == CENTRAL_SIG).unwrap();
        assert_eq!(u16::from_le_bytes([out[cd + 4], out[cd + 5]]), 0x0014);
    }

    #[test]
    fn a_deflated_entry_is_copied_without_being_recompressed() {
        let body = b"the quick brown fox ".repeat(50);
        let deflated = miniz_oxide::deflate::compress_to_vec(&body, 9);

        let mut archive = Archive {
            entries: vec![Entry {
                name: b"x.xml".to_vec(),
                method: METHOD_DEFLATE,
                crc: crc32(&body),
                stored: deflated.clone(),
                size: body.len() as u32,
                utf8: true,
            }],
        };
        let out = archive.write();
        let back = Archive::read(&out, &mut report()).unwrap();
        assert_eq!(back.entries[0].stored, deflated, "the compressed bytes must be untouched");
        assert_eq!(back.entries[0].read().unwrap(), body);

        // Writing new content recompresses and refreshes the CRC.
        archive.entries[0].write(b"replaced");
        assert_eq!(archive.entries[0].crc, crc32(b"replaced"));
        assert_eq!(archive.entries[0].read().unwrap(), b"replaced");
    }

    #[test]
    fn a_stored_entry_stays_stored() {
        // OpenDocument requires the mimetype entry to be uncompressed and
        // first, so its content type can be read as magic bytes.
        let zip = build(&[("mimetype", b"application/vnd.oasis.opendocument.text")], b"", b"");
        let mut archive = Archive::read(&zip, &mut report()).unwrap();
        archive.entries[0].write(b"application/vnd.oasis.opendocument.text");
        assert_eq!(archive.entries[0].method, METHOD_STORE);

        let out = archive.write();
        let at = out.windows(4).position(|w| w == LOCAL_SIG).unwrap();
        assert_eq!(u16::from_le_bytes([out[at + 8], out[at + 9]]), METHOD_STORE);
    }

    #[test]
    fn an_encrypted_archive_is_refused_rather_than_half_processed() {
        let mut zip = build(&[("a.txt", b"hi")], b"", b"");
        let cd = zip.windows(4).position(|w| w == CENTRAL_SIG).unwrap();
        zip[cd + 8] = FLAG_ENCRYPTED as u8;
        assert!(matches!(Archive::read(&zip, &mut report()), Err(Error::Encrypted(_))));
    }

    #[test]
    fn zip64_is_declined_rather_than_misparsed() {
        let mut zip = build(&[("a.txt", b"hi")], b"", b"");
        let eocd = find_eocd(&zip).unwrap();
        zip[eocd + 12] = 0xFF; // central directory size = 0xFFFFFFFF
        zip[eocd + 13] = 0xFF;
        zip[eocd + 14] = 0xFF;
        zip[eocd + 15] = 0xFF;
        assert!(matches!(Archive::read(&zip, &mut report()), Err(Error::Unsupported { .. })));
    }

    #[test]
    fn an_eocd_signature_inside_a_comment_does_not_confuse_the_scan() {
        let mut zip = build(&[("a.txt", b"hi")], b"", b"");
        let comment: &[u8] = b"PK\x05\x06 this looks like a record but is not";
        let eocd = find_eocd(&zip).unwrap();
        zip[eocd + 20..eocd + 22].copy_from_slice(&(comment.len() as u16).to_le_bytes());
        zip.extend_from_slice(comment);

        assert_eq!(find_eocd(&zip), Some(eocd));
        let mut rep = report();
        assert_eq!(Archive::read(&zip, &mut rep).unwrap().entries.len(), 1);
        assert!(rep.removed.iter().any(|r| r.location == "archive comment"));
    }

    #[test]
    fn malformed_archives_are_errors_not_panics() {
        let mut rep = report();
        assert!(Archive::read(b"", &mut rep).is_err());
        assert!(Archive::read(b"PK\x05\x06", &mut rep).is_err());

        let full = build(&[("a.txt", b"hello"), ("b.txt", b"world")], b"UT\x01\x00\x02", b"c");
        for n in 0..full.len() {
            let _ = Archive::read(&full[..n], &mut report());
        }
        // Every single-byte corruption must also be survivable.
        for i in 0..full.len() {
            let mut bad = full.clone();
            bad[i] ^= 0xFF;
            let _ = Archive::read(&bad, &mut report());
        }
    }

    #[test]
    fn an_unsupported_compression_method_is_reported_not_guessed_at() {
        let entry = Entry {
            name: b"x".to_vec(),
            method: 14, // LZMA
            crc: 0,
            stored: vec![1, 2, 3],
            size: 3,
            utf8: false,
        };
        assert!(matches!(entry.read(), Err(Error::Unsupported { .. })));
    }
}
