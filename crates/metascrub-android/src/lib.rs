//! The Android side of the boundary for metascrub: bytes in, cleaned bytes (or
//! a report) out.
//!
//! ## Why this crate exists at all
//!
//! Every other crate in this workspace is `#![forbid(unsafe_code)]`. JNI cannot
//! be: the VM hands over raw pointers, and reading a byte array means calling
//! back into it. Rather than weaken the rule where the parsers live, the
//! unsafety is confined here, in the smallest crate that can hold it — the same
//! treatment `tox-sys` and `tox-android` get in the sibling messenger.
//!
//! This crate hands cleaned bytes back, hands a report back, and — for images
//! only, and only when asked — hands back a copy softened to reduce its sensor
//! fingerprint (`pixelwash`). Three narrow jobs; it should not grow a fourth.
//!
//! ## Nothing is logged
//!
//! There is no logger here on purpose. On a phone a log line is readable by
//! `adb logcat` and by a bug report, and the file being cleaned is the private
//! thing. The desktop makes the same choice for the same reason.

use jni::objects::{JByteArray, JClass};
use jni::sys::{jboolean, jbyteArray, jint, jstring};
use jni::JNIEnv;
use metascrub::{Assurance, ColorProfile, Orientation, Policy, Report};
use pixelwash::{Settings, Strength};
use zeroize::Zeroize;

/// Build a policy from the two "keep more than the safe minimum" toggles the
/// interface offers. Everything else stays at the safe default: the point of the
/// tool is that anything not explicitly kept is dropped.
fn build_policy(keep_colour: jboolean, keep_orientation: jboolean) -> Policy {
    let mut policy = Policy::default();
    if keep_colour != 0 {
        policy.color_profile = ColorProfile::Keep;
    }
    if keep_orientation != 0 {
        policy.orientation = Orientation::PreserveMinimal;
    }
    policy
}

/// Clean `input` and return the sanitized bytes.
///
/// Throws a Java `RuntimeException` — so the Activity shows a failure rather
/// than a silent empty result — if the file is in a format metascrub claims to
/// handle but could not parse. A format it cannot take apart at all comes back
/// as the input unchanged; the Activity reads `assurance == none` from
/// [`Java_org_crake_metascrub_Native_reportJson`] and warns instead of
/// pretending the file was cleaned.
///
/// # Safety
///
/// Called by the VM through `System.loadLibrary`, with a valid `JNIEnv` for the
/// calling thread. The name binds it to `org.crake.metascrub.Native.sanitize`;
/// changing either without the other is an `UnsatisfiedLinkError` at run time.
#[no_mangle]
pub extern "system" fn Java_org_crake_metascrub_Native_sanitize<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    input: JByteArray<'local>,
    keep_colour: jboolean,
    keep_orientation: jboolean,
) -> jbyteArray {
    let mut bytes = match env.convert_byte_array(&input) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };
    let result = match metascrub::sanitize(&bytes, &build_policy(keep_colour, keep_orientation)) {
        Ok(out) => env
            .byte_array_from_slice(&out.data)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", e.to_string());
            std::ptr::null_mut()
        }
    };
    bytes.zeroize();
    result
}

/// Inspect `input` and return a JSON report for the interface to render.
///
/// The report carries the assurance, the format, whether a location was found,
/// the input/output sizes, and the removed / retained / warning lists. It never
/// contains a metadata *value* — only categories, structural locations and
/// counts — because the core `Report` is built that way.
///
/// # Safety
///
/// As [`Java_org_crake_metascrub_Native_sanitize`].
#[no_mangle]
pub extern "system" fn Java_org_crake_metascrub_Native_reportJson<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    input: JByteArray<'local>,
    keep_colour: jboolean,
    keep_orientation: jboolean,
) -> jstring {
    let mut bytes = match env.convert_byte_array(&input) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };
    let json = match metascrub::inspect(&bytes, &build_policy(keep_colour, keep_orientation)) {
        Ok(report) => report_to_json(&report),
        Err(e) => format!("{{\"error\":{}}}", json_string(&e.to_string())),
    };
    bytes.zeroize();
    env.new_string(json).map(|s| s.into_raw()).unwrap_or(std::ptr::null_mut())
}

/// Reduce the sensor fingerprint of a photograph and return the re-encoded JPEG.
///
/// This is a different operation from sanitizing: it reaches into the *pixels*
/// (denoise, downscale, add noise, re-encode) to weaken PRNU correlation. It
/// **reduces** linkability and does not remove the fingerprint; the interface
/// built on it says so. Throws a Java `RuntimeException` if the bytes are not an
/// image this can decode. `strength` is 0 gentle, 1 balanced, 2 thorough.
///
/// # Safety
///
/// As [`Java_org_crake_metascrub_Native_sanitize`].
#[no_mangle]
pub extern "system" fn Java_org_crake_metascrub_Native_reduceFingerprint<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    input: JByteArray<'local>,
    strength: jint,
) -> jbyteArray {
    let mut bytes = match env.convert_byte_array(&input) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };
    let settings = Settings {
        strength: match strength {
            0 => Strength::Gentle,
            2 => Strength::Thorough,
            _ => Strength::Balanced,
        },
        // On a phone the whole wash pipeline (full-resolution denoise, then a
        // Lanczos resize, then the noise pass) peaks at several times the decoded
        // RGBA buffer, so the desktop default of 120 MP can drive well over a
        // gigabyte of transient allocation and OOM the process. Cap well below
        // that: an image past this is refused cleanly (TooLarge) instead of
        // crashing. 50 MP still covers the full-resolution output of essentially
        // every phone camera (high-count sensors bin down to far less by default).
        max_megapixels: Some(50),
    };
    let result = match pixelwash::wash(&bytes, &settings) {
        Ok(washed) => env
            .byte_array_from_slice(&washed.data)
            .map(|a| a.into_raw())
            .unwrap_or(std::ptr::null_mut()),
        Err(e) => {
            let _ = env.throw_new("java/lang/RuntimeException", e.to_string());
            std::ptr::null_mut()
        }
    };
    bytes.zeroize();
    result
}

/// A JSON string literal, escaped. Hand-rolled so the crate carries no
/// serialization dependency, matching the desktop CLI.
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
