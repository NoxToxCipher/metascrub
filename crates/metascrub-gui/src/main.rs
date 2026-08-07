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

use eframe::egui;
use egui::{Color32, FontFamily, FontId, RichText, Stroke};
use metascrub::{Assurance, ColorProfile, Orientation, Policy, Sanitized};
use pixelwash::{Strength, WashReport};

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
    pub const OK: Color32 = Color32::from_rgb(0x74, 0xa9, 0x7b);
    pub const WARN: Color32 = Color32::from_rgb(0xd9, 0x97, 0x3f);
    pub const DANGER: Color32 = Color32::from_rgb(0xc4, 0x58, 0x4b);
}

fn mono(size: f32) -> FontId {
    FontId::new(size, FontFamily::Monospace)
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
}

/// Work happens off the UI thread so a large PDF cannot freeze the window.
enum Job {
    Done(PathBuf, Result<Sanitized, String>, Option<Result<WashReport, String>>),
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct App {
    entries: Vec<Entry>,
    policy: Policy,
    tx: Sender<Job>,
    rx: Receiver<Job>,
    pending: usize,
    /// Set when a save fails, so the failure is visible rather than silent.
    error: Option<String>,

    /// Pixel washing is off until asked for: it degrades the photograph, and a
    /// protection that costs something should be a decision, not a default.
    wash_enabled: bool,
    wash_strength: Strength,
    /// Shown the first time washing is switched on, before any file is touched.
    intro_open: bool,
    /// The reference panel, explaining every removal and the PRNU work.
    reference_open: bool,
}

impl Default for App {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            entries: Vec::new(),
            policy: Policy::default(),
            tx,
            rx,
            pending: 0,
            error: None,
            wash_enabled: false,
            wash_strength: Strength::default(),
            intro_open: false,
            reference_open: false,
        }
    }
}

/// Marker recording that the pixel-washing explanation has been read, so it
/// appears once rather than every launch. A file rather than a registry entry
/// or a settings service: one path, easy to find, easy to delete.
fn intro_marker() -> Option<PathBuf> {
    let base = std::env::var_os("APPDATA")
        .or_else(|| std::env::var_os("XDG_CONFIG_HOME"))
        .or_else(|| std::env::var_os("HOME"))?;
    Some(PathBuf::from(base).join("metascrub").join("prnu-intro-seen"))
}

fn intro_already_seen() -> bool {
    intro_marker().map(|p| p.exists()).unwrap_or(false)
}

fn remember_intro_seen() {
    if let Some(path) = intro_marker() {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, b"read\n");
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.all_styles_mut(|style| {
            style.visuals.dark_mode = true;
            style.visuals.panel_fill = theme::GROUND;
            style.visuals.window_fill = theme::PANEL;
            style.visuals.extreme_bg_color = theme::PANEL2;
            style.visuals.override_text_color = Some(theme::INK);
            style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, theme::LINE);
            style.visuals.widgets.inactive.bg_fill = theme::PANEL2;
            style.visuals.widgets.hovered.bg_fill = theme::LINE;
            style.visuals.widgets.active.bg_fill = theme::LINE;
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(12.0, 7.0);
        });

        Self::default()
    }

    fn queue(&mut self, paths: Vec<PathBuf>) {
        for path in paths {
            if self.entries.iter().any(|e| e.path == path) {
                continue; // already listed
            }
            self.pending += 1;
            let tx = self.tx.clone();
            let policy = self.policy.clone();
            let wash = self.wash_enabled.then_some(self.wash_strength);
            std::thread::spawn(move || {
                let (result, wash_report) = process(&path, &policy, wash);
                let _ = tx.send(Job::Done(path, result, wash_report));
            });
        }
    }

    fn drain(&mut self) {
        while let Ok(Job::Done(path, result, wash)) = self.rx.try_recv() {
            self.pending = self.pending.saturating_sub(1);
            self.entries.push(Entry { path, result, wash, saved_to: None });
        }
    }

    /// Re-run everything against the current policy, so toggling a setting
    /// updates the findings rather than leaving stale ones on screen.
    fn rerun(&mut self) {
        let paths: Vec<PathBuf> = self.entries.iter().map(|e| e.path.clone()).collect();
        self.entries.clear();
        self.queue(paths);
    }

    fn save_all(&mut self) {
        self.error = None;
        for entry in self.entries.iter_mut().filter(|e| e.is_writable()) {
            let Ok(sanitized) = &entry.result else { continue };
            let dst = clean_name(&entry.path);
            match write_atomic(&dst, &sanitized.data) {
                Ok(()) => entry.saved_to = Some(dst),
                Err(e) => {
                    self.error = Some(format!("could not write {}: {e}", dst.display()));
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
fn process(
    path: &std::path::Path,
    policy: &Policy,
    wash: Option<Strength>,
) -> (Result<Sanitized, String>, Option<Result<WashReport, String>>) {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return (Err(e.to_string()), None),
    };

    let stripped = match metascrub::sanitize(&bytes, policy) {
        Ok(s) => s,
        Err(e) => return (Err(e.to_string()), None),
    };

    let Some(strength) = wash else {
        return (Ok(stripped), None);
    };

    // Only images can be washed. Anything else keeps its stripped form and says
    // so, rather than silently ignoring the setting.
    let settings = pixelwash::Settings { strength, ..Default::default() };
    match pixelwash::wash(&bytes, &settings) {
        Ok(washed) => {
            let report = washed.report.clone();
            match metascrub::sanitize(&washed.data, policy) {
                Ok(final_pass) => (
                    Ok(Sanitized { data: final_pass.data, report: stripped.report }),
                    Some(Ok(report)),
                ),
                Err(e) => (Ok(stripped), Some(Err(e.to_string()))),
            }
        }
        Err(e) => (Ok(stripped), Some(Err(e.to_string()))),
    }
}

/// `holiday.jpg` becomes `holiday.clean.jpg`. The original is never touched:
/// nothing is overwritten unless the user explicitly asks for it.
fn clean_name(path: &std::path::Path) -> PathBuf {
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let name = match path.extension() {
        Some(ext) => format!("{stem}.clean.{}", ext.to_string_lossy()),
        None => format!("{stem}.clean"),
    };
    path.with_file_name(name)
}

/// Write to a temporary file and rename. An interrupted direct write leaves a
/// truncated file wearing a name that says it was cleaned, and the user has no
/// reason to distrust it.
fn write_atomic(dst: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    let dir = dst.parent().unwrap_or_else(|| std::path::Path::new("."));
    let name = dst.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp = dir.join(format!(".{name}.{}.metascrub", std::process::id()));

    let mut file = std::fs::File::create(&tmp)?;
    let written = file.write_all(data).and_then(|()| file.sync_all());
    drop(file);
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    if let Err(e) = std::fs::rename(&tmp, dst) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(())
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
                                RichText::new(format!("reading {} file(s)", self.pending))
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
    fn top_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("top")
            .frame(egui::Frame::default().fill(theme::PANEL2).inner_margin(10.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("metascrub").size(15.0).strong().color(theme::INK));
                    ui.label(
                        RichText::new("removes what a file says about you")
                            .size(12.0)
                            .color(theme::INK_FAINT),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("What is removed, and why").clicked() {
                            self.reference_open = true;
                        }
                        ui.separator();

                        // Both settings keep *more* than the safe minimum, so
                        // the default state is the protective one.
                        let mut keep_rotation =
                            self.policy.orientation == Orientation::PreserveMinimal;
                        if ui
                            .checkbox(&mut keep_rotation, "Keep rotation")
                            .on_hover_text(
                                "Photos will not display sideways. The file keeps a small \
                                 EXIF block holding only the rotation.",
                            )
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
                            .checkbox(&mut keep_icc, "Keep colour")
                            .on_hover_text(
                                "Wide-gamut images render correctly. The profile is a blob \
                                 we do not parse and can name your monitor.",
                            )
                            .changed()
                        {
                            self.policy.color_profile =
                                if keep_icc { ColorProfile::Keep } else { ColorProfile::Drop };
                            self.rerun();
                        }
                    });
                });

                // Second row: the pixel work, kept visually apart from the
                // metadata settings above because it is a different kind of
                // claim and must not be read as part of the same guarantee.
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    let mut enabled = self.wash_enabled;
                    if ui
                        .checkbox(&mut enabled, "Reduce camera fingerprint")
                        .on_hover_text(
                            "Denoises, shrinks and re-compresses the photograph to make the \
                             pattern your sensor leaves in the pixels harder to match. \
                             Reduces it; cannot remove it. Costs image quality.",
                        )
                        .changed()
                    {
                        self.wash_enabled = enabled;
                        if enabled && !intro_already_seen() {
                            self.intro_open = true;
                        } else {
                            self.rerun();
                        }
                    }

                    if self.wash_enabled {
                        let before = self.wash_strength;
                        egui::ComboBox::from_id_salt("wash_strength")
                            .selected_text(match self.wash_strength {
                                Strength::Gentle => "Gentle",
                                Strength::Balanced => "Balanced",
                                Strength::Thorough => "Thorough",
                            })
                            .show_ui(ui, |ui| {
                                for s in [Strength::Gentle, Strength::Balanced, Strength::Thorough]
                                {
                                    let label = match s {
                                        Strength::Gentle => "Gentle",
                                        Strength::Balanced => "Balanced",
                                        Strength::Thorough => "Thorough",
                                    };
                                    ui.selectable_value(&mut self.wash_strength, s, label)
                                        .on_hover_text(s.describe());
                                }
                            });
                        if before != self.wash_strength {
                            self.rerun();
                        }
                        ui.label(
                            RichText::new(self.wash_strength.describe())
                                .size(11.0)
                                .color(theme::INK_FAINT),
                        );
                        ui.label(
                            RichText::new("best effort only").font(mono(10.0)).color(theme::WARN),
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
        egui::Window::new("Before you use this")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .default_width(520.0)
            .show(ctx, |ui| {
                ui.label(
                    RichText::new("About your camera's fingerprint")
                        .size(16.0)
                        .strong()
                        .color(theme::INK),
                );
                ui.add_space(8.0);
                ui.label(RichText::new(reference::FIRST_USE).size(13.0).color(theme::INK_DIM));
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new("I understand").color(theme::GROUND).strong(),
                            )
                            .fill(theme::ACCENT),
                        )
                        .clicked()
                    {
                        remember_intro_seen();
                        self.intro_open = false;
                        self.rerun();
                    }
                    if ui.button("Read the full explanation").clicked() {
                        remember_intro_seen();
                        self.intro_open = false;
                        self.reference_open = true;
                        self.rerun();
                    }
                    if ui.button("Cancel").clicked() {
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
        egui::Window::new("What is removed, and why")
            .open(&mut open)
            .collapsible(false)
            .default_width(640.0)
            .default_height(560.0)
            .vscroll(true)
            .show(ctx, |ui| {
                ui.label(RichText::new("Metadata").size(17.0).strong().color(theme::INK));
                ui.label(
                    RichText::new(
                        "Files are rebuilt from a list of what to keep, so anything not named \
                         here is dropped as well, including private sections this tool has \
                         never seen.",
                    )
                    .size(12.5)
                    .color(theme::INK_FAINT),
                );
                ui.add_space(10.0);

                for item in reference::METADATA {
                    egui::Frame::default()
                        .fill(theme::PANEL)
                        .stroke(Stroke::new(1.0, theme::LINE))
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(item.name).size(13.5).strong().color(theme::ACCENT),
                            );
                            ui.add_space(3.0);
                            ui.label(RichText::new(item.what).size(12.5).color(theme::INK));
                            ui.add_space(4.0);
                            ui.label(RichText::new(item.why).size(12.5).color(theme::INK_DIM));
                        });
                    ui.add_space(6.0);
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(
                    RichText::new("The camera's fingerprint in the pixels")
                        .size(17.0)
                        .strong()
                        .color(theme::INK),
                );
                ui.label(
                    RichText::new(
                        "A separate problem from metadata, addressed by a separate, optional \
                         tool, with a weaker guarantee.",
                    )
                    .size(12.5)
                    .color(theme::INK_FAINT),
                );
                ui.add_space(10.0);

                for section in reference::PRNU {
                    ui.label(
                        RichText::new(section.heading).size(14.0).strong().color(theme::ACCENT),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new(section.body).size(12.5).color(theme::INK_DIM));
                    ui.add_space(14.0);
                }

                ui.add_space(6.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(
                    RichText::new("Things you may have been told")
                        .size(17.0)
                        .strong()
                        .color(theme::INK),
                );
                ui.label(
                    RichText::new(
                        "Bad advice is worse than none, because someone who believes a file is \
                         clean will act as though it is.",
                    )
                    .size(12.5)
                    .color(theme::INK_FAINT),
                );
                ui.add_space(10.0);

                for myth in reference::MYTHS {
                    egui::Frame::default()
                        .fill(theme::PANEL)
                        .stroke(Stroke::new(1.0, theme::LINE))
                        .corner_radius(6.0)
                        .inner_margin(10.0)
                        .show(ui, |ui| {
                            ui.horizontal_top(|ui| {
                                ui.label(RichText::new("claim").font(mono(9.5)).color(theme::WARN));
                                ui.label(
                                    RichText::new(myth.claim)
                                        .size(12.5)
                                        .italics()
                                        .color(theme::INK),
                                );
                            });
                            ui.add_space(5.0);
                            ui.horizontal_top(|ui| {
                                ui.label(RichText::new("truth").font(mono(9.5)).color(theme::OK));
                                ui.label(
                                    RichText::new(myth.reality).size(12.5).color(theme::INK_DIM),
                                );
                            });
                        });
                    ui.add_space(6.0);
                }

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(12.0);

                ui.label(
                    RichText::new("What the research actually shows")
                        .size(17.0)
                        .strong()
                        .color(theme::INK),
                );
                ui.label(
                    RichText::new(
                        "Including the parts that limit what this tool is allowed to claim.",
                    )
                    .size(12.5)
                    .color(theme::INK_FAINT),
                );
                ui.add_space(10.0);

                for section in reference::EVIDENCE {
                    ui.label(
                        RichText::new(section.heading).size(14.0).strong().color(theme::ACCENT),
                    );
                    ui.add_space(4.0);
                    ui.label(RichText::new(section.body).size(12.5).color(theme::INK_DIM));
                    ui.add_space(14.0);
                }

                ui.add_space(8.0);
                ui.label(
                    RichText::new(
                        "Sources: Lukáš, Fridrich & Goljan, 'Digital Camera Identification from \
                         Sensor Pattern Noise', IEEE Transactions on Information Forensics and \
                         Security, 2006. Subsequent work on robustness under downscaling, on \
                         counter-forensic resampling, and on smartphone reliability.",
                    )
                    .size(11.0)
                    .color(theme::INK_FAINT),
                );
                ui.add_space(8.0);
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
                            RichText::new(format!("{complete} complete"))
                                .font(mono(12.0))
                                .color(theme::OK),
                        );
                        ui.label(RichText::new("·").color(theme::INK_FAINT));
                        ui.label(
                            RichText::new(format!("{partial} best effort"))
                                .font(mono(12.0))
                                .color(theme::WARN),
                        );
                        if skipped > 0 {
                            ui.label(RichText::new("·").color(theme::INK_FAINT));
                            ui.label(
                                RichText::new(format!("{skipped} skipped"))
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
                                        "Save {writable} cleaned cop{}",
                                        if writable == 1 { "y" } else { "ies" }
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
                            .add_enabled(!self.entries.is_empty(), egui::Button::new("Clear"))
                            .clicked()
                        {
                            self.entries.clear();
                            self.error = None;
                        }
                        if ui.button("Add files...").clicked() {
                            if let Some(files) = rfd::FileDialog::new().pick_files() {
                                self.queue(files);
                            }
                        }
                    });
                });

                if let Some(err) = &self.error {
                    ui.add_space(4.0);
                    ui.label(RichText::new(format!("! {err}")).size(12.0).color(theme::DANGER));
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
                        RichText::new("Drop files here").size(20.0).strong().color(theme::INK),
                    );
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(
                            "Photos, PDFs and Office documents.\n\
                             Nothing is uploaded. Nothing leaves this computer.",
                        )
                        .size(13.0)
                        .color(theme::INK_FAINT),
                    );
                    ui.add_space(16.0);
                    if ui.button("Choose files...").clicked() {
                        if let Some(files) = rfd::FileDialog::new().pick_files() {
                            self.queue(files);
                        }
                    }
                    ui.add_space(40.0);
                });
            });
    }

    fn file_card(&mut self, ui: &mut egui::Ui, index: usize) {
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
                            badge(ui, "COULD NOT READ", theme::DANGER, Mark::Cross);
                        });
                        ui.label(RichText::new(e).size(12.0).color(theme::INK_DIM));
                    }
                    Ok(s) => {
                        let report = &s.report;
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&name).strong().color(theme::INK));
                            match report.assurance {
                                Assurance::Complete => badge(ui, "COMPLETE", theme::OK, Mark::Disc),
                                Assurance::BestEffort => {
                                    badge(ui, "BEST EFFORT", theme::WARN, Mark::Triangle)
                                }
                                Assurance::None => {
                                    badge(ui, "NOT CLEANED", theme::DANGER, Mark::Cross)
                                }
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
                                RichText::new("This file recorded where it was taken")
                                    .size(12.5)
                                    .strong()
                                    .color(theme::DANGER),
                            );
                        }

                        if !report.removed.is_empty() {
                            ui.add_space(6.0);
                            for item in &report.removed {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("\u{00D7}").font(mono(12.0)).color(theme::OK));
                                    ui.label(RichText::new(item.kind.to_string()).size(12.5).color(theme::INK));
                                    ui.label(RichText::new(&item.location).font(mono(10.5)).color(theme::INK_FAINT));
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

                        for warning in &report.warnings {
                            ui.add_space(4.0);
                            ui.label(RichText::new(format!("! {warning}")).size(12.0).color(theme::WARN));
                        }

                        // Reported below the metadata findings and in its own
                        // words, so "reduced" is never mistaken for "removed".
                        match &entry.wash {
                            Some(Ok(w)) => {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!(
                                        "fingerprint reduced (best effort): {}x{} -> {}x{}, quality {}",
                                        w.original.0,
                                        w.original.1,
                                        w.washed.0,
                                        w.washed.1,
                                        w.quality
                                    ))
                                    .font(mono(10.5))
                                    .color(theme::WARN),
                                );
                            }
                            Some(Err(e)) => {
                                ui.add_space(6.0);
                                ui.label(
                                    RichText::new(format!(
                                        "fingerprint not reduced: {e}"
                                    ))
                                    .size(12.0)
                                    .color(theme::WARN),
                                );
                            }
                            None => {}
                        }

                        if let Some(dst) = &entry.saved_to {
                            ui.add_space(4.0);
                            ui.label(
                                RichText::new(format!("saved to {}", dst.display()))
                                    .font(mono(10.5))
                                    .color(theme::OK),
                            );
                        }
                    }
                }
            });
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
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([760.0, 620.0])
            .with_min_inner_size([520.0, 400.0])
            .with_title("metascrub"),
        ..Default::default()
    };
    eframe::run_native("metascrub", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
