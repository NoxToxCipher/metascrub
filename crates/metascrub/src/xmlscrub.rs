//! Targeted edits to XML parts, done on bytes rather than through a parser.
//!
//! The obvious approach is to parse the XML, change what needs changing, and
//! serialize it back. For office documents that is the riskier choice: a
//! round trip through any parser renormalizes namespace prefixes, entity
//! escaping, self-closing tags and whitespace, and Word is particular about
//! parts of its own format in ways that are tedious to rediscover. A document
//! that no longer opens is a worse outcome than one that still says who edited
//! it, because the user will simply send the original instead.
//!
//! So the transformations here are surgical. They locate a specific attribute
//! inside a specific tag and either blank its value or remove the attribute
//! outright, leaving every other byte of the part exactly as it was.
//!
//! **Why this is safe on bytes.** An XML attribute value delimited by double
//! quotes cannot itself contain a raw double quote (it has to be `&quot;`), and
//! character data outside a tag cannot contain a raw `<`. Tracking whether the
//! cursor is inside a tag, a comment or a CDATA section is therefore enough to
//! know that a match is a real attribute and not text that looks like one.
//!
//! **The honest limit.** A producer that writes attribute values in single
//! quotes will not be matched. Every OOXML and OpenDocument writer in practice
//! uses double quotes, and a missed match leaves a name in the document rather
//! than corrupting it, so the failure is visible in the report rather than
//! silent. Parts that are wholly replaced (the property files, which is where
//! the bulk of document metadata lives) do not depend on any of this.

/// What to do with a matched attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    /// Replace the value with a neutral placeholder, keeping the attribute.
    /// Used where the schema requires the attribute to be present.
    Blank(&'static str),
    /// Remove the attribute entirely.
    Remove,
}

/// A rule, matched against an attribute's **local** name, so it applies
/// whatever namespace prefix the producer used. Word writes `w:author`,
/// `w15:author` and `w16cid:author` for the same idea.
pub(crate) struct Rule {
    /// Local name to match, or a prefix of one when `prefix_match` is set.
    pub name: &'static str,
    /// Match any local name starting with `name` rather than equal to it.
    pub prefix_match: bool,
    pub action: Action,
}

/// The rules applied to every XML part of a document.
///
/// Authors are blanked rather than removed because the elements that carry
/// them (tracked insertions, comments) require the attribute. Dates and
/// revision-save identifiers are optional in the schema, so they go entirely.
pub(crate) const DOCUMENT_RULES: &[Rule] = &[
    Rule { name: "author", prefix_match: false, action: Action::Blank("author") },
    Rule { name: "initials", prefix_match: false, action: Action::Blank("a") },
    Rule { name: "creator", prefix_match: false, action: Action::Blank("author") },
    Rule { name: "lastModifiedBy", prefix_match: false, action: Action::Blank("author") },
    // Identity of the account that made a change, in the comment and
    // co-authoring parts. These are stable across documents.
    Rule { name: "userId", prefix_match: false, action: Action::Remove },
    Rule { name: "providerId", prefix_match: false, action: Action::Remove },
    // When an edit was made.
    Rule { name: "date", prefix_match: false, action: Action::Remove },
    Rule { name: "dateUtc", prefix_match: false, action: Action::Remove },
    // Revision-save identifiers. Word stamps these on every run and paragraph;
    // two documents edited in the same session share them, which links files to
    // each other and to one machine.
    Rule { name: "rsid", prefix_match: true, action: Action::Remove },
];

/// Result of a scrub pass.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Counts {
    pub blanked: usize,
    pub removed: usize,
}

impl Counts {
    pub(crate) fn any(&self) -> bool {
        self.blanked + self.removed > 0
    }
}

/// Apply `rules` to every start tag in `xml`.
pub(crate) fn scrub_attributes(xml: &[u8], rules: &[Rule]) -> (Vec<u8>, Counts) {
    let mut out = Vec::with_capacity(xml.len());
    let mut counts = Counts::default();
    let mut i = 0;

    while i < xml.len() {
        if xml[i] != b'<' {
            out.push(xml[i]);
            i += 1;
            continue;
        }
        // Comments, CDATA, declarations and processing instructions are copied
        // whole: they hold no attributes we act on, and their content rules are
        // different enough that scanning them as a tag would be wrong.
        if let Some(end) = skip_verbatim(xml, i) {
            out.extend_from_slice(&xml[i..end]);
            i = end;
            continue;
        }
        let Some(tag_end) = find_tag_end(xml, i) else {
            out.extend_from_slice(&xml[i..]);
            break;
        };
        rewrite_tag(&xml[i..tag_end], rules, &mut out, &mut counts);
        i = tag_end;
    }
    (out, counts)
}

/// If `start` opens a comment, CDATA section, doctype or processing
/// instruction, return the offset just past its end.
fn skip_verbatim(xml: &[u8], start: usize) -> Option<usize> {
    let rest = &xml[start..];
    let (opener, closer): (&[u8], &[u8]) = if rest.starts_with(b"<!--") {
        (b"<!--", b"-->")
    } else if rest.starts_with(b"<![CDATA[") {
        (b"<![CDATA[", b"]]>")
    } else if rest.starts_with(b"<?") || rest.starts_with(b"<!") {
        (b"<?", b">")
    } else {
        return None;
    };
    let from = opener.len();
    let found = rest[from..]
        .windows(closer.len())
        .position(|w| w == closer)
        .map(|p| start + from + p + closer.len());
    // An unterminated construct means the rest of the file is inside it.
    Some(found.unwrap_or(xml.len()))
}

/// Find the `>` that closes the tag beginning at `start`, skipping any inside
/// quoted attribute values.
fn find_tag_end(xml: &[u8], start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    for (offset, &byte) in xml[start..].iter().enumerate() {
        match (quote, byte) {
            (Some(q), b) if b == q => quote = None,
            (Some(_), _) => {}
            (None, b'"') | (None, b'\'') => quote = Some(byte),
            (None, b'>') => return Some(start + offset + 1),
            (None, _) => {}
        }
    }
    None
}

/// Rewrite one complete tag, applying the rules to its attributes.
fn rewrite_tag(tag: &[u8], rules: &[Rule], out: &mut Vec<u8>, counts: &mut Counts) {
    // Copy the element name and anything before the first space.
    let name_end = tag
        .iter()
        .position(|&b| b.is_ascii_whitespace())
        .unwrap_or(tag.len());
    out.extend_from_slice(&tag[..name_end]);

    let mut i = name_end;
    while i < tag.len() {
        if tag[i].is_ascii_whitespace() {
            // Hold the whitespace back: if the attribute that follows is
            // removed, its separator goes with it rather than piling up.
            let ws_start = i;
            while i < tag.len() && tag[i].is_ascii_whitespace() {
                i += 1;
            }
            match parse_attribute(tag, i) {
                Some(attr) => {
                    match rule_for(rules, attr.local_name(tag)) {
                        Some(Action::Remove) => counts.removed += 1,
                        Some(Action::Blank(placeholder)) => {
                            counts.blanked += 1;
                            out.extend_from_slice(&tag[ws_start..attr.value_start]);
                            out.extend_from_slice(placeholder.as_bytes());
                            out.extend_from_slice(&tag[attr.value_end..attr.end]);
                        }
                        None => out.extend_from_slice(&tag[ws_start..attr.end]),
                    }
                    i = attr.end;
                }
                // Not an attribute: the tail of the tag, such as `/>`.
                None => {
                    out.extend_from_slice(&tag[ws_start..]);
                    return;
                }
            }
        } else {
            out.push(tag[i]);
            i += 1;
        }
    }
}

/// One `name="value"` pair.
struct Attribute {
    name_start: usize,
    name_end: usize,
    value_start: usize,
    value_end: usize,
    /// Just past the closing quote.
    end: usize,
}

impl Attribute {
    /// The part of the name after any namespace prefix.
    fn local_name<'a>(&self, tag: &'a [u8]) -> &'a [u8] {
        let name = &tag[self.name_start..self.name_end];
        match name.iter().rposition(|&b| b == b':') {
            Some(colon) => &name[colon + 1..],
            None => name,
        }
    }
}

fn parse_attribute(tag: &[u8], start: usize) -> Option<Attribute> {
    let mut i = start;
    while i < tag.len() && !matches!(tag[i], b'=' | b'/' | b'>') && !tag[i].is_ascii_whitespace() {
        i += 1;
    }
    if i == start || tag.get(i) != Some(&b'=') {
        return None;
    }
    let name_end = i;
    i += 1;
    let quote = *tag.get(i)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    i += 1;
    let value_start = i;
    while i < tag.len() && tag[i] != quote {
        i += 1;
    }
    if i >= tag.len() {
        return None;
    }
    Some(Attribute { name_start: start, name_end, value_start, value_end: i, end: i + 1 })
}

fn rule_for(rules: &[Rule], local: &[u8]) -> Option<Action> {
    rules
        .iter()
        .find(|r| {
            if r.prefix_match {
                local.len() >= r.name.len() && local[..r.name.len()].eq_ignore_ascii_case(r.name.as_bytes())
            } else {
                local.eq_ignore_ascii_case(r.name.as_bytes())
            }
        })
        .map(|r| r.action)
}

/// Remove whole elements by local name, including their children.
///
/// Used for `w:rsids`, the block in `word/settings.xml` listing every
/// revision-save identifier the document has ever carried.
pub(crate) fn remove_elements(xml: &[u8], local_names: &[&str]) -> (Vec<u8>, usize) {
    let mut out = Vec::with_capacity(xml.len());
    let mut removed = 0;
    let mut i = 0;

    while i < xml.len() {
        if xml[i] != b'<' {
            out.push(xml[i]);
            i += 1;
            continue;
        }
        if let Some(end) = skip_verbatim(xml, i) {
            out.extend_from_slice(&xml[i..end]);
            i = end;
            continue;
        }
        let Some(tag_end) = find_tag_end(xml, i) else {
            out.extend_from_slice(&xml[i..]);
            break;
        };
        let tag = &xml[i..tag_end];

        match element_name(tag).filter(|n| local_names.iter().any(|w| n.eq_ignore_ascii_case(w.as_bytes()))) {
            Some(name) => {
                removed += 1;
                if tag.ends_with(b"/>") {
                    i = tag_end;
                } else {
                    i = skip_to_close(xml, tag_end, name);
                }
            }
            None => {
                out.extend_from_slice(tag);
                i = tag_end;
            }
        }
    }
    (out, removed)
}

/// The local name of a start tag, or `None` for an end tag.
fn element_name(tag: &[u8]) -> Option<&[u8]> {
    let body = tag.strip_prefix(b"<")?;
    if body.starts_with(b"/") {
        return None;
    }
    let end = body
        .iter()
        .position(|&b| b.is_ascii_whitespace() || b == b'>' || b == b'/')
        .unwrap_or(body.len());
    let name = &body[..end];
    Some(match name.iter().rposition(|&b| b == b':') {
        Some(colon) => &name[colon + 1..],
        None => name,
    })
}

/// Scan past the matching end tag for `name`, counting nested occurrences.
fn skip_to_close(xml: &[u8], from: usize, name: &[u8]) -> usize {
    let mut depth = 1usize;
    let mut i = from;
    while i < xml.len() {
        if xml[i] != b'<' {
            i += 1;
            continue;
        }
        if let Some(end) = skip_verbatim(xml, i) {
            i = end;
            continue;
        }
        let Some(tag_end) = find_tag_end(xml, i) else { return xml.len() };
        let tag = &xml[i..tag_end];

        if let Some(open) = element_name(tag) {
            if open == name && !tag.ends_with(b"/>") {
                depth += 1;
            }
        } else if closing_name(tag) == Some(name) {
            depth -= 1;
            if depth == 0 {
                return tag_end;
            }
        }
        i = tag_end;
    }
    xml.len()
}

fn closing_name(tag: &[u8]) -> Option<&[u8]> {
    let body = tag.strip_prefix(b"</")?;
    let end = body
        .iter()
        .position(|&b| b.is_ascii_whitespace() || b == b'>')
        .unwrap_or(body.len());
    let name = &body[..end];
    Some(match name.iter().rposition(|&b| b == b':') {
        Some(colon) => &name[colon + 1..],
        None => name,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrub(xml: &str) -> (String, Counts) {
        let (out, counts) = scrub_attributes(xml.as_bytes(), DOCUMENT_RULES);
        (String::from_utf8(out).unwrap(), counts)
    }

    #[test]
    fn tracked_change_authors_are_blanked_and_dates_removed() {
        let (out, counts) = scrub(
            r#"<w:ins w:id="1" w:author="Jane Q. Reviewer" w:date="2026-03-04T10:00:00Z"><w:r/></w:ins>"#,
        );
        assert_eq!(out, r#"<w:ins w:id="1" w:author="author"><w:r/></w:ins>"#);
        assert_eq!(counts, Counts { blanked: 1, removed: 1 });
    }

    #[test]
    fn any_namespace_prefix_matches_because_word_uses_several() {
        let (out, _) = scrub(r#"<w15:person w15:author="Someone Real" w16cid:initials="SR"/>"#);
        assert!(!out.contains("Someone Real"));
        assert!(out.contains(r#"w15:author="author""#));
        assert!(out.contains(r#"w16cid:initials="a""#));
    }

    #[test]
    fn revision_save_ids_are_removed_along_with_their_separator() {
        let (out, counts) = scrub(
            r#"<w:p w:rsidR="00A12B34" w:rsidRDefault="00A12B34" w:rsidP="00C56D78"><w:r/></w:p>"#,
        );
        assert_eq!(out, "<w:p><w:r/></w:p>", "leftover whitespace would pile up");
        assert_eq!(counts.removed, 3);
    }

    #[test]
    fn attributes_we_have_no_rule_for_are_left_exactly_alone() {
        let src = r#"<w:tbl w:id="7"  w:val="keep   me"><w:tr/></w:tbl>"#;
        let (out, counts) = scrub(src);
        assert_eq!(out, src);
        assert!(!counts.any());
    }

    #[test]
    fn a_self_closing_tag_keeps_its_slash() {
        let (out, _) = scrub(r#"<w:comment w:author="Real Name" w:id="1"/>"#);
        assert_eq!(out, r#"<w:comment w:author="author" w:id="1"/>"#);

        let (out, _) = scrub(r#"<w:comment w:rsidR="00AA" />"#);
        assert_eq!(out, "<w:comment />");
    }

    #[test]
    fn text_that_looks_like_an_attribute_is_not_treated_as_one() {
        // Character data cannot contain a raw '<', so this is unambiguous.
        let src = r#"<w:t>the string w:author="not an attribute" appears here</w:t>"#;
        let (out, counts) = scrub(src);
        assert_eq!(out, src);
        assert!(!counts.any());
    }

    #[test]
    fn comments_cdata_and_declarations_pass_through_untouched() {
        let src = concat!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>"#,
            r#"<!-- w:author="in a comment" -->"#,
            r#"<r><![CDATA[ w:rsidR="in cdata" ]]></r>"#,
        );
        let (out, counts) = scrub(src);
        assert_eq!(out, src);
        assert!(!counts.any());
    }

    #[test]
    fn a_greater_than_inside_an_attribute_value_does_not_end_the_tag() {
        let src = r#"<w:t w:val="a > b" w:author="Someone"/>"#;
        let (out, _) = scrub(src);
        assert_eq!(out, r#"<w:t w:val="a > b" w:author="author"/>"#);
    }

    #[test]
    fn single_quoted_values_are_matched_too() {
        let (out, counts) = scrub("<w:ins w:author='Jane' w:id='2'/>");
        assert_eq!(out, "<w:ins w:author='author' w:id='2'/>");
        assert_eq!(counts.blanked, 1);
    }

    #[test]
    fn whole_elements_can_be_removed_with_their_children() {
        let src = concat!(
            "<w:settings><w:zoom w:percent=\"100\"/>",
            "<w:rsids><w:rsidRoot w:val=\"00A12B34\"/><w:rsid w:val=\"00C56D78\"/></w:rsids>",
            "<w:defaultTabStop w:val=\"720\"/></w:settings>",
        );
        let (out, removed) = remove_elements(src.as_bytes(), &["rsids"]);
        let out = String::from_utf8(out).unwrap();
        assert_eq!(removed, 1);
        assert!(!out.contains("00A12B34") && !out.contains("00C56D78"));
        assert!(out.contains("w:zoom") && out.contains("defaultTabStop"));
    }

    #[test]
    fn a_self_closing_element_is_removed_without_eating_the_rest() {
        let (out, removed) = remove_elements(b"<a/><rsids/><b/>", &["rsids"]);
        assert_eq!(String::from_utf8(out).unwrap(), "<a/><b/>");
        assert_eq!(removed, 1);
    }

    #[test]
    fn nested_elements_of_the_same_name_are_matched_at_the_right_depth() {
        let src = "<keep><x><x>inner</x></x></keep><keep2/>";
        let (out, removed) = remove_elements(src.as_bytes(), &["x"]);
        assert_eq!(String::from_utf8(out).unwrap(), "<keep></keep><keep2/>");
        assert_eq!(removed, 1);
    }

    #[test]
    fn malformed_xml_is_copied_through_rather_than_mangled() {
        for src in [
            "<w:ins w:author=\"unterminated",
            "<unclosed",
            "<!-- never closed",
            "<a><![CDATA[ unterminated",
            "",
            "no tags at all",
            "<<<>>>",
        ] {
            let (out, _) = scrub_attributes(src.as_bytes(), DOCUMENT_RULES);
            assert!(
                String::from_utf8_lossy(&out).len() >= src.len().saturating_sub(64),
                "input {src:?} lost too much"
            );
            let _ = remove_elements(src.as_bytes(), &["rsids"]);
        }
    }

    #[test]
    fn truncation_at_every_offset_never_panics() {
        let src = concat!(
            r#"<?xml version="1.0"?><w:document><w:ins w:author="A" w:date="2026-01-01">"#,
            r#"<w:r w:rsidR="00AA"><w:t>hi</w:t></w:r></w:ins><w:rsids><w:rsid w:val="1"/>"#,
            "</w:rsids></w:document>",
        );
        for n in 0..src.len() {
            let _ = scrub_attributes(&src.as_bytes()[..n], DOCUMENT_RULES);
            let _ = remove_elements(&src.as_bytes()[..n], &["rsids"]);
        }
    }
}
