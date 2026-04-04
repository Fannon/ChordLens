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
use parking_lot::RwLock;
use std::sync::Arc;

// ─── Palette ──────────────────────────────────────────────────────────────────

const BG: Color32 = Color32::from_rgb(13, 13, 18);
const CHORD_TEXT: Color32 = Color32::from_rgb(245, 245, 250);
const SLASH_TEXT: Color32 = Color32::from_rgb(120, 220, 180);
const NOTE_TEXT: Color32 = Color32::from_rgb(140, 140, 160);
const INV_TEXT: Color32 = Color32::from_rgb(90, 90, 110);
const DIVIDER: Color32 = Color32::from_rgb(35, 35, 50);

// ─── Widget sizes ─────────────────────────────────────────────────────────────

pub const EDITOR_WIDTH: u32 = 420;
pub const EDITOR_HEIGHT: u32 = 260;

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
    editor_state: Arc<EguiState>,
    chord_state: Arc<RwLock<ChordState>>,
) -> Option<Box<dyn nih_plug::prelude::Editor>> {
    create_egui_editor(
        editor_state,
        // Per-editor user state (unused – all our state is in Arc)
        (),
        // One-time setup callback
        |ctx, _| {
            setup_fonts(ctx);
            style_egui(ctx);
        },
        // Per-frame paint callback
        move |ctx, _setter, _state| {
            // Snapshot the current chord once per frame so we render a
            // consistent picture even if the audio thread fires mid-frame.
            let snapshot = chord_state.read().clone();

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE.fill(BG))
                .show(ctx, |ui| {
                    draw_ui(ui, &snapshot);
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
    style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    style.visuals.popup_shadow = egui::epaint::Shadow::NONE;

    style.spacing.item_spacing = Vec2::new(0.0, 0.0);

    ctx.set_style(style);
}

// ─── Frame drawing ────────────────────────────────────────────────────────────

fn draw_ui(ui: &mut egui::Ui, snapshot: &ChordState) {
    // Vertically centre everything in the available rect
    let avail = ui.available_size();

    ui.allocate_ui_with_layout(avail, egui::Layout::top_down(egui::Align::Center), |ui| {
        // ── Top padding ──────────────────────────────────────────────────
        ui.add_space(avail.y * 0.15);

        // ── Main chord name ──────────────────────────────────────────────
        let root = &snapshot.chord_info.root;
        let quality = &snapshot.chord_info.quality;
        let omitted = &snapshot.chord_info.omitted;
        let slash = &snapshot.chord_info.slash;

        let mut job = egui::text::LayoutJob::default();
        job.halign = egui::Align::Center;
        
        job.append(
            root, 
            0.0,
            egui::text::TextFormat {
                font_id: FontId::new(96.0, FontFamily::Proportional),
                color: CHORD_TEXT,
                ..Default::default()
            }
        );

        if !quality.is_empty() {
            job.append(
                quality,
                0.0,
                egui::text::TextFormat {
                    font_id: FontId::new(48.0, FontFamily::Proportional),
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
                    font_id: FontId::new(24.0, FontFamily::Proportional),
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
                    font_id: FontId::new(56.0, FontFamily::Proportional),
                    color: SLASH_TEXT,
                    valign: egui::Align::BOTTOM,
                    ..Default::default()
                }
            );
        }
        
        ui.label(job);

        // ── Inversion hint ───────────────────────────────────────────────
        let inv = &snapshot.chord_info.inversion;
        if !inv.is_empty() {
            ui.add_space(4.0);
            ui.label(
                RichText::new(inv)
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .color(INV_TEXT)
                    .italics(),
            );
        }

        // ── Thin decorative divider ─────────────────────────────────────
        ui.add_space(16.0);
        let divider_rect = {
            let (rect, _) =
                ui.allocate_exact_size(Vec2::new(avail.x * 0.55, 1.0), egui::Sense::hover());
            rect
        };
        ui.painter().rect_filled(divider_rect, 0.0, DIVIDER);

        // ── Active notes list ────────────────────────────────────────────
        ui.add_space(12.0);
        let notes = &snapshot.chord_info.active_notes;
        if notes.is_empty() {
            ui.label(
                RichText::new("no notes")
                    .font(FontId::new(13.0, FontFamily::Proportional))
                    .color(INV_TEXT),
            );
        } else {
            let mut note_job = egui::text::LayoutJob::default();
            note_job.halign = egui::Align::Center;
            
            for (i, (name, role)) in notes.iter().enumerate() {
                let color = match role {
                    crate::chord::NoteRole::Normal => NOTE_TEXT,
                    crate::chord::NoteRole::Root => CHORD_TEXT, // bright text
                    crate::chord::NoteRole::Bass => SLASH_TEXT, // teal highlight
                };
                
                note_job.append(
                    name,
                    0.0,
                    egui::text::TextFormat {
                        font_id: FontId::new(14.0, FontFamily::Proportional),
                        color,
                        ..Default::default()
                    }
                );
                
                if i < notes.len() - 1 {
                    note_job.append(
                        "  ·  ",
                        0.0,
                        egui::text::TextFormat {
                            font_id: FontId::new(14.0, FontFamily::Proportional),
                            color: DIVIDER,
                            ..Default::default()
                        }
                    );
                }
            }
            ui.label(note_job);
        }

        // ── Branding whisper ─────────────────────────────────────────────
        ui.add_space(16.0);
        ui.label(
            RichText::new("CHORD LENS")
                .font(FontId::new(9.0, FontFamily::Proportional))
                .color(Color32::from_rgb(40, 40, 55)),
        );
    });
}
