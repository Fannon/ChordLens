//! # editor.rs — egui GUI for ChordLens
//!
//! Reads chord state written by the audio thread and renders it using egui.

use nih_plug_egui::{
    create_egui_editor,
    egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, Pos2, RichText, Vec2},
    EguiState,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const BG: Color32 = Color32::from_rgb(13, 13, 18);
const CHORD_TEXT: Color32 = Color32::from_rgb(245, 245, 250);
const SLASH_TEXT: Color32 = Color32::from_rgb(150, 150, 165);
const NOTE_TEXT: Color32 = Color32::from_rgb(140, 140, 160);
const INV_TEXT: Color32 = Color32::from_rgb(130, 130, 140);
const DIVIDER: Color32 = Color32::from_rgb(60, 60, 75);

const SHOW_GREY_OCTAVES: bool = false;

pub const EDITOR_WIDTH: u32 = 480;
pub const EDITOR_HEIGHT: u32 = 300;

static INTER_REGULAR: &[u8] = include_bytes!("../assets/Inter-Regular.ttf");
static INTER_LIGHT: &[u8] = include_bytes!("../assets/Inter-Light.ttf");

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "Inter-Regular".to_owned(),
        Arc::new(FontData::from_static(INTER_REGULAR)),
    );
    fonts.font_data.insert(
        "Inter-Light".to_owned(),
        Arc::new(FontData::from_static(INTER_LIGHT)),
    );
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "Inter-Regular".to_owned());
    ctx.set_fonts(fonts);
}

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
            let snapshot = chord_state.read().clone();
            draw_ui(ctx, setter, &params, &snapshot, &reset_history, &rs_state);
        },
    )
}

fn style_egui(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.visuals.window_fill = BG;
    style.visuals.panel_fill = BG;
    style.visuals.faint_bg_color = BG;
    style.visuals.extreme_bg_color = BG;
    style.visuals.widgets.noninteractive.bg_fill = BG;
    style.visuals.window_shadow = egui::Shadow::NONE;
    style.visuals.popup_shadow = egui::Shadow::NONE;
    style.spacing.item_spacing = Vec2::ZERO;
    style.spacing.window_margin = egui::Margin::ZERO;
    ctx.set_style(style);
}

const NOTE_OFF: Color32 = Color32::from_rgb(100, 100, 110);

fn get_note_color(
    pc: u8,
    scale_root: u8,
    scale_intervals: &[i32],
    chromatic_mode: bool,
    role: &crate::chord::NoteRole,
) -> Color32 {
    if chromatic_mode {
        return if matches!(role, crate::chord::NoteRole::Root) {
            Color32::from_rgb(120, 220, 180)
        } else {
            NOTE_TEXT
        };
    }
    let rel = (pc as i32 + 12 - scale_root as i32) % 12;
    if rel == 0 {
        return Color32::from_rgb(120, 220, 180);
    }
    if rel == 2 {
        return Color32::from_rgb(120, 200, 220);
    }
    if rel == 4 || rel == 3 {
        return Color32::from_rgb(150, 180, 240);
    }
    if rel == 5 {
        return Color32::from_rgb(190, 160, 240);
    }
    if rel == 7 {
        return Color32::from_rgb(230, 160, 230);
    }
    if rel == 9 || rel == 8 {
        return Color32::from_rgb(245, 180, 180);
    }
    if rel == 11 || rel == 10 {
        return Color32::from_rgb(235, 210, 150);
    }
    if scale_intervals.contains(&rel) {
        return NOTE_TEXT;
    }
    NOTE_OFF
}

fn draw_ui(
    ctx: &egui::Context,
    setter: &nih_plug::prelude::ParamSetter,
    params: &Arc<crate::ChordLensParams>,
    snapshot: &crate::ChordState,
    reset_history: &Arc<AtomicBool>,
    _egui_state: &Arc<EguiState>,
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(BG))
        .show(ctx, |ui| {
            let full_rect = ui.available_rect_before_wrap();

            // ── Header Section (45px) ──
            ui.allocate_new_ui(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    full_rect.min,
                    Vec2::new(full_rect.width(), 45.0),
                )),
                |ui| {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(12.0);
                        ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                        ui.visuals_mut().widgets.hovered.bg_fill = Color32::from_rgb(40, 40, 50);

                        let mut root = params.key_root.value();
                        let r_changed = egui::ComboBox::from_id_salt("root_cmb")
                            .selected_text(root.as_str())
                            .width(62.0)
                            .show_ui(ui, |ui| {
                                let mut changed = false;
                                for r in [
                                    crate::KeyRoot::Auto,
                                    crate::KeyRoot::Chromatic,
                                    crate::KeyRoot::C,
                                    crate::KeyRoot::CSharp,
                                    crate::KeyRoot::D,
                                    crate::KeyRoot::DSharp,
                                    crate::KeyRoot::E,
                                    crate::KeyRoot::F,
                                    crate::KeyRoot::FSharp,
                                    crate::KeyRoot::G,
                                    crate::KeyRoot::GSharp,
                                    crate::KeyRoot::A,
                                    crate::KeyRoot::ASharp,
                                    crate::KeyRoot::B,
                                ] {
                                    if ui.selectable_label(root == r, r.as_str()).clicked() {
                                        root = r;
                                        changed = true;
                                    }
                                }
                                changed
                            })
                            .inner
                            .unwrap_or(false);
                        if r_changed {
                            setter.begin_set_parameter(&params.key_root);
                            setter.set_parameter(&params.key_root, root);
                            setter.end_set_parameter(&params.key_root);
                        }
                        ui.add_space(4.0);

                        let show_h = params.show_history.value();
                        let h_fill = if show_h {
                            Color32::from_rgb(60, 60, 75)
                        } else {
                            Color32::from_rgb(45, 45, 55)
                        };
                        let h_text = if show_h {
                            Color32::from_rgb(220, 220, 230)
                        } else {
                            Color32::from_rgb(200, 200, 210)
                        };
                        let h_label = if show_h { "Notes" } else { "History" };
                        let h_btn = egui::Button::new(
                            egui::RichText::new(h_label).size(11.0).color(h_text),
                        )
                        .fill(h_fill)
                        .min_size(Vec2::new(52.0, 22.0));

                        if root == crate::KeyRoot::Auto {
                            ui.label(
                                egui::RichText::new(&snapshot.key_text)
                                    .color(Color32::from_rgb(120, 120, 130))
                                    .size(16.0),
                            );
                            ui.add_space(8.0);
                            if ui
                                .add(
                                    egui::Button::new(egui::RichText::new("Reset").size(11.0))
                                        .fill(Color32::from_rgb(45, 45, 55))
                                        .min_size(Vec2::new(46.0, 22.0)),
                                )
                                .clicked()
                            {
                                reset_history.store(true, Ordering::Relaxed);
                            }
                            ui.add_space(4.0); // 4px padding between Reset and History
                            if ui.add(h_btn).clicked() {
                                setter.begin_set_parameter(&params.show_history);
                                setter.set_parameter(&params.show_history, !show_h);
                                setter.end_set_parameter(&params.show_history);
                            }
                        } else if root == crate::KeyRoot::Chromatic {
                            ui.label(
                                egui::RichText::new(&snapshot.key_text)
                                    .color(Color32::from_rgb(120, 120, 130))
                                    .size(16.0),
                            );
                            ui.add_space(8.0);
                            if ui.add(h_btn).clicked() {
                                setter.begin_set_parameter(&params.show_history);
                                setter.set_parameter(&params.show_history, !show_h);
                                setter.end_set_parameter(&params.show_history);
                            }
                        } else {
                            let mut mode = params.key_mode.value();
                            let m_changed = egui::ComboBox::from_id_salt("mode_cmb")
                                .selected_text(mode.as_str())
                                .width(82.0)
                                .show_ui(ui, |ui| {
                                    let mut changed = false;
                                    for i in [
                                        crate::KeyMode::Major,
                                        crate::KeyMode::Minor,
                                        crate::KeyMode::Dorian,
                                        crate::KeyMode::Phrygian,
                                        crate::KeyMode::Lydian,
                                        crate::KeyMode::Mixolydian,
                                        crate::KeyMode::Locrian,
                                    ] {
                                        if ui.selectable_label(mode == i, i.as_str()).clicked() {
                                            mode = i;
                                            changed = true;
                                        }
                                    }
                                    changed
                                })
                                .inner
                                .unwrap_or(false);
                            if m_changed {
                                setter.begin_set_parameter(&params.key_mode);
                                setter.set_parameter(&params.key_mode, mode);
                                setter.end_set_parameter(&params.key_mode);
                            }
                            ui.add_space(8.0);
                            if ui.add(h_btn).clicked() {
                                setter.begin_set_parameter(&params.show_history);
                                setter.set_parameter(&params.show_history, !show_h);
                                setter.end_set_parameter(&params.show_history);
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                            ui.add_space(14.0); // Extreme right padding
                            ui.vertical(|ui| {
                                ui.add_space(2.0); // Push text down a bit to align with header items
                                let inv = &snapshot.chord_info.inversion;
                                if !inv.is_empty() && inv.to_lowercase() != "root pos" {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(
                                                egui::RichText::new(inv)
                                                    .color(Color32::from_rgb(100, 100, 110))
                                                    .font(FontId::new(
                                                        14.0,
                                                        FontFamily::Proportional,
                                                    )),
                                            );
                                        },
                                    );
                                }
                            });
                        });
                    });
                },
            );

            // ── Main Content Area ──
            ui.allocate_new_ui(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                    Pos2::new(full_rect.min.x, full_rect.min.y + 45.0),
                    Pos2::new(full_rect.max.x, full_rect.max.y - 75.0),
                )),
                |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.add_space(15.0);
                        let root_name = &snapshot.chord_info.root;
                        let root_pc = if root_name.is_empty() || root_name == "–" {
                            None
                        } else {
                            let mut found = None;
                            for p in 0..12 {
                                if root_name
                                    .starts_with(crate::chord::pc_name(p, snapshot.scale_root))
                                {
                                    found = Some(p);
                                }
                            }
                            found
                        };
                        let root_color = if let Some(p) = root_pc {
                            get_note_color(
                                p,
                                snapshot.scale_root,
                                &snapshot.scale_intervals,
                                snapshot.chromatic_mode,
                                &crate::chord::NoteRole::Root,
                            )
                        } else {
                            CHORD_TEXT
                        };
                        let mut job = egui::text::LayoutJob {
                            halign: egui::Align::Center,
                            ..Default::default()
                        };
                        let (root_note, root_octave) = if let Some(first_digit) =
                            root_name.find(|c: char| c.is_ascii_digit() || c == '-')
                        {
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
                            },
                        );
                        if !root_octave.is_empty() {
                            job.append(
                                root_octave,
                                0.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(128.0, FontFamily::Proportional),
                                    color: root_color,
                                    valign: egui::Align::Center,
                                    ..Default::default()
                                },
                            );
                        }
                        if !snapshot.chord_info.quality.is_empty() {
                            job.append(
                                &snapshot.chord_info.quality,
                                0.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(64.0, FontFamily::Proportional),
                                    color: Color32::from_rgb(180, 180, 195),
                                    valign: egui::Align::Center,
                                    ..Default::default()
                                },
                            );
                        }
                        if !snapshot.chord_info.omitted.is_empty() {
                            job.append(
                                &snapshot.chord_info.omitted,
                                4.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(32.0, FontFamily::Proportional),
                                    color: Color32::from_rgb(140, 140, 160),
                                    valign: egui::Align::TOP,
                                    ..Default::default()
                                },
                            );
                        }
                        if !snapshot.chord_info.slash.is_empty() {
                            job.append(
                                &snapshot.chord_info.slash,
                                2.0,
                                egui::text::TextFormat {
                                    font_id: FontId::new(72.0, FontFamily::Proportional),
                                    color: SLASH_TEXT,
                                    valign: egui::Align::BOTTOM,
                                    ..Default::default()
                                },
                            );
                        }
                        ui.label(job);
                    });
                },
            );

            // ── Footer Section ──
            ui.allocate_new_ui(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                    Pos2::new(full_rect.min.x, full_rect.max.y - 75.0),
                    Vec2::new(full_rect.width(), 75.0),
                )),
                |ui| {
                    if params.show_history.value() {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.add_space(6.0); // Right padding
                            ui.add_space(6.0); // Match the bottom margin to the right padding
                            let history = &snapshot.chord_history;
                            if !history.is_empty() {
                                let mut hist_job = egui::text::LayoutJob {
                                    halign: egui::Align::RIGHT,
                                    ..Default::default()
                                };
                                let hist_len = history.len();
                                let start_idx = hist_len.saturating_sub(6);
                                for (i, entry) in history.iter().enumerate().skip(start_idx) {
                                    let mut root_pc = None;
                                    for p in 0..12 {
                                        if entry.root.starts_with(crate::chord::pc_name(
                                            p,
                                            snapshot.scale_root,
                                        )) {
                                            root_pc = Some(p);
                                            break;
                                        }
                                    }
                                    let display_pos = i - start_idx;
                                    let total_visible = hist_len - start_idx;
                                    let opacity = if i == hist_len - 1 {
                                        255
                                    } else {
                                        (150.0 * (display_pos as f32 / total_visible as f32))
                                            .max(60.0) as u8
                                    };
                                    let base_color = if let Some(p) = root_pc {
                                        get_note_color(
                                            p,
                                            snapshot.scale_root,
                                            &snapshot.scale_intervals,
                                            snapshot.chromatic_mode,
                                            &crate::chord::NoteRole::Root,
                                        )
                                    } else {
                                        CHORD_TEXT
                                    };
                                    let color = base_color.gamma_multiply(opacity as f32 / 255.0);
                                    if i > start_idx {
                                        hist_job.append(
                                            " → ",
                                            0.0,
                                            egui::text::TextFormat {
                                                font_id: FontId::new(
                                                    14.0,
                                                    FontFamily::Proportional,
                                                ),
                                                color: DIVIDER,
                                                valign: egui::Align::Center,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                    hist_job.append(
                                        &format!("{}{}", entry.root, entry.quality),
                                        0.0,
                                        egui::text::TextFormat {
                                            font_id: FontId::new(20.0, FontFamily::Proportional),
                                            color,
                                            ..Default::default()
                                        },
                                    );
                                    if !entry.omitted.is_empty() {
                                        hist_job.append(
                                            &entry.omitted,
                                            2.0,
                                            egui::text::TextFormat {
                                                font_id: FontId::new(
                                                    12.0,
                                                    FontFamily::Proportional,
                                                ),
                                                color: color.gamma_multiply(0.7),
                                                valign: egui::Align::TOP,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                    if !entry.slash.is_empty() {
                                        hist_job.append(
                                            &entry.slash,
                                            0.0,
                                            egui::text::TextFormat {
                                                font_id: FontId::new(
                                                    16.0,
                                                    FontFamily::Proportional,
                                                ),
                                                color: color.gamma_multiply(0.8),
                                                valign: egui::Align::BOTTOM,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                }
                                ui.label(hist_job);
                            } else {
                                ui.label(
                                    RichText::new("no history yet")
                                        .font(FontId::new(14.0, FontFamily::Proportional))
                                        .color(INV_TEXT)
                                        .italics(),
                                );
                            }
                        });
                    } else {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            ui.add_space(8.0);
                            let notes = &snapshot.chord_info.active_notes;
                            if !notes.is_empty() {
                                let mut note_job = egui::text::LayoutJob {
                                    halign: egui::Align::Center,
                                    ..Default::default()
                                };
                                for (i, (name, role)) in notes.iter().enumerate() {
                                    let mut pc = 0;
                                    for p in 0..12 {
                                        if name.starts_with(crate::chord::pc_name(
                                            p,
                                            snapshot.scale_root,
                                        )) {
                                            pc = p;
                                        }
                                    }
                                    let color = get_note_color(
                                        pc,
                                        snapshot.scale_root,
                                        &snapshot.scale_intervals,
                                        snapshot.chromatic_mode,
                                        role,
                                    );
                                    let (note_part, octave_part) = if let Some(first_digit) =
                                        name.find(|c: char| c.is_ascii_digit() || c == '-')
                                    {
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
                                        },
                                    );
                                    if !octave_part.is_empty() {
                                        note_job.append(
                                            octave_part,
                                            0.0,
                                            egui::text::TextFormat {
                                                font_id: FontId::new(
                                                    28.0,
                                                    FontFamily::Proportional,
                                                ),
                                                color: if SHOW_GREY_OCTAVES {
                                                    NOTE_OFF
                                                } else {
                                                    color
                                                },
                                                ..Default::default()
                                            },
                                        );
                                    }
                                    if i < notes.len() - 1 {
                                        note_job.append(
                                            " · ",
                                            0.0,
                                            egui::text::TextFormat {
                                                font_id: FontId::new(
                                                    28.0,
                                                    FontFamily::Proportional,
                                                ),
                                                color: DIVIDER,
                                                ..Default::default()
                                            },
                                        );
                                    }
                                }
                                ui.label(note_job);
                            } else {
                                ui.label(
                                    RichText::new("no notes")
                                        .font(FontId::new(14.0, FontFamily::Proportional))
                                        .color(INV_TEXT),
                                );
                            }
                        });
                    }

                    if !snapshot.chromatic_mode
                        && params.show_nashville.value()
                        && !snapshot.nashville_text.is_empty()
                    {
                        let nash_rect = ui.available_rect_before_wrap();
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                                Pos2::new(nash_rect.min.x + 20.0, nash_rect.max.y - 64.0),
                                Pos2::new(nash_rect.max.x, nash_rect.max.y - 12.0),
                            )),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(&snapshot.nashville_text)
                                        .font(FontId::new(48.0, FontFamily::Proportional))
                                        .color(Color32::from_rgb(220, 220, 230)),
                                );
                            },
                        );
                    }

                    #[cfg(debug_assertions)]
                    if !snapshot.debug_key_diagnostics.is_empty() {
                        let debug_rect = ui.available_rect_before_wrap();
                        ui.allocate_new_ui(
                            egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                                Pos2::new(debug_rect.min.x + 12.0, debug_rect.min.y + 4.0),
                                Pos2::new(debug_rect.max.x - 12.0, debug_rect.max.y),
                            )),
                            |ui| {
                                ui.label(
                                    egui::RichText::new(&snapshot.debug_key_diagnostics)
                                        .font(FontId::new(11.0, FontFamily::Monospace))
                                        .color(Color32::from_rgb(110, 150, 170)),
                                );
                            },
                        );
                    }
                },
            );
        });
}
