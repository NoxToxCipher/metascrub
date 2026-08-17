//! A plain C ABI over the metascrub core: bytes in, cleaned bytes (or a JSON
//! report) out. This is the boundary a native C/C++ host links against — the
//! Sailfish/Silica app, and any future GTK or Qt front end.
//!
//! ## Why this crate is allowed `unsafe`
//!
//! Every crate in this workspace is `#![forbid(unsafe_code)]` except the ones
//! that touch a foreign runtime. On Android that is the JNI crate; here it is the
//! C ABI. The same rule applies: keep the unsafety in the smallest crate that can
//! hold it, and keep the parsers — where a hostile file is actually read — safe.
//! This crate does no parsing of its own; it only marshals pointers to and from
//! the core, which stays memory-safe.
//!
//! ## The contract
//!
//! Three jobs, mirroring the Android bridge exactly so every front end shows the
//! same thing: clean a file ([`ms_sanitize`]), report what a file carries without
//! rebuilding it ([`ms_report_json`]), and soften a photo's sensor fingerprint
//! ([`ms_reduce_fingerprint`], which **reduces** and does not remove). The report
//! JSON never contains a metadata *value* — only categories, structural locations
//! and counts — because the core `Report` is built that way.
//!
//! ## Ownership
//!
//! Every buffer this crate returns is owned by the caller and must be freed with
//! the matching function ([`ms_buffer_free`] for bytes, [`ms_string_free`] for
//! JSON) and nothing else. The *input* pointer is always borrowed, never freed or
//! retained past the call; the host owns the file bytes and should wipe them
//! itself once done (this crate never copies them into an owned buffer, so there
//! is nothing here to wipe).
//!
//! ## Nothing is logged
//!
//! As on Android and the desktop: no logger, on purpose. The file being cleaned
//! is the private thing, and a log line is the last place it should surface.

use metascrub::{ColorProfile, Orientation, Policy, Report};
use pixelwash::{PngSettings, Settings, Strength};
use std::os::raw::c_char;

/// An owned byte buffer handed across the boundary. On failure `data` is null and
/// `len` is 0. Free it with [`ms_buffer_free`] and nothing else.
#[repr(C)]
pub struct MsBuffer {
    pub data: *mut u8,
    pub len: usize,
}

impl MsBuffer {
    /// Hand a `Vec` to the caller. `into_boxed_slice` makes the allocation exactly
    /// `len` bytes, so [`ms_buffer_free`] can reconstruct it precisely — reusing
    /// the `Vec`'s own (possibly larger) capacity would be undefined behaviour to
    /// free as if it were `len`.
    fn from_vec(v: Vec<u8>) -> MsBuffer {
        let boxed = v.into_boxed_slice();
        let len = boxed.len();
        let data = Box::into_raw(boxed) as *mut u8;
        MsBuffer { data, len }
    }

    fn null() -> MsBuffer {
        MsBuffer { data: std::ptr::null_mut(), len: 0 }
    }
}

/// Build a policy from the two "keep more than the safe minimum" toggles the
/// interface offers. Everything else stays at the safe default: anything not
/// explicitly kept is dropped. Identical to the Android bridge's `build_policy`.
fn build_policy(keep_colour: bool, keep_orientation: bool) -> Policy {
    let mut policy = Policy::default();
    if keep_colour {
        policy.color_profile = ColorProfile::Keep;
    }
    if keep_orientation {
        policy.orientation = Orientation::PreserveMinimal;
    }
    policy
}

/// Borrow `data`/`len` as a slice, or `None` if the pointer is null.
///
/// # Safety
/// `data` must either be null or point to `len` readable bytes that outlive the
/// returned slice.
unsafe fn as_slice<'a>(data: *const u8, len: usize) -> Option<&'a [u8]> {
    if data.is_null() {
        None
    } else {
        Some(std::slice::from_raw_parts(data, len))
    }
}

/// Clean `input` and return the sanitized bytes.
///
/// On error — a null pointer, or a format metascrub claims to handle but could
/// not parse — the returned buffer's `data` is null. A format the core cannot
/// take apart at all comes back as the input unchanged; the host reads
/// `assurance == "none"` from [`ms_report_json`] and warns rather than pretending
/// the file was cleaned.
///
/// # Safety
/// `input` must be null or point to `len` readable bytes. Free the result with
/// [`ms_buffer_free`].
#[no_mangle]
pub unsafe extern "C" fn ms_sanitize(
    input: *const u8,
    len: usize,
    keep_colour: bool,
    keep_orientation: bool,
) -> MsBuffer {
    let Some(bytes) = as_slice(input, len) else {
        return MsBuffer::null();
    };
    match metascrub::sanitize(bytes, &build_policy(keep_colour, keep_orientation)) {
        Ok(out) => MsBuffer::from_vec(out.data),
        Err(_) => MsBuffer::null(),
    }
}

/// Inspect `input` and return a JSON report, without rebuilding the file. The
/// shape matches the Android bridge exactly. On any error the JSON is
/// `{"error":"..."}`. Never null unless allocation of the string itself fails.
///
/// # Safety
/// `input` must be null or point to `len` readable bytes. Free the result with
/// [`ms_string_free`].
#[no_mangle]
pub unsafe extern "C" fn ms_report_json(
    input: *const u8,
    len: usize,
    keep_colour: bool,
    keep_orientation: bool,
) -> *mut c_char {
    let Some(bytes) = as_slice(input, len) else {
        return cstring(String::from("{\"error\":\"null input\"}"));
    };
    let json = match metascrub::inspect(bytes, &build_policy(keep_colour, keep_orientation)) {
        Ok(report) => report_to_json(&report),
        Err(e) => format!("{{\"error\":{}}}", json_string(&e.to_string())),
    };
    cstring(json)
}

/// Reduce the sensor fingerprint of a photograph and return the re-encoded JPEG.
///
/// A different operation from sanitizing: it reaches into the *pixels* (denoise,
/// downscale, add noise, re-encode) to weaken PRNU correlation. It **reduces**
/// linkability and does not remove the fingerprint. `strength` is 0 gentle, 1
/// balanced, 2 thorough. On error (not an image this can decode, or too large)
/// the buffer's `data` is null. The host should still run the result through
/// [`ms_sanitize`] so no metadata rides the re-encoded JPEG — the same two-step
/// the Android app does.
///
/// # Safety
/// `input` must be null or point to `len` readable bytes. Free the result with
/// [`ms_buffer_free`].
#[no_mangle]
pub unsafe extern "C" fn ms_reduce_fingerprint(
    input: *const u8,
    len: usize,
    strength: i32,
) -> MsBuffer {
    let Some(bytes) = as_slice(input, len) else {
        return MsBuffer::null();
    };
    let settings = Settings {
        strength: match strength {
            0 => Strength::Gentle,
            2 => Strength::Thorough,
            _ => Strength::Balanced,
        },
        // The wash pipeline peaks at several times the decoded buffer, so cap it
        // well below the desktop default; a phone-sized ceiling refuses an
        // oversize image cleanly instead of running the process out of memory.
        // Matches the Android bridge.
        max_megapixels: Some(50),
    };
    match pixelwash::wash(bytes, &settings) {
        Ok(washed) => MsBuffer::from_vec(washed.data),
        Err(_) => MsBuffer::null(),
    }
}

/// Convert a photo to a metadata-free PNG, re-encoded from raw pixels.
///
/// The "render path": a host takes a JPEG (or PNG/WebP) a user picked, and gets
/// back a PNG it can display or store — a Crake avatar, say — that carries none of
/// the source's metadata. It drops everything by rebuilding from decoded pixels,
/// preserves any alpha channel, rotates the image upright per the source's EXIF
/// orientation before that flag is dropped, and optionally downscales so the
/// longest edge is at most `max_edge` (pass 0 to keep the size). A small image is
/// never enlarged.
///
/// `max_bytes` caps the encoded PNG: when non-zero, the image is shrunk until the
/// result fits that many bytes, so a host can ask for "any image, as a PNG no
/// larger than N bytes" (Crake passes 65536 for a 64 KB avatar) without resizing
/// the image itself. Pass 0 to disable the budget. If even a small render cannot
/// fit an absurdly small budget, `data` is null.
///
/// This is a format conversion plus a metadata scrub. It is **not** fingerprint
/// reduction and must never be presented as such — for that, use
/// [`ms_reduce_fingerprint`]. The PNG is additionally run through the sanitizer so
/// the guarantee is the same honest allowlist rebuild the rest of the core gives.
///
/// On error (not a decodable image, or larger than the phone-sized cap) `data` is
/// null.
///
/// # Safety
/// `input` must be null or point to `len` readable bytes. Free the result with
/// [`ms_buffer_free`].
#[no_mangle]
pub unsafe extern "C" fn ms_to_png(
    input: *const u8,
    len: usize,
    max_edge: u32,
    max_bytes: usize,
) -> MsBuffer {
    let Some(bytes) = as_slice(input, len) else {
        return MsBuffer::null();
    };
    let settings = PngSettings {
        max_edge: if max_edge == 0 { None } else { Some(max_edge) },
        max_bytes: if max_bytes == 0 { None } else { Some(max_bytes) },
        // Same phone-sized decode ceiling as the wash path, so an oversize image is
        // refused cleanly instead of running the process out of memory.
        max_megapixels: Some(50),
    };
    let png = match pixelwash::to_png(bytes, &settings) {
        Ok(png) => png,
        Err(_) => return MsBuffer::null(),
    };
    // Belt and braces: the PNG is already built from raw pixels, but rebuilding it
    // through the sanitizer's allowlist means the render path makes exactly the
    // same honest guarantee as every other output of the core. The PNG sanitizer
    // only drops chunks and copies the kept ones verbatim, so it can only shrink
    // the file — a byte budget met by `to_png` is still met after this.
    match metascrub::sanitize(&png, &Policy::default()) {
        Ok(out) => MsBuffer::from_vec(out.data),
        Err(_) => MsBuffer::null(),
    }
}

/// Free a buffer returned by [`ms_sanitize`] or [`ms_reduce_fingerprint`].
/// Passing a null-`data` buffer is a no-op. Never call this on a buffer twice.
///
/// # Safety
/// `buf` must be a value returned by this library and not already freed.
#[no_mangle]
pub unsafe extern "C" fn ms_buffer_free(buf: MsBuffer) {
    if !buf.data.is_null() {
        // Reconstruct the exact boxed slice `from_vec` produced and drop it.
        let slice = std::slice::from_raw_parts_mut(buf.data, buf.len);
        drop(Box::from_raw(slice as *mut [u8]));
    }
}

/// Free a JSON string returned by [`ms_report_json`]. Null is a no-op.
///
/// # Safety
/// `s` must be a value returned by this library and not already freed.
#[no_mangle]
pub unsafe extern "C" fn ms_string_free(s: *mut c_char) {
    if !s.is_null() {
        drop(std::ffi::CString::from_raw(s));
    }
}

/// Move a `String` into a C string the caller owns. Null only if the string
/// contains an interior NUL, which the report serializer never emits.
fn cstring(s: String) -> *mut c_char {
    match std::ffi::CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

// --- report serialization -------------------------------------------------
//
// Hand-rolled so the crate carries no serialization dependency, and byte-for-byte
// the same shape as the Android bridge so both front ends parse one contract.
// NOTE: this duplicates `report_to_json`/`json_string` in `metascrub-android`.
// The clean consolidation is a `Report::to_json()` on the core that both bridges
// call; kept separate for now so this scaffold touches neither the core nor the
// audited Android crate.

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn report_to_json(r: &Report) -> String {
    use metascrub::Assurance;
    use std::fmt::Write;
    let assurance = match r.assurance {
        Assurance::Complete => "complete",
        Assurance::BestEffort => "best_effort",
        Assurance::None => "none",
    };
    let mut s = String::from("{");
    let _ = write!(s, "\"assurance\":{}", json_string(assurance));
    let _ = write!(s, ",\"format\":{}", json_string(&r.format.to_string()));
    let _ = write!(s, ",\"found_location\":{}", r.found_location);
    let _ = write!(s, ",\"input_bytes\":{}", r.input_len);
    let _ = write!(s, ",\"output_bytes\":{}", r.output_len);

    s.push_str(",\"removed\":[");
    for (i, item) in r.removed.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"kind\":{},\"location\":{},\"bytes\":{}}}",
            json_string(&item.kind.to_string()),
            json_string(&item.location),
            item.bytes,
        );
    }

    s.push_str("],\"retained\":[");
    for (i, ret) in r.retained.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        let _ = write!(
            s,
            "{{\"what\":{},\"reveals\":{}}}",
            json_string(&ret.what),
            json_string(&ret.reveals),
        );
    }

    s.push_str("],\"warnings\":[");
    for (i, w) in r.warnings.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        s.push_str(&json_string(w));
    }
    s.push_str("]}");
    s
}
