//! Desktop interface for `metascrub`.
//!
//! The whole product is one claim: what this says about a file is true. So the
//! interface is built around the [`Assurance`] level rather than around a
//! progress bar. Three outcomes have to look different at a glance:
//!
//! - **Complete** — the container was rebuilt from a keep-list, nothing outside
//!   it survived.
//! - **Best effort** — known metadata was removed but the container was not
//!   rebuilt, and the warnings say what was not guaranteed.
//! - **Not cleaned** — the format could not be taken apart. No output is
//!   written, because a file named "clean" that was never cleaned is worse than
//!   no file at all.
//!
//! State is shown with a shape as well as a colour, since roughly one man in
//! twelve cannot reliably tell the green from the amber (WCAG 1.4.1).
#![forbid(unsafe_code)]
// The console window is noise for a desktop app, but keep it in debug builds so
// panics and logging remain visible while developing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use zeroize::{Zeroize, Zeroizing};

use eframe::egui;
use egui::{Color32, FontFamily, FontId, RichText, Stroke};
use metascrub::{Assurance, ColorProfile, Orientation, Policy, Sanitized};
use pixelwash::{Strength, WashReport};

mod i18n;
mod privacy;
mod reference;

// ---------------------------------------------------------------------------
// Palette, carried over from design/scrubber-mockup.html
// ---------------------------------------------------------------------------

mod theme {
    use egui::Color32;

    pub const GROUND: Color32 = Color32::from_rgb(0x0f, 0x14, 0x18);
    pub const PANEL: Color32 = Color32::from_rgb(0x17, 0x1d, 0x22);
    pub const PANEL2: Color32 = Color32::from_rgb(0x1d, 0x25, 0x2b);
    pub const LINE: Color32 = Color32::from_rgb(0x2a, 0x34, 0x3b);
    pub const INK: Color32 = Color32::from_rgb(0xdd, 0xe4, 0xe8);
    pub const INK_DIM: Color32 = Color32::from_rgb(0x93, 0xa3, 0xac);
    pub const INK_FAINT: Color32 = Color32::from_rgb(0x66, 0x75, 0x7e);
    pub const ACCENT: Color32 = Color32::from_rgb(0x58, 0xa6, 0xb0);
    pub const ACCENT_DIM: Color32 = Color32::from_rgb(0x2f, 0x5a, 0x60);
    pub const OK: Color32 = Color32::from_rgb(0x74, 0xa9, 0x7b);
    pub const WARN: Color32 = Color32::from_rgb(0xd9, 0x97, 0x3f);
    pub const DANGER: Color32 = Color32::from_rgb(0xc4, 0x58, 0x4b);
}

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
}

/// Add a Myanmar (Burmese) font as a fallback for both text families.
///
/// egui's default fonts (Ubuntu-Light, Hack) cover Latin and Cyrillic — so
/// English, Russian and Latin render — but not the Myanmar script, so without
/// this the entire Burmese interface renders as empty boxes. Padauk (SIL, OFL)
/// is appended as a *fallback*: Latin and Cyrillic keep the default typeface,
/// and only code points the default fonts lack fall through to Padauk. It is
/// registered for the monospace family too, because the status badges and the
/// finding rows draw in it.
fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};

    // Pinned provenance — see fonts/PROVENANCE.md. Padauk (SIL, OFL-1.1),
    // SHA-256 c89cf56e572abda9652d9e54203bd729b0c59541c4b569046b9b61acd0b532f3.
    // This is a third-party binary that `cargo deny` cannot see. The length is
    // checked at compile time so a swapped or truncated font of a different size
    // fails the build; the SHA-256 in PROVENANCE.md is the authoritative value
    // (re-verify it in CI).
    const PADAUK: &[u8] = include_bytes!("../fonts/Padauk-Regular.ttf");
    const _: () = assert!(
        PADAUK.len() == 498_860,
        "bundled Padauk font changed size — re-check fonts/PROVENANCE.md and the SHA-256"
    );

    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert("padauk".to_owned(), std::sync::Arc::new(FontData::from_static(PADAUK)));
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        fonts.families.entry(family).or_default().push("padauk".to_owned());
    }
    ctx.set_fonts(fonts);
}

/// Draw the Crake mark — a small teal bird — inside `rect`, from the suite's
/// `icon.svg` (viewBox 15,16 66x68): body + head circles, a beak triangle, and
/// an eye. Drawn from primitives, so no image/SVG dependency is pulled in.
fn draw_crake(painter: &egui::Painter, rect: egui::Rect, tint: Color32, eye: Color32) {
    let sx = rect.width() / 66.0;
    let sy = rect.height() / 68.0;
    let map =
        |x: f32, y: f32| egui::pos2(rect.left() + (x - 15.0) * sx, rect.top() + (y - 16.0) * sy);
    let s = (sx + sy) / 2.0; // near-uniform radius scale
    painter.circle_filled(map(52.0, 56.0), 24.0 * s, tint); // body
    painter.circle_filled(map(50.0, 34.0), 13.0 * s, tint); // head
    painter.add(egui::Shape::convex_polygon(
        vec![map(38.0, 29.0), map(38.0, 43.0), map(20.0, 35.0)], // beak
        tint,
        Stroke::NONE,
    ));
    painter.circle_filled(map(53.0, 31.0), 3.6 * s, eye); // eye (a background dot, like the SVG cut-out)
}

/// Draw the Crake mark as a small brand watermark in a corner. Placed bottom-left
/// because the bottom-right corner holds the Save button.
fn draw_crake_mark(ctx: &egui::Context, area: egui::Rect) {
    let size = 24.0;
    let margin = 12.0;
    let rect = egui::Rect::from_min_size(
        egui::pos2(area.left() + margin, area.bottom() - size - margin),
        egui::vec2(size, size),
    );
    let painter =
        ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("crake_mark")));
    draw_crake(&painter, rect, theme::ACCENT, theme::GROUND);
}

// ---------------------------------------------------------------------------
// One file's outcome
// ---------------------------------------------------------------------------

struct Entry {
    path: PathBuf,
    result: Result<Sanitized, String>,
    /// Present when pixel washing ran. Reported separately from the metadata
    /// findings because it is a weaker, statistical claim and must never be
    /// read as part of the same guarantee.
    wash: Option<Result<WashReport, String>>,
    /// Where the cleaned copy went, once saved.
    saved_to: Option<PathBuf>,
    /// A stable random name for this file, generated once when the entry lands.
    /// Kept on the entry so the card can show the exact name the file will be
    /// saved under, and so it does not change on every repaint or every save.
    random_stem: String,
}

impl Entry {
    fn assurance(&self) -> Option<Assurance> {
        self.result.as_ref().ok().map(|s| s.report.assurance)
    }

    /// A file is worth writing only if we actually rebuilt it.
    fn is_writable(&self) -> bool {
        matches!(self.assurance(), Some(Assurance::Complete | Assurance::BestEffort))
            && self.saved_to.is_none()
    }

    /// The extension the saved copy should carry. Pixel washing always
    /// re-encodes to JPEG, so a washed PNG/WebP/TIFF/GIF must be saved as `.jpg`
    /// — writing JPEG bytes into a `.png` name would produce a file that will
    /// not open. Otherwise keep the original file's extension.
    fn output_ext(&self) -> Option<String> {
        if matches!(self.wash, Some(Ok(_))) {
            Some("jpg".to_string())
        } else {
            self.path.extension().map(|e| e.to_string_lossy().into_owned())
        }
    }
}

/// Work happens off the UI thread so a large PDF cannot freeze the window. The
/// first field is the generation the work was started under; a result whose
/// generation is stale (a setting changed while it was in flight) is discarded.
enum Job {
    Done(u64, PathBuf, Result<Sanitized, String>, Option<Result<WashReport, String>>),
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct App {
    entries: Vec<Entry>,
    policy: Policy,
    /// Receives results from the worker pool. The matching senders live in the
    /// pool workers (see `worker_loop`), which keep the channel open.
    rx: Receiver<Job>,
    /// Sends files to the bounded worker pool.
    job_tx: Sender<WorkItem>,
    pending: usize,
    /// Set when a save fails, so the failure is visible rather than silent.
    error: Option<String>,

    /// Pixel washing is off until asked for: it degrades the photograph, and a
    /// protection that costs something should be a decision, not a default.
    wash_enabled: bool,
    wash_strength: Strength,
    /// Shown the first time washing is switched on, before any file is touched.
    intro_open: bool,
    /// Whether the pixel-washing explanation has been shown this session. Kept
    /// in memory, not on disk, so the tool leaves no "was used here" trace.
    intro_seen: bool,
    /// The reference panel, explaining every removal and the PRNU work.
    reference_open: bool,
    /// Handbook search query; empty shows everything.
    ref_query: String,
    /// Handbook category filter; `None` shows all categories.
    ref_category: Option<RefCategory>,
    /// Interface language for the core screen.
    lang: i18n::Lang,
    /// Whether to give each cleaned copy a random name. The file name is
    /// metadata too (dates, places, a camera prefix), so this is on by default —
    /// stripping the name is the more protective choice. Unchecked keeps the
    /// original name with a `.clean` suffix.
    randomize_name: bool,

    /// Bumped whenever the working set is re-processed (a settings toggle, or a
    /// clear). A worker result tagged with an older generation is discarded, so
    /// toggling a setting while a large file is still in flight cannot leave a
    /// stale, old-policy finding on screen.
    generation: u64,
    /// Paths with a worker running, so a re-run knows to reprocess a file that
    /// has not landed yet, and a second drop of the same file is not queued twice.
    in_flight: Vec<PathBuf>,
}

/// The sections of the handbook, used for the category filter and search.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RefCategory {
    FileTypes,
    Metadata,
    Raw,
    Fingerprint,
    BeyondFile,
    Myths,
    Evidence,
    Letter,
}

impl RefCategory {
    const ALL: [RefCategory; 8] = [
        RefCategory::FileTypes,
        RefCategory::Metadata,
        RefCategory::Raw,
        RefCategory::Fingerprint,
        RefCategory::BeyondFile,
        RefCategory::Myths,
        RefCategory::Evidence,
        RefCategory::Letter,
    ];
    fn label(self, lang: i18n::Lang) -> &'static str {
        let t = i18n::T::for_lang(lang);
        match self {
            RefCategory::FileTypes => t.cat_files,
            RefCategory::Metadata => t.cat_metadata,
            RefCategory::Raw => t.cat_raw,
            RefCategory::Fingerprint => t.cat_fingerprint,
            RefCategory::BeyondFile => t.cat_beyond,
            RefCategory::Myths => t.cat_myths,
            RefCategory::Evidence => t.cat_evidence,
            RefCategory::Letter => t.cat_letter,
        }
    }
}

/// True if the (already-lowercased) query is empty or appears in any of the
/// given text fields. The search that powers the handbook.
fn handbook_hit(query: &str, fields: &[&str]) -> bool {
    query.is_empty() || fields.iter().any(|f| f.to_lowercase().contains(query))
}

/// One file handed to the worker pool. Cheap to buffer (no file bytes), so a
/// large drop queues these rather than spawning a thread each.
struct WorkItem {
    path: PathBuf,
    policy: Policy,
    wash: Option<Strength>,
    lang: i18n::Lang,
    generation: u64,
}

/// A pool worker: pull one job at a time, process it, post the result. Bounding
/// the number of these bounds peak memory — only `pool size` files are read and
/// parsed at once, instead of one 2 GB-capable thread per dropped file, which a
/// folder of thousands of files would otherwise spawn all at once.
fn worker_loop(jobs: Arc<Mutex<Receiver<WorkItem>>>, results: Sender<Job>) {
    loop {
        // Hold the lock only for the receive; processing runs unlocked so the
        // other workers proceed in parallel.
        let item = match jobs.lock() {
            Ok(rx) => rx.recv(),
            Err(_) => return, // a worker panicked holding the lock; give up
        };
        let Ok(item) = item else { return }; // channel closed: app is exiting
        let (mut result, wash_report) = process(&item.path, &item.policy, item.wash, item.lang);
        if let Ok(s) = &mut result {
            if s.report.assurance == Assurance::None {
                s.data.zeroize();
            }
        }
        if results.send(Job::Done(item.generation, item.path, result, wash_report)).is_err() {
            return; // UI gone
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Wipe the cleaned bytes still held in the entry list on exit.
        self.wipe_entries();
    }
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = channel();
        // Bounded worker pool: a handful of long-lived threads draining a job
        // queue, instead of one thread per file.
        let (job_tx, job_rx) = channel::<WorkItem>();
        let job_rx = Arc::new(Mutex::new(job_rx));
        let workers =
            std::thread::available_parallelism().map(|n| n.get()).unwrap_or(2).clamp(1, 4);
        for _ in 0..workers {
            let jobs = Arc::clone(&job_rx);
            let results = tx.clone();
            std::thread::spawn(move || worker_loop(jobs, results));
        }
        // `tx` is not stored: the pool workers each hold a clone, which keeps the
        // results channel open, and nothing else sends results.
        drop(tx);
        Self {
            job_tx,
            entries: Vec::new(),
            policy: Policy { max_input_bytes: Some(MAX_FILE_BYTES), ..Policy::default() },
            rx,
            pending: 0,
            error: None,
            wash_enabled: false,
            wash_strength: Strength::default(),
            intro_open: false,
            intro_seen: false,
            reference_open: false,
            ref_query: String::new(),
            ref_category: None,
            lang: i18n::Lang::En,
            randomize_name: true,
            generation: 0,
            in_flight: Vec::new(),
        }
    }
}

// The pixel-washing explanation is shown once per session, tracked in memory
// (`App::intro_seen`), not with a file on disk. An earlier version wrote a
// marker to %APPDATA%\metascrub\, but that directory's mere existence records
// that the tool was run on this machine — the same class of trace privacy.rs
// exists to remove. With this gone, metascrub writes no persistent files at all.

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        cc.egui_ctx.all_styles_mut(|style| {
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = theme::GROUND;
            style.visuals.window_fill = theme::PANEL;
            style.visuals.extreme_bg_color = theme::PANEL2;
            style.visuals.override_text_color = Some(theme::INK);
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme::LINE);

            // Every interactive widget needs its own outline. Setting only the
            // noninteractive one left unticked checkboxes drawing egui's default
            // near-black stroke on a near-black panel, so the box was invisible
            // until hovered and the setting looked like plain text.
            let outline = Stroke::new(1.0, theme::LINE);
            let outline_lit = Stroke::new(1.0, theme::ACCENT);
            style.visuals.widgets.inactive.bg_fill = theme::PANEL2;
            style.visuals.widgets.inactive.weak_bg_fill = theme::PANEL2;
            style.visuals.widgets.inactive.bg_stroke = outline;
            style.visuals.widgets.hovered.bg_fill = theme::LINE;
            style.visuals.widgets.hovered.weak_bg_fill = theme::LINE;
            style.visuals.widgets.hovered.bg_stroke = outline_lit;
            style.visuals.widgets.active.bg_fill = theme::LINE;
            style.visuals.widgets.active.weak_bg_fill = theme::LINE;
            style.visuals.widgets.active.bg_stroke = outline_lit;
            // The tick itself, and any glyph drawn inside a widget.
            style.visuals.widgets.inactive.fg_stroke = Stroke::new(1.6, theme::INK);
            style.visuals.widgets.hovered.fg_stroke = Stroke::new(1.8, theme::INK);
            style.visuals.widgets.active.fg_stroke = Stroke::new(1.8, theme::ACCENT);
            style.visuals.selection.bg_fill = theme::ACCENT_DIM;
            style.visuals.selection.stroke = Stroke::new(1.0, theme::ACCENT);

            style.spacing.item_spacing = egui::vec2(9.0, 9.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
            style.spacing.window_margin = egui::Margin::same(14);
        });

        Self::default()
    }

    fn queue(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            // Skip a path that is already shown or already has a worker running,
            // so dropping the same file twice before its first result lands does
            // not spawn two workers and push two entries for it.
            if self.entries.iter().any(|e| e.path == path) || self.in_flight.contains(&path) {
                continue;
            }
            self.pending += 1;
            self.in_flight.push(path.clone());
            // Hand the file to the bounded pool rather than spawning a thread per
            // file, so a folder of thousands of files cannot spawn thousands of
            // threads each holding up to a 2 GB buffer. (The worker wipes the
            // original for un-cleanable files; see `worker_loop`.)
            let sent = self.job_tx.send(WorkItem {
                path: path.clone(),
                policy: self.policy.clone(),
                wash: self.wash_enabled.then_some(self.wash_strength),
                lang: self.lang,
                generation: self.generation,
            });
            if sent.is_err() {
                // Only reachable if the whole pool has died. Undo the accounting
                // so a lost job cannot pin `pending` (which would spin the
                // repaint loop forever waiting for a result that never comes).
                self.pending = self.pending.saturating_sub(1);
                self.in_flight.retain(|p| p != &path);
            }
        }
    }

    /// Zeroize the cleaned bytes held in the entry list. A Complete output is
    /// low-sensitivity, but a BestEffort output still carries the *retained*
    /// metadata the UI warns about (a raw's kept maker note, for instance), so
    /// none of it should linger in freed heap after a Clear, a re-run, or exit.
    fn wipe_entries(&mut self) {
        for e in &mut self.entries {
            if let Ok(s) = &mut e.result {
                s.data.zeroize();
            }
        }
    }

    fn drain(&mut self) {
        while let Ok(Job::Done(generation, path, result, wash)) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            if generation != self.generation {
                // A result from before the last settings change or clear. Its
                // path was already re-queued (its live worker holds the current
                // in_flight slot), so drop this and leave in_flight untouched.
                continue;
            }
            self.in_flight.retain(|p| p != &path);
            if self.entries.iter().any(|e| e.path == path) {
                continue; // guard against a duplicate landing
            }
            self.entries.push(Entry {
                path,
                result,
                wash,
                saved_to: None,
                random_stem: metascrub::random_stem(24),
            });
        }
    }

    /// Re-run everything against the current policy, so toggling a setting
    /// updates the findings rather than leaving stale ones on screen. Files
    /// still in flight are re-queued too (not just the completed ones), and the
    /// generation bump makes their old-policy results be discarded on arrival.
    fn rerun(&mut self) {
        let mut paths: Vec<PathBuf> = self.entries.iter().map(|e| e.path.clone()).collect();
        paths.extend(self.in_flight.iter().cloned());
        self.wipe_entries();
        self.entries.clear();
        self.in_flight.clear();
        self.generation += 1;
        self.queue(paths);
    }

    fn save_all(&mut self) {
        self.error = None;
        let randomize = self.randomize_name;
        for entry in self.entries.iter_mut().filter(|e| e.is_writable()) {
            let ext = entry.output_ext();
            let Ok(sanitized) = &entry.result else { continue };
            let stem = randomize.then_some(entry.random_stem.as_str());
            let dst = output_name(&entry.path, stem, ext.as_deref());
            match write_atomic(&dst, &sanitized.data) {
                Ok(()) => entry.saved_to = Some(dst),
                Err(e) => {
                    self.error =
                        Some(format!("{} {}: {e}", self.tr().could_not_write, dst.display()));
                    break;
                }
            }
        }
    }

    fn counts(&self) -> (usize, usize, usize) {
        let mut complete = 0;
        let mut partial = 0;
        let mut skipped = 0;
        for e in &self.entries {
            match e.assurance() {
                Some(Assurance::Complete) => complete += 1,
                Some(Assurance::BestEffort) => partial += 1,
                _ => skipped += 1,
            }
        }
        (complete, partial, skipped)
    }
}

/// Read a file, strip its metadata, and optionally wash its pixels.
///
/// The order matters. The **metadata report always describes the original**, so
/// the user is told what the file they hold actually carried. Washing then runs
/// on those same original bytes rather than on the stripped copy, because
/// washing decodes and re-encodes anyway and compressing twice would cost
/// quality for nothing. The washed result is finally passed back through the
/// sanitizer, since a fresh encode is only metadata-free if nothing put
/// anything back.
/// Largest file the interface will read into memory. Checked against the file's
/// size on disk *before* reading, so an enormous file is refused rather than
/// loaded. No photograph or document approaches this; the parsers allocate
/// roughly the input size again, so the real memory cost is a small multiple.
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024 * 1024;

fn process(
    path: &std::path::Path,
    policy: &Policy,
    wash: Option<Strength>,
    lang: i18n::Lang,
) -> (Result<Sanitized, String>, Option<Result<WashReport, String>>) {
    // Refuse an oversize file by its size on disk, before it is read. The
    // policy also carries this limit, but that check only runs after the bytes
    // are in memory, which is too late to prevent the allocation.
    match std::fs::metadata(path) {
        Ok(m) if m.len() > MAX_FILE_BYTES => {
            return (
                Err(format!(
                    "{}{:.1}{}{}{}",
                    i18n::T::for_lang(lang).size_a,
                    m.len() as f64 / 1e9,
                    i18n::T::for_lang(lang).size_b,
                    MAX_FILE_BYTES / 1_000_000_000,
                    i18n::T::for_lang(lang).size_c
                )),
                None,
            );
        }
        _ => {}
    }

    // Wiped when this returns. The buffer holds the original file, metadata and
    // all, so a freed heap page could otherwise keep someone's coordinates
    // readable to whatever allocates next, or to a memory dump.
    let bytes = match std::fs::read(path) {
        Ok(b) => Zeroizing::new(b),
        Err(e) => return (Err(e.to_string()), None),
    };

    // Every interactive clean checks its own output: re-scan for anything
    // removable that survived, and confirm the result is reproducible.
    let stripped = match metascrub::sanitize_verified(&bytes, policy) {
        Ok(s) => s,
        Err(e) => return (Err(e.to_string()), None),
    };

    let Some(strength) = wash else {
        return (Ok(stripped), None);
    };

    // Fingerprint reduction only makes sense for a camera-captured raster photo.
    // It re-encodes and downscales the pixels, so it cannot apply to a raw (that
    // would destroy the raw, and the raw is the strongest fingerprint carrier),
    // nor to vector, document or non-photo files (there is no sensor pattern).
    // Say so plainly instead of failing with a decode error.
    use metascrub::Format;
    let not_applicable = match metascrub::detect(&bytes) {
        Format::Jpeg | Format::Png | Format::WebP | Format::Tiff | Format::Gif => None,
        Format::Raw => Some(i18n::T::for_lang(lang).fp_raw.to_string()),
        Format::Heif | Format::Avif => Some(i18n::T::for_lang(lang).fp_heif.to_string()),
        _ => Some(i18n::T::for_lang(lang).fp_nonphoto.to_string()),
    };
    if let Some(reason) = not_applicable {
        return (Ok(stripped), Some(Err(reason)));
    }

    let settings = pixelwash::Settings { strength, ..Default::default() };
    match pixelwash::wash(&bytes, &settings) {
        Ok(washed) => {
            let wash_report = washed.report.clone();
            // The intermediate carries the full-resolution washed image.
            let washed_data = Zeroizing::new(washed.data);
            // Verify the bytes that actually get saved, not the pre-wash strip.
            // pixelwash always re-encodes to JPEG, so the saved artifact is a
            // fresh JPEG; its format, size and verification must describe *it*.
            match metascrub::sanitize_verified(&washed_data, policy) {
                Ok(final_pass) => {
                    // Keep the original file's findings (the GPS, the maker note,
                    // the thumbnail the user needs to know were there), but retag
                    // the format, size and verification to the washed JPEG that
                    // lands on disk. Otherwise a 40 MP -> 10 MP reduction looks
                    // like it barely changed the file, and the "verified" tick
                    // would refer to a different artifact than the one saved.
                    let mut report = stripped.report;
                    report.format = final_pass.report.format;
                    report.output_len = final_pass.data.len();
                    report.verification = final_pass.report.verification;
                    (Ok(Sanitized { data: final_pass.data, report }), Some(Ok(wash_report)))
                }
                Err(e) => (Ok(stripped), Some(Err(e.to_string()))),
            }
        }
        Err(e) => (Ok(stripped), Some(Err(e.to_string()))),
    }
}

/// `holiday.jpg` becomes `holiday.clean.jpg`. The original is never touched:
/// nothing is overwritten unless the user explicitly asks for it. `ext` is the
/// extension to give the copy (from [`Entry::output_ext`]), which differs from
/// the source when the image was washed to JPEG.
fn clean_name(path: &std::path::Path, ext: Option<&str>) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match ext {
        Some(ext) if !ext.is_empty() => format!("{stem}.clean.{ext}"),
        _ => format!("{stem}.clean"),
    };
    path.with_file_name(name)
}

/// The name a cleaned copy is saved under. `random_stem` is the file's
/// pre-generated random name (from the [`Entry`]) to use when random naming is
/// on; `None` keeps the original name with a `.clean` suffix. `ext` is the
/// extension the saved bytes call for (see [`Entry::output_ext`]), lower-cased
/// for the random form so the file still opens by double-click.
fn output_name(path: &std::path::Path, random_stem: Option<&str>, ext: Option<&str>) -> PathBuf {
    match random_stem {
        Some(stem) => {
            let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
            let name = match ext.map(|e| e.to_lowercase()) {
                Some(e) if !e.is_empty() => format!("{stem}.{e}"),
                _ => stem.to_string(),
            };
            dir.join(name)
        }
        None => clean_name(path, ext),
    }
}

/// Write cleaned bytes through the library's single hardened writer.
///
/// The interface used to carry its own copy of this, and the copy was weaker: a
/// predictable temporary name opened with `create`, which a local attacker can
/// pre-place as a symlink to make the write land somewhere else. The library's
/// version uses an unpredictable name and `create_new`, so there is one correct
/// implementation rather than two that can drift apart.
fn write_atomic(dst: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    metascrub::write_atomic(dst, data).map_err(|e| match e {
        metascrub::Error::Io(io) => io,
        other => std::io::Error::other(other.to_string()),
    })
}

fn human(bytes: usize) -> String {
    const UNITS: [&str; 4] = ["B", "kB", "MB", "GB"];
    let mut v = bytes as f64;
    let mut unit = 0;
    while v >= 1024.0 && unit < UNITS.len() - 1 {
        v /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[unit])
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let ctx = &ctx;
        let window_rect = ui.max_rect(); // full window, before panels consume it
        self.drain();
        if self.pending > 0 {
            ctx.request_repaint();
        }

        // Files dropped anywhere on the window.
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if !dropped.is_empty() {
            self.queue(dropped);
        }
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());

        self.top_bar(ui);
        self.bottom_bar(ui);
        self.intro_window(ctx);
        self.reference_window(ctx);
        draw_crake_mark(ctx, window_rect);

        egui::CentralPanel::default().show(ui, |ui| {
            if self.entries.is_empty() && self.pending == 0 {
                self.drop_zone(ui, hovering);
            } else {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for i in 0..self.entries.len() {
                        self.file_card(ui, i);
                        ui.add_space(4.0);
                    }
                    if self.pending > 0 {
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(
                                RichText::new(format!(
                                    "{}{}{}",
                                    self.tr().reading_pre,
                                    self.pending,
                                    self.tr().reading_post
                                ))
                                .color(theme::INK_DIM),
                            );
                        });
                    }
                });
            }
        });
    }
}

impl App {
    /// Current-language strings for the core screen.
    fn tr(&self) -> i18n::T {
        i18n::T::for_lang(self.lang)
    }

    fn strength_label(&self, s: Strength) -> &'static str {
        let t = self.tr();
        match s {
            Strength::Gentle => t.wash_gentle,
            Strength::Balanced => t.wash_balanced,
            Strength::Thorough => t.wash_thorough,
        }
    }

    fn strength_desc(&self, s: Strength) -> &'static str {
        let t = self.tr();
        match s {
            Strength::Gentle => t.wash_gentle_desc,
            Strength::Balanced => t.wash_balanced_desc,
            Strength::Thorough => t.wash_thorough_desc,
        }
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top")
            .frame(egui::Frame::default().fill(theme::PANEL2).inner_margin(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("metascrub").size(15.0).strong().color(theme::INK));
                    ui.label(
                        RichText::new(self.tr().tagline)
                            .size(12.0)
                            .color(theme::INK_FAINT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(self.tr().handbook).clicked() {
                            self.reference_open = true;
                        }
                        ui.separator();
                        for lang in [i18n::Lang::La, i18n::Lang::My, i18n::Lang::Ru, i18n::Lang::En] {
                            if ui
                                .selectable_label(self.lang == lang, lang.label())
                                .clicked()
                            {
                                self.lang = lang;
                            }
                        }
                        if self.lang == i18n::Lang::My {
                            ui.label(RichText::new("မူကြမ်း / draft").size(11.0).color(theme::WARN))
                                .on_hover_text(
                                    "Burmese is an unverified draft translation. The English text is authoritative until a native speaker has checked it.",
                                );
                        }
                    });
                });

                // All the metadata settings on their own row, left-to-right, so
                // the labels read before their controls and the brand + language
                // row above keeps its room (the tagline used to end up under the
                // Keep colour box). Keep-rotation and keep-colour change the
                // findings and rerun; the output name only renames the copy.
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    // Both keep *more* than the safe minimum, so the default
                    // state is the protective one.
                    let mut keep_rotation =
                        self.policy.orientation == Orientation::PreserveMinimal;
                    if ui
                        .checkbox(&mut keep_rotation, self.tr().keep_rotation)
                        .on_hover_text(self.tr().help_rotation)
                        .changed()
                    {
                        self.policy.orientation = if keep_rotation {
                            Orientation::PreserveMinimal
                        } else {
                            Orientation::Drop
                        };
                        self.rerun();
                    }

                    let mut keep_icc = self.policy.color_profile == ColorProfile::Keep;
                    if ui
                        .checkbox(&mut keep_icc, self.tr().keep_colour)
                        .on_hover_text(self.tr().help_colour)
                        .changed()
                    {
                        self.policy.color_profile =
                            if keep_icc { ColorProfile::Keep } else { ColorProfile::Drop };
                        self.rerun();
                    }

                    ui.separator();

                    // The file name is metadata too (dates, places, a camera
                    // prefix), so this is a privacy choice. A checkbox like the
                    // others, ON by default: for a privacy tool, stripping the
                    // name is the more protective option, and the exact random
                    // name is shown on each file's card. Unchecked keeps the
                    // original name with a `.clean` suffix.
                    let label = self.tr().out_name_random;
                    let hint = self.tr().help_out_name;
                    ui.checkbox(&mut self.randomize_name, label).on_hover_text(hint);
                });

                // The pixel work, on its own row, kept visually apart from the
                // metadata settings above because it is a different kind of
                // claim and must not be read as part of the same guarantee.
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let mut enabled = self.wash_enabled;
                    if ui
                        .checkbox(&mut enabled, self.tr().reduce_fingerprint)
                        .on_hover_text(self.tr().help_fingerprint)
                        .changed()
                    {
                        self.wash_enabled = enabled;
                        if enabled && !self.intro_seen {
                            self.intro_open = true;
                        } else {
                            self.rerun();
                        }
                    }

                    if self.wash_enabled {
                        let before = self.wash_strength;
                        egui::ComboBox::from_id_salt("wash_strength")
                            .selected_text(self.strength_label(self.wash_strength))
                            .show_ui(ui, |ui| {
                                for s in [Strength::Gentle, Strength::Balanced, Strength::Thorough]
                                {
                                    let label = self.strength_label(s);
                                    ui.selectable_value(&mut self.wash_strength, s, label)
                                        .on_hover_text(self.strength_desc(s));
                                }
                            });
                        if before != self.wash_strength {
                            self.rerun();
                        }
                        ui.label(
                            RichText::new(self.strength_desc(self.wash_strength))
                                .size(11.0)
                                .color(theme::INK_FAINT),
                        );
                        ui.label(
                            RichText::new(self.tr().best_effort_only).font(mono(10.0)).color(theme::WARN),
                        );
                    }
                });
            });
    }

    /// Shown once, before pixel washing touches anything.
    fn intro_window(&mut self, ctx: &egui::Context) {
        if !self.intro_open {
            return;
        }
        let mut open = true;
        egui::Window::new(self.tr().intro_title)
            .id(egui::Id::new("intro_window"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(RichText::new(self.tr().fp_title).size(16.0).strong().color(theme::INK));
                ui.add_space(8.0);
                ui.label(
                    RichText::new(reference::first_use(self.lang)).size(13.0).color(theme::INK_DIM),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(self.tr().i_understand).color(theme::GROUND).strong(),
                            )
                            .fill(theme::ACCENT),
                        )
                        .clicked()
                    {
                        self.intro_seen = true;
                        self.intro_open = false;
                        self.rerun();
                    }
                    if ui.button(self.tr().read_full).clicked() {
                        self.intro_seen = true;
                        self.intro_open = false;
                        self.reference_open = true;
                        self.rerun();
                    }
                    if ui.button(self.tr().cancel).clicked() {
                        self.wash_enabled = false;
                        self.intro_open = false;
                    }
                });
            });
        // Dismissing with the window's own close button counts as cancelling,
        // since the explanation was not acknowledged.
        if !open {
            self.wash_enabled = false;
            self.intro_open = false;
        }
    }

    /// The reference panel: what is removed, why, and the PRNU explainer.
    fn reference_window(&mut self, ctx: &egui::Context) {
        if !self.reference_open {
            return;
        }
        let mut open = self.reference_open;
        egui::Window::new(self.tr().handbook)
            .id(egui::Id::new("handbook_window"))
            .open(&mut open)
            .collapsible(false)
            .default_width(720.0)
            .default_height(620.0)
            .vscroll(false)
            .show(ctx, |ui| {
                // Cap the measure. Dragged to full width on a wide monitor, the
                // body text ran off the right edge, and lines that long are
                // miserable to read even when they fit. Wide enough that the
                // category chips sit on a single row.
                ui.set_max_width(700.0);
                ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);

                // Handbook toolbar: search across everything, plus category filter.
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(self.tr().search).font(mono(10.5)).color(theme::INK_FAINT),
                    );
                    ui.add(
                        egui::TextEdit::singleline(&mut self.ref_query)
                            .hint_text("gps, serial, thumbnail, raw, pdf\u{2026}")
                            .desired_width(300.0),
                    );
                    if !self.ref_query.is_empty() && ui.button(self.tr().clear).clicked() {
                        self.ref_query.clear();
                    }
                });
                ui.add_space(6.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.selectable_label(self.ref_category.is_none(), "All").clicked() {
                        self.ref_category = None;
                    }
                    for c in RefCategory::ALL {
                        if ui
                            .selectable_label(self.ref_category == Some(c), c.label(self.lang))
                            .clicked()
                        {
                            self.ref_category =
                                if self.ref_category == Some(c) { None } else { Some(c) };
                        }
                    }
                });
                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                let q = self.ref_query.trim().to_lowercase();
                let cat = self.ref_category;
                let show_cat = |c: RefCategory| cat.is_none_or(|s| s == c);
                let query_display = self.ref_query.clone();

                // The toolbar above stays pinned; only the entries scroll.
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    if show_cat(RefCategory::FileTypes) {
                        let items: Vec<_> = reference::file_types(self.lang)
                            .iter()
                            .filter(|ft| handbook_hit(&q, &[ft.name, ft.carries, ft.identifies]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_filetypes)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_filetypes)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for ft in items {
                                egui::Frame::default()
                                    .fill(theme::PANEL)
                                    .stroke(Stroke::new(1.0, theme::LINE))
                                    .corner_radius(6.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(ft.name)
                                                .size(13.5)
                                                .strong()
                                                .color(theme::ACCENT),
                                        );
                                        ui.add_space(3.0);
                                        ui.label(
                                            RichText::new(self.tr().hb_carries)
                                                .font(mono(9.5))
                                                .color(theme::INK_FAINT),
                                        );
                                        ui.label(
                                            RichText::new(ft.carries).size(12.5).color(theme::INK),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(self.tr().hb_identifies)
                                                .font(mono(9.5))
                                                .color(theme::WARN),
                                        );
                                        ui.label(
                                            RichText::new(ft.identifies)
                                                .size(12.5)
                                                .color(theme::INK_DIM),
                                        );
                                    });
                                ui.add_space(6.0);
                            }
                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::Metadata) {
                        let items: Vec<_> = reference::metadata(self.lang)
                            .iter()
                            .filter(|it| handbook_hit(&q, &[it.name, it.what, it.why]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_metadata)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_metadata)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for item in items {
                                egui::Frame::default()
                                    .fill(theme::PANEL)
                                    .stroke(Stroke::new(1.0, theme::LINE))
                                    .corner_radius(6.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        ui.label(
                                            RichText::new(item.name)
                                                .size(13.5)
                                                .strong()
                                                .color(theme::ACCENT),
                                        );
                                        ui.add_space(3.0);
                                        ui.label(
                                            RichText::new(item.what).size(12.5).color(theme::INK),
                                        );
                                        ui.add_space(4.0);
                                        ui.label(
                                            RichText::new(item.why)
                                                .size(12.5)
                                                .color(theme::INK_DIM),
                                        );
                                    });
                                ui.add_space(6.0);
                            }
                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::Raw) {
                        let items: Vec<_> = reference::raw(self.lang)
                            .iter()
                            .filter(|s| handbook_hit(&q, &[s.heading, s.body]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_raw)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_raw)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for section in items {
                                ui.label(
                                    RichText::new(section.heading)
                                        .size(14.0)
                                        .strong()
                                        .color(theme::WARN),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(section.body).size(12.5).color(theme::INK_DIM),
                                );
                                ui.add_space(14.0);
                            }
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::Fingerprint) {
                        let items: Vec<_> = reference::prnu(self.lang)
                            .iter()
                            .filter(|s| handbook_hit(&q, &[s.heading, s.body]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_fingerprint)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_fingerprint)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for section in items {
                                ui.label(
                                    RichText::new(section.heading)
                                        .size(14.0)
                                        .strong()
                                        .color(theme::ACCENT),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(section.body).size(12.5).color(theme::INK_DIM),
                                );
                                ui.add_space(14.0);
                            }
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::BeyondFile) {
                        let items: Vec<_> = reference::beyond_the_file(self.lang)
                            .iter()
                            .filter(|s| handbook_hit(&q, &[s.heading, s.body]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_cannot_reach)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_beyond)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for section in items {
                                ui.label(
                                    RichText::new(section.heading)
                                        .size(14.0)
                                        .strong()
                                        .color(theme::DANGER),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(section.body).size(12.5).color(theme::INK_DIM),
                                );
                                ui.add_space(14.0);
                            }
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::Myths) {
                        let items: Vec<_> = reference::myths(self.lang)
                            .iter()
                            .filter(|m| handbook_hit(&q, &[m.claim, m.reality]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_myths)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_myths)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for myth in items {
                                egui::Frame::default()
                                    .fill(theme::PANEL)
                                    .stroke(Stroke::new(1.0, theme::LINE))
                                    .corner_radius(6.0)
                                    .inner_margin(10.0)
                                    .show(ui, |ui| {
                                        ui.horizontal_top(|ui| {
                                            ui.label(
                                                RichText::new(self.tr().hb_claim)
                                                    .font(mono(9.5))
                                                    .color(theme::WARN),
                                            );
                                            ui.label(
                                                RichText::new(myth.claim)
                                                    .size(12.5)
                                                    .italics()
                                                    .color(theme::INK),
                                            );
                                        });
                                        ui.add_space(5.0);
                                        ui.horizontal_top(|ui| {
                                            ui.label(
                                                RichText::new(self.tr().hb_truth)
                                                    .font(mono(9.5))
                                                    .color(theme::OK),
                                            );
                                            ui.label(
                                                RichText::new(myth.reality)
                                                    .size(12.5)
                                                    .color(theme::INK_DIM),
                                            );
                                        });
                                    });
                                ui.add_space(6.0);
                            }
                            ui.add_space(16.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    if show_cat(RefCategory::Evidence) {
                        let items: Vec<_> = reference::evidence(self.lang)
                            .iter()
                            .filter(|s| handbook_hit(&q, &[s.heading, s.body]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_evidence)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.label(
                                RichText::new(self.tr().intro_evidence)
                                    .size(12.5)
                                    .color(theme::INK_FAINT),
                            );
                            ui.add_space(10.0);
                            for section in items {
                                ui.label(
                                    RichText::new(section.heading)
                                        .size(14.0)
                                        .strong()
                                        .color(theme::ACCENT),
                                );
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(section.body).size(12.5).color(theme::INK_DIM),
                                );
                                ui.add_space(14.0);
                            }
                            ui.add_space(8.0);
                            ui.label(
                            RichText::new(
                                "Sources: Luk\u{00E1}\u{0161}, Fridrich & Goljan, 'Digital Camera \
                                 Identification from Sensor Pattern Noise', IEEE Transactions on \
                                 Information Forensics and Security, 2006. Subsequent work on \
                                 robustness under downscaling, on counter-forensic resampling, and \
                                 on smartphone reliability.",
                            )
                            .size(11.0)
                            .color(theme::INK_FAINT),
                        );
                            ui.add_space(8.0);
                        }
                    }

                    if show_cat(RefCategory::Letter) {
                        let items: Vec<_> = reference::letter(self.lang)
                            .iter()
                            .filter(|s| handbook_hit(&q, &[s.heading, s.body]))
                            .collect();
                        if !items.is_empty() {
                            ui.label(
                                RichText::new(self.tr().hb_letter)
                                    .size(17.0)
                                    .strong()
                                    .color(theme::INK),
                            );
                            ui.add_space(10.0);
                            for section in items {
                                // The letter is one flowing piece; its heading is
                                // empty, so only the body is shown.
                                ui.label(
                                    RichText::new(section.body).size(13.0).color(theme::INK_DIM),
                                );
                                ui.add_space(14.0);
                            }
                            ui.add_space(6.0);
                            ui.separator();
                            ui.add_space(12.0);
                        }
                    }

                    // Nothing matched the search across any category.
                    if !q.is_empty()
                        && reference::FILE_TYPES
                            .iter()
                            .all(|ft| !handbook_hit(&q, &[ft.name, ft.carries, ft.identifies]))
                        && reference::METADATA
                            .iter()
                            .all(|it| !handbook_hit(&q, &[it.name, it.what, it.why]))
                        && reference::RAW.iter().all(|s| !handbook_hit(&q, &[s.heading, s.body]))
                        && reference::PRNU.iter().all(|s| !handbook_hit(&q, &[s.heading, s.body]))
                        && reference::BEYOND_THE_FILE
                            .iter()
                            .all(|s| !handbook_hit(&q, &[s.heading, s.body]))
                        && reference::MYTHS.iter().all(|m| !handbook_hit(&q, &[m.claim, m.reality]))
                        && reference::EVIDENCE
                            .iter()
                            .all(|s| !handbook_hit(&q, &[s.heading, s.body]))
                        && reference::LETTER.iter().all(|s| !handbook_hit(&q, &[s.heading, s.body]))
                    {
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new(format!(
                                "{} \u{201C}{}\u{201D}.",
                                self.tr().no_match,
                                query_display
                            ))
                            .size(13.0)
                            .color(theme::INK_FAINT),
                        );
                        ui.label(
                            RichText::new(self.tr().try_plainer).size(12.0).color(theme::INK_FAINT),
                        );
                    }
                });
            });
        self.reference_open = open;
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("bottom")
            .frame(egui::Frame::default().fill(theme::PANEL2).inner_margin(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let (complete, partial, skipped) = self.counts();
                    if !self.entries.is_empty() {
                        ui.label(
                            RichText::new(format!("{complete} {}", self.tr().r_complete))
                                .font(mono(12.0))
                                .color(theme::OK),
                        );
                        ui.label(RichText::new("·").color(theme::INK_FAINT));
                        ui.label(
                            RichText::new(format!("{partial} {}", self.tr().r_best_effort))
                                .font(mono(12.0))
                                .color(theme::WARN),
                        );
                        if skipped > 0 {
                            ui.label(RichText::new("·").color(theme::INK_FAINT));
                            ui.label(
                                RichText::new(format!("{skipped} {}", self.tr().r_skipped))
                                    .font(mono(12.0))
                                    .color(theme::DANGER),
                            );
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let writable = self.entries.iter().filter(|e| e.is_writable()).count();
                        if ui
                            .add_enabled(
                                writable > 0,
                                egui::Button::new(
                                    RichText::new(format!(
                                        "{} ({writable})",
                                        self.tr().save_cleaned
                                    ))
                                    .color(theme::GROUND)
                                    .strong(),
                                )
                                .fill(theme::ACCENT),
                            )
                            .clicked()
                        {
                            self.save_all();
                        }
                        if ui
                            .add_enabled(
                                !self.entries.is_empty(),
                                egui::Button::new(self.tr().clear_list),
                            )
                            .clicked()
                        {
                            self.wipe_entries();
                            self.entries.clear();
                            // Discard any in-flight results too, so a worker that
                            // lands after Clear does not repopulate the list.
                            self.in_flight.clear();
                            self.generation += 1;
                            self.error = None;
                        }
                        if ui.button(self.tr().add_files).clicked() {
                            if let Some(files) = rfd::FileDialog::new().pick_files() {
                                privacy::forget_recent(&files);
                                self.queue(files);
                            }
                        }
                    });
                });

                if let Some(err) = &self.error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("! {err}")).size(12.0).color(theme::DANGER));
                }

                // Said once something has been saved, because "cleaned" reads as
                // "dealt with", and the untouched original is still sitting in
                // the same folder carrying everything that was just removed.
                if self.entries.iter().any(|e| e.saved_to.is_some()) {
                    ui.add_space(3.0);
                    ui.label(RichText::new(self.tr().untouched).size(11.5).color(theme::WARN));
                }
            });
    }

    fn drop_zone(&mut self, ui: &mut egui::Ui, hovering: bool) {
        let border = if hovering { theme::ACCENT } else { theme::LINE };
        egui::Frame::default()
            .stroke(Stroke::new(2.0, border))
            .corner_radius(10.0)
            .inner_margin(40.0)
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(
                        RichText::new(self.tr().drop_here).size(20.0).strong().color(theme::INK),
                    );
                    ui.add_space(6.0);
                    ui.label(RichText::new(self.tr().drop_sub).size(13.0).color(theme::INK_FAINT));
                    ui.add_space(16.0);
                    if ui.button(self.tr().choose_files).clicked() {
                        if let Some(files) = rfd::FileDialog::new().pick_files() {
                            privacy::forget_recent(&files);
                            self.queue(files);
                        }
                    }
                    ui.add_space(40.0);
                });
            });
    }

    fn file_card(&mut self, ui: &mut egui::Ui, index: usize) {
        let mut save_as: Option<usize> = None;
        let entry = &self.entries[index];
        let name = entry
            .path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| entry.path.display().to_string());

        egui::Frame::default()
            .fill(theme::PANEL)
            .stroke(Stroke::new(1.0, theme::LINE))
            .corner_radius(8.0)
            .inner_margin(12.0)
            .show(ui, |ui| {
                match &entry.result {
                    Err(e) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&name).strong().color(theme::INK));
                            badge(ui, self.tr().badge_could_not_read, theme::DANGER, Mark::Cross);
                        });
                        ui.label(RichText::new(e).size(12.0).color(theme::INK_DIM));
                    }
                    Ok(s) => {
                        let report = &s.report;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&name).strong().color(theme::INK));
                            match report.assurance {
                                Assurance::Complete => {
                                    badge(ui, self.tr().badge_complete, theme::OK, Mark::Disc)
                                }
                                Assurance::BestEffort => badge(
                                    ui,
                                    self.tr().badge_best_effort,
                                    theme::WARN,
                                    Mark::Triangle,
                                ),
                                Assurance::None => badge(
                                    ui,
                                    self.tr().badge_not_cleaned,
                                    theme::DANGER,
                                    Mark::Cross,
                                ),
                            }
                        });

                        ui.label(
                            RichText::new(format!(
                                "{}  {} -> {}",
                                report.format,
                                human(report.input_len),
                                human(report.output_len)
                            ))
                            .font(mono(11.0))
                            .color(theme::INK_FAINT),
                        );

                        // The finding most likely to change someone's mind.
                        if report.found_location {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(self.tr().recorded_location)
                                    .size(12.5)
                                    .strong()
                                    .color(theme::DANGER),
                            );
                        }

                        // "Nothing was found" and "nothing was reported" look
                        // identical if the list is simply absent, and the second
                        // reads as a broken tool. Say which it was.
                        if report.removed.is_empty() {
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(self.tr().no_metadata)
                                    .size(12.0)
                                    .color(theme::INK_FAINT),
                            );
                        }

                        if !report.removed.is_empty() {
                            ui.add_space(6.0);
                            for item in &report.removed {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new("\u{00D7}").font(mono(12.0)).color(theme::OK),
                                    );
                                    ui.label(
                                        RichText::new(i18n::kind_label(item.kind, self.lang))
                                            .size(12.5)
                                            .color(theme::INK),
                                    );
                                    ui.label(
                                        RichText::new(&item.location)
                                            .font(mono(10.5))
                                            .color(theme::INK_FAINT),
                                    );
                                    if item.bytes > 0 {
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                ui.label(
                                                    RichText::new(human(item.bytes))
                                                        .font(mono(10.5))
                                                        .color(theme::INK_FAINT),
                                                );
                                            },
                                        );
                                    }
                                });
                            }
                        }

                        // The tool checking its own output, shown plainly so the
                        // guarantee is visible rather than implied.
                        if let Some(v) = report.verification {
                            ui.add_space(6.0);
                            let (mark, colour, text) = if v.passed() {
                                ("\u{2713}", theme::OK, self.tr().verify_ok.to_string())
                            } else if !v.output_reinspected_clean {
                                ("\u{2717}", theme::DANGER, self.tr().verify_fail_meta.to_string())
                            } else {
                                (
                                    "\u{2717}",
                                    theme::DANGER,
                                    self.tr().verify_fail_determinism.to_string(),
                                )
                            };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(mark).font(mono(12.0)).color(colour));
                                ui.label(RichText::new(text).size(12.0).color(colour));
                            });
                        }

                        // What could not be removed, and what it would reveal.
                        // Framed prominently: a partial clean that stays quiet
                        // about its residue is worse than one that spells it out.
                        if !report.retained.is_empty() {
                            ui.add_space(8.0);
                            egui::Frame::default()
                                .fill(theme::PANEL)
                                .stroke(Stroke::new(1.0, theme::WARN))
                                .corner_radius(6.0)
                                .inner_margin(10.0)
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new(self.tr().still_in_file)
                                            .size(13.0)
                                            .strong()
                                            .color(theme::WARN),
                                    );
                                    ui.label(
                                        RichText::new(self.tr().retained_explain)
                                            .size(11.5)
                                            .color(theme::INK_FAINT),
                                    );
                                    for r in &report.retained {
                                        ui.add_space(6.0);
                                        ui.label(
                                            RichText::new(format!("\u{2022} {}", r.what))
                                                .size(12.5)
                                                .color(theme::INK),
                                        );
                                        ui.label(
                                            RichText::new(format!(
                                                "     {} {}",
                                                self.tr().investigator_see,
                                                r.reveals
                                            ))
                                            .size(11.5)
                                            .italics()
                                            .color(theme::INK_DIM),
                                        );
                                    }
                                });
                        }

                        for warning in &report.warnings {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("! {warning}")).size(12.0).color(theme::WARN),
                            );
                        }

                        // Reported below the metadata findings and in its own
                        // words, so "reduced" is never mistaken for "removed".
                        match &entry.wash {
                            Some(Ok(w)) => {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!(
                                        "{}{}x{} -> {}x{}{}{}",
                                        self.tr().fp_reduced,
                                        w.original.0,
                                        w.original.1,
                                        w.washed.0,
                                        w.washed.1,
                                        self.tr().fp_quality,
                                        w.quality
                                    ))
                                    .font(mono(10.5))
                                    .color(theme::WARN),
                                );
                            }
                            Some(Err(e)) => {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!("{}{e}", self.tr().fp_not_reduced))
                                        .size(12.0)
                                        .color(theme::WARN),
                                );
                            }
                            None => {}
                        }

                        if let Some(dst) = &entry.saved_to {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("{} {}", self.tr().saved_to, dst.display()))
                                    .font(mono(10.5))
                                    .color(theme::OK),
                            );
                        } else if s.report.assurance != Assurance::None {
                            // A per-file save, because the automatic name is
                            // derived from the original and the original name is
                            // often not one you would choose. Windows screenshots
                            // are called things like "Screenshot 2026-08-07
                            // 235753.png", and a date and time in a filename is
                            // itself information you may not want to hand over.
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .button(self.tr().save_as)
                                    .on_hover_text(self.tr().save_as_hint)
                                    .clicked()
                                {
                                    save_as = Some(index);
                                }
                                // Show the exact name the file will be saved
                                // under — the random name is stable (generated
                                // once per file), so it can be shown rather than
                                // hidden behind a placeholder. The extension
                                // follows the saved bytes (a washed PNG is saved
                                // as .jpg), not the source name.
                                let ext = entry.output_ext();
                                let stem =
                                    self.randomize_name.then_some(entry.random_stem.as_str());
                                let preview = output_name(&entry.path, stem, ext.as_deref())
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_default();
                                ui.label(
                                    RichText::new(format!(
                                        "{} {}",
                                        self.tr().otherwise_saved,
                                        preview
                                    ))
                                    .font(mono(10.0))
                                    .color(theme::INK_FAINT),
                                );
                            });
                        }
                    }
                }
            });

        // Acted on after the frame closes, so the dialog does not run while the
        // entry is still borrowed for drawing.
        if let Some(i) = save_as {
            self.save_one_as(i);
        }
    }

    /// Save a single entry under a name the user picks.
    fn save_one_as(&mut self, index: usize) {
        let Some(entry) = self.entries.get(index) else { return };
        let Ok(sanitized) = &entry.result else { return };

        // Pre-fill the dialog with the file's chosen name (its stable random
        // name, or the .clean name) — which the user can accept or type over,
        // also how they get a fully custom name.
        let stem = self.randomize_name.then_some(entry.random_stem.as_str());
        let suggested = output_name(&entry.path, stem, entry.output_ext().as_deref());
        let dialog = rfd::FileDialog::new().set_file_name(
            suggested.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(),
        );
        let dialog = match entry.path.parent() {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        };

        let Some(dst) = dialog.save_file() else { return };
        privacy::forget_recent(std::slice::from_ref(&dst));
        // Wipe this copy of the cleaned bytes once written, like the input buffer.
        let data = Zeroizing::new(sanitized.data.clone());
        match write_atomic(&dst, &data) {
            Ok(()) => {
                if let Some(e) = self.entries.get_mut(index) {
                    e.saved_to = Some(dst);
                }
            }
            Err(e) => {
                self.error = Some(format!("{} {}: {e}", self.tr().could_not_write, dst.display()))
            }
        }
    }
}

/// The mark drawn beside a badge's text.
///
/// Drawn rather than typed. The bundled font has no check mark, so a `\u{2713}`
/// renders as a hollow "missing glyph" box, which is both ugly and ambiguous on
/// the one control that tells the user whether their file is safe.
#[derive(Clone, Copy)]
enum Mark {
    Disc,
    Triangle,
    Cross,
}

/// State is a shape *and* a word *and* a colour. Any one of the three carries
/// the meaning on its own, so the badge still reads in greyscale, for colour
/// blindness, or read aloud (WCAG 1.4.1).
fn badge(ui: &mut egui::Ui, text: &str, colour: Color32, mark: Mark) {
    egui::Frame::default()
        .stroke(Stroke::new(1.0, colour))
        .corner_radius(3.0)
        .inner_margin(egui::Margin::symmetric(6, 3))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(9.0, 9.0), egui::Sense::hover());
                {
                    let p = ui.painter();
                    let c = rect.center();
                    let r = 4.0;
                    match mark {
                        Mark::Disc => {
                            p.circle_filled(c, r * 0.85, colour);
                        }
                        Mark::Triangle => {
                            p.add(egui::Shape::convex_polygon(
                                vec![
                                    egui::pos2(c.x, c.y - r),
                                    egui::pos2(c.x + r, c.y + r * 0.8),
                                    egui::pos2(c.x - r, c.y + r * 0.8),
                                ],
                                colour,
                                Stroke::NONE,
                            ));
                        }
                        Mark::Cross => {
                            let s = Stroke::new(1.6, colour);
                            p.line_segment(
                                [egui::pos2(c.x - r, c.y - r), egui::pos2(c.x + r, c.y + r)],
                                s,
                            );
                            p.line_segment(
                                [egui::pos2(c.x + r, c.y - r), egui::pos2(c.x - r, c.y + r)],
                                s,
                            );
                        }
                    }
                }
                ui.label(RichText::new(text).font(mono(10.0)).color(colour));
            });
        });
}

fn main() -> eframe::Result<()> {
    // Before anything is loaded, so a crash during startup cannot dump either.
    privacy::suppress_crash_dumps();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 620.0])
            .with_min_inner_size([520.0, 400.0])
            .with_title("metascrub"),
        ..Default::default()
    };
    eframe::run_native("metascrub", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
