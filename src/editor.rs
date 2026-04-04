//! # editor.rs — egui GUI for ChordLens
//!
//! Reads chord state written by the audio thread and renders it using egui.
//! Deliberately minimalistic: large chord name, smaller note list, subtle
//! inversion hint.  All layout is done with egui's built-in layout system;
//! no textures or images are used so the plugin has zero asset dependencies.

use crate::ChordState;

use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Vec2},
    EguiState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
// ─── Palette ──────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(13, 13, 18);
const CHORD_TEXT: Color32 = Color32::from_rgb(245, 245, 250);
const SLASH_TEXT: Color32 = Color32::from_rgb(150, 150, 165);
const NOTE_TEXT: Color32 = Color32::from_rgb(140, 140, 160);
const INV_TEXT: Color32 = Color32::from_rgb(130, 130, 140);
const DIVIDER: Color32 = Color32::from_rgb(60, 60, 75);

/// Toggles whether octave numbers are displayed in a subtle grey or in the scale color.
const SHOW_GREY_OCTAVES: bool = false;

// ─── Widget sizes ─────────────────────────────────────────────────────────────

pub const EDITOR_WIDTH: u32 = 480;
pub const EDITOR_HEIGHT: u32 = 300;

// ─── Font setup ──────────────────────────────────────────────────────────────

/// Register the bundled Inter font so we're not at the mercy of the host's
/// system fonts.  The font bytes are baked into the binary at compile time.
///
/// Inter is released under the SIL Open Font License 1.1.
static INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");
static INTER_LIGHT: &[u8] = include_bytes!("../assets/Inter-Light.ttf");

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "Inter-Regular".to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts
        .font_data
        .insert("Inter-Light".to_owned(), Arc::new(FontData::from_static(INTER_LIGHT)));

    // Make Inter the *first* choice for proportional text
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter-Regular".to_owned());

    ctx.set_fonts(fonts);
}

// ─── Editor factory ──────────────────────────────────────────────────────────

/// Build the egui editor.  Called once per window open by nih-plug.
///
/// `chord_state` is an `Arc<RwLock<ChordState>>` written exclusively from the
/// audio thread (the `process()` function) and read here on the GUI thread.
/// Because the audio thread **only writes** and the GUI thread **only reads**,
/// and `parking_lot::RwLock` never allocates on the uncontended fast path,
/// this is safe and performant.
pub fn create(
    params: Arc<crate::ChordLensParams>,
    chord_state: Arc<parking_lot::RwLock<crate::ChordState>>,
    reset_history: Arc<AtomicBool>,
) -> Option<Box<dyn nih_plug::prelude::Editor>> {
    let editor_state = params.editor_state.clone();
    let rs_state = editor_state.clone();
    create_egui_editor(
        editor_state,
        (),
        |ctx, _| {
            setup_fonts(ctx);
            style_egui(ctx);
        },
        move |ctx, setter, _state| {
            // Take a lightweight read-lock to snapshot the current chord state
            // so we don't hold the lock while rendering UI commands.
            let snapshot = chord_state.read().clone();
            
            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(BG))
                .show(ctx, |ui| {
                    draw_ui(ui, setter, &params, &snapshot, &reset_history, &rs_state);
                });
        },
    )
}

// ─── Styling ─────────────────────────────────────────────────────────────────

fn style_egui(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();

    // Transparent windows / panels
    style.visuals.window_fill = BG;
    style.visuals.panel_fill = BG;
    style.visuals.faint_bg_color = BG;
    style.visuals.extreme_bg_color = BG;
    style.visuals.widgets.noninteractive.bg_fill = BG;

    // No strokes/shadows
    style.visuals.window_shadow = egui::Shadow::NONE;
    style.visuals.popup_shadow = egui::Shadow::NONE;

    style.spacing.item_spacing = Vec2::new(0.0, 0.0);

    ctx.set_style(style);
}

const NOTE_OFF: Color32 = Color32::from_rgb(100, 100, 110);

fn get_note_color(pc: u8, scale_root: u8, scale_intervals: &[i32]) -> Color32 {
    let rel = (pc as i32 + 12 - scale_root as i32) % 12;
    
    // Cohesive Hue-Shifted Palette (Harmonious Pastels centered on Mint)
    // Tonic is the original project teal.
    if rel == 0 { return Color32::from_rgb(120, 220, 180); } // 1 (Mint)
    if rel == 2 { return Color32::from_rgb(120, 200, 220); } // 2 (Aqua)
    if rel == 4 || rel == 3 { return Color32::from_rgb(150, 180, 240); } // 3 (Soft Blue)
    if rel == 5 { return Color32::from_rgb(190, 160, 240); } // 4 (Lavender)
    if rel == 7 { return Color32::from_rgb(230, 160, 230); } // 5 (Rose)
    if rel == 9 || rel == 8 { return Color32::from_rgb(245, 180, 180); } // 6 (Salmon)
    if rel == 11 || rel == 10 { return Color32::from_rgb(235, 210, 150); } // 7 (Amber)

    // Falls into scale but not a primary degree? Check intervals
    if scale_intervals.contains(&rel) {
        return NOTE_TEXT; // Fallback
    }

    NOTE_OFF // Out of scale (Grey)
}

// ─── Frame drawing ────────────────────────────────────────────────────────────

fn draw_ui(
    ui: &mut egui::Ui, 
    setter: &nih_plug::prelude::ParamSetter, 
    params: &Arc<crate::ChordLensParams>, 
    snapshot: &ChordState,
    reset_history: &Arc<AtomicBool>,
    _egui_state: &Arc<EguiState>,
) {
    // ── Top Bar (Key Detection) ──
    egui::TopBottomPanel::top("top_bar_panel")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(12i8)))
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(30, 30, 40);
                
                let mut root = params.key_root.value();
                let r_changed = egui::ComboBox::from_id_salt("root_cmb")
                    .selected_text(root.as_str())
                    .show_ui(ui, |ui| {
                        let mut changed = false;
                        for r in [
                            crate::KeyRoot::Auto, crate::KeyRoot::C, crate::KeyRoot::CSharp, 
                            crate::KeyRoot::D, crate::KeyRoot::DSharp, crate::KeyRoot::E, 
                            crate::KeyRoot::F, crate::KeyRoot::FSharp, crate::KeyRoot::G, 
                            crate::KeyRoot::GSharp, crate::KeyRoot::A, crate::KeyRoot::ASharp, crate::KeyRoot::B
                        ] {
                            if ui.selectable_label(root == r, r.as_str()).clicked() {
                                root = r;
                                changed = true;
                            }
                        }
                        changed
                    }).inner.unwrap_or(false);

                if r_changed {
                    setter.begin_set_parameter(&params.key_root);
                    setter.set_parameter(&params.key_root, root);
                    setter.end_set_parameter(&params.key_root);
                }
                
                if root != crate::KeyRoot::Auto {
                    let mut mode = params.key_mode.value();
                    let m_changed = egui::ComboBox::from_id_salt("mode_cmb")
                        .selected_text(mode.as_str())
                        .show_ui(ui, |ui| {
                            let mut changed = false;
                            for m in [
                                crate::KeyMode::Major, crate::KeyMode::Minor, crate::KeyMode::Dorian,
                                crate::KeyMode::Phrygian, crate::KeyMode::Lydian, crate::KeyMode::Mixolydian,
                                crate::KeyMode::Locrian
                            ] {
                                if ui.selectable_label(mode == m, m.as_str()).clicked() {
                                    mode = m;
                                    changed = true;
                                }
                            }
                            changed
                        }).inner.unwrap_or(false);

                    if m_changed {
                        setter.begin_set_parameter(&params.key_mode);
                        setter.set_parameter(&params.key_mode, mode);
                        setter.end_set_parameter(&params.key_mode);
                    }
                }

                // Nashville Toggle
                ui.add_space(8.0);
                let mut show_n = params.show_nashville.value();
                if ui.checkbox(&mut show_n, "N").clicked() {
                    setter.begin_set_parameter(&params.show_nashville);
                    setter.set_parameter(&params.show_nashville, show_n);
                    setter.end_set_parameter(&params.show_nashville);
                }

                let mut show_rl = params.allow_rootless.value();
                if ui.checkbox(&mut show_rl, "RL").on_hover_text("Allow Root-less Voicings (Experimental)").clicked() {
                    setter.begin_set_parameter(&params.allow_rootless);
                    setter.set_parameter(&params.allow_rootless, show_rl);
                    setter.end_set_parameter(&params.allow_rootless);
                }

                ui.add_space(8.0);
                ui.label(RichText::new("Acc:").size(12.0).color(Color32::from_rgb(120, 120, 130)));
                let mut d_ms = params.debounce_ms.value();
                if ui.add(egui::DragValue::new(&mut d_ms).suffix("ms").range(0..=100)).changed() {
                    setter.begin_set_parameter(&params.debounce_ms);
                    setter.set_parameter(&params.debounce_ms, d_ms);
                    setter.end_set_parameter(&params.debounce_ms);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if params.key_root.value() == crate::KeyRoot::Auto {
                        if ui.add(egui::Button::new(egui::RichText::new("Clear").size(10.0)).fill(Color32::from_rgb(40, 40, 50))).clicked() {
                            reset_history.store(true, Ordering::Relaxed);
                        }
                        ui.add_space(8.0);
                    }
                    ui.label(egui::RichText::new(&snapshot.key_text).color(Color32::from_rgb(120, 120, 130)).size(16.0));
                });
            });
        });

    // ── Bottom Box (Active Notes) ──
    egui::TopBottomPanel::bottom("bottom_bar_panel")
        .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(14i8, 16i8)))
        .exact_height(70.0) // Compact bottom bar
        .show_separator_line(false)
        .show_inside(ui, |ui| {
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                let notes = &snapshot.chord_info.active_notes;
                if notes.is_empty() {
                    ui.label(
                        RichText::new("no notes")
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .color(INV_TEXT),
                    );
                } else {
                    let mut note_job = egui::text::LayoutJob::default();
                    note_job.halign = egui::Align::Center;
                    
                    for (i, (name, _role)) in notes.iter().enumerate() {
                        let mut pc = 0;
                        for p in 0..12 {
                            if name.starts_with(&crate::chord::pc_name(p, snapshot.scale_root)) {
                                pc = p;
                            }
                        }
                        
                        let color = get_note_color(pc, snapshot.scale_root, &snapshot.scale_intervals);
                        
                        // Split name (e.g. "C#", "C#4") to color octave number in grey
                        let (note_part, octave_part) = if let Some(first_digit) = name.find(|c: char| c.is_ascii_digit() || c == '-') {
                            (&name[..first_digit], &name[first_digit..])
                        } else {
                            (name.as_str(), "")
                        };

                        note_job.append(
                            note_part,
                            0.0,
                            egui::text::TextFormat {
                                font_id: FontId::new(28.0, FontFamily::Proportional),
                                color,
                                ..Default::default()
                            }
                        );
                        
                        if !octave_part.is_empty() {
                            let octave_color = if SHOW_GREY_OCTAVES { NOTE_OFF } else { color };
                            note_job.append(
                                octave_part,
                                0.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(28.0, FontFamily::Proportional), 
                                    color: octave_color,
                                    ..Default::default()
                                }
                            );
                        }
                        
                        if i < notes.len() - 1 {
                            note_job.append(
                                " · ", // reduced spacing
                                0.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(28.0, FontFamily::Proportional),
                                    color: DIVIDER,
                                    ..Default::default()
                                }
                            );
                        }
                    }
                    ui.label(note_job);
                }

                // Nashville Numerals bottom left
                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    if params.show_nashville.value() && !snapshot.nashville_text.is_empty() {
                        ui.label(
                            egui::RichText::new(&snapshot.nashville_text)
                                .font(FontId::new(48.0, FontFamily::Proportional)) // Significantly bigger Nashville
                                .color(Color32::from_rgb(220, 220, 230)),
                        );
                    }
                    ui.add_space(10.0);
                });

                // Spacer instead of sizing logic (now at the top)
                ui.with_layout(egui::Layout::bottom_up(egui::Align::RIGHT), |ui| {
                    ui.add_space(10.0);
                });
            });
        });

    // ── Central Area (Huge Chord Display) ──
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE)
        .show_inside(ui, |ui| {
            // Keep exactly centered relative to the remaining free space!
            ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                ui.add_space(ui.available_height() * 0.5 - 75.0); // manual centering roughly based on typograhpy bounds
                
                let root_name = &snapshot.chord_info.root;
                let root_pc = if root_name.is_empty() || root_name == "–" { None } else {
                    let mut found = None;
                    for p in 0..12 {
                        if root_name.starts_with(&crate::chord::pc_name(p, snapshot.scale_root)) {
                            found = Some(p);
                        }
                    }
                    found
                };
                
                let root_color = if let Some(p) = root_pc {
                    get_note_color(p, snapshot.scale_root, &snapshot.scale_intervals)
                } else {
                    CHORD_TEXT
                };

                let quality = &snapshot.chord_info.quality;
                let omitted = &snapshot.chord_info.omitted;
                let slash = &snapshot.chord_info.slash;

                let mut job = egui::text::LayoutJob::default();
                job.halign = egui::Align::Center;
                
                let (root_note, root_octave) = if let Some(first_digit) = root_name.find(|c: char| c.is_ascii_digit() || c == '-') {
                    (&root_name[..first_digit], &root_name[first_digit..])
                } else {
                    (root_name.as_str(), "")
                };

                job.append(
                    root_note, 
                    0.0,
                    egui::text::TextFormat {
                        font_id: FontId::new(128.0, FontFamily::Proportional),
                        color: root_color,
                        ..Default::default()
                    }
                );
                
                if !root_octave.is_empty() {
                    let root_octave_color = if SHOW_GREY_OCTAVES { NOTE_OFF } else { root_color };
                    job.append(
                        root_octave, 
                        0.0,
                        egui::text::TextFormat {
                            font_id: FontId::new(128.0, FontFamily::Proportional), // Same size as the root
                            color: root_octave_color,
                            valign: egui::Align::Center,
                            ..Default::default()
                        }
                    );
                }

                if !quality.is_empty() {
                    job.append(
                        quality,
                        0.0,
                        egui::text::TextFormat {
                            font_id: FontId::new(64.0, FontFamily::Proportional),
                            color: Color32::from_rgb(180, 180, 195),
                            valign: egui::Align::Center,
                            ..Default::default()
                        }
                    );
                }

                if !omitted.is_empty() {
                    job.append(
                        omitted,
                        4.0, // slight space
                        egui::text::TextFormat {
                            font_id: FontId::new(32.0, FontFamily::Proportional),
                            color: Color32::from_rgb(140, 140, 160),
                            valign: egui::Align::TOP,
                            ..Default::default()
                        }
                    );
                }

                if !slash.is_empty() {
                    job.append(
                        slash,
                        2.0,
                        egui::text::TextFormat {
                            font_id: FontId::new(72.0, FontFamily::Proportional),
                            color: SLASH_TEXT,
                            valign: egui::Align::BOTTOM,
                            ..Default::default()
                        }
                    );
                }
                
                ui.label(job);

                // Inversion hint
                let inv = &snapshot.chord_info.inversion;
                if !inv.is_empty() {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(inv)
                            .font(FontId::new(14.0, FontFamily::Proportional))
                            .color(INV_TEXT)
                            .italics(),
                    );
                }
            });
        });
}
