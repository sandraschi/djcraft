#![forbid(unsafe_code)]
//! Dedicated File-Oriented Crate & Gig-Prep Workspace for DJCraft.
//!
//! 2-pane file-manager layout:
//! - Left pane: Hierarchical Crate / Subcrate tree with full CRUD (+ Crate, + Subcrate,
//!   Rename, Duplicate, Delete) and drag-drop target support (drop any track row onto a crate).
//! - Right pane: Gig-prep track table with drag-and-drop row reordering (`::` handle),
//!   up/down order buttons, OS Windows Explorer file drop, multi-field sort/filter,
//!   star ratings, Serato/VirtualDJ import-export, and batch audio/stem analysis.

use std::path::Path;

use djcraft_audio::dsp::has_sidecar_stems;
use djcraft_audio::DeckId;
use djcraft_core::{CrateNode, CrateQuery, CrateSortField, CrateTrack};
use egui::{Color32, CornerRadius, Sense, Stroke, StrokeKind};

use crate::skins::SkinPalette;
use crate::theme;

#[derive(Debug, Default)]
pub struct CrateUiState {
    pub new_crate_name: String,
    pub rename_buf: String,
    pub renaming_crate_id: Option<i64>,
    pub dragging_track_id: Option<i64>,
    pub bpm_min_text: String,
    pub bpm_max_text: String,
    pub harmonic_deck_a: bool,
}

pub enum CrateAction {
    SelectCrate(Option<i64>),
    CreateCrate {
        parent_id: Option<i64>,
        name: String,
    },
    RenameCrate {
        crate_id: i64,
        new_name: String,
    },
    DuplicateCrate {
        crate_id: i64,
    },
    DeleteCrate {
        crate_id: i64,
    },
    AddFilesDialog,
    ScanFolderDialog,
    ImportSeratoDialog,
    ExportSeratoDialog,
    ImportVdjDialog,
    ExportVdjDialog,
    BatchStemActiveCrate,
    BatchStemFolderDialog,
    LoadToDeck {
        deck: DeckId,
        track: CrateTrack,
    },
    CopyTrackToCrate {
        crate_id: i64,
        track_id: i64,
    },
    ReorderTrack {
        crate_id: i64,
        track_id: i64,
        new_index: usize,
    },
    MoveTrackBy {
        crate_id: i64,
        track_id: i64,
        delta: isize,
    },
    RemoveTrackFromCrate {
        crate_id: i64,
        track_id: i64,
    },
    DeleteTrack {
        track_id: i64,
    },
    SetRating {
        track_id: i64,
        rating: i32,
    },
    AnalyzeBuiltin {
        track_id: i64,
    },
    AnalyzeDettato {
        track_id: i64,
    },
    QueryChanged,
}

#[allow(clippy::too_many_arguments)]
pub fn render_gig_prep_workspace(
    ui: &mut egui::Ui,
    palette: &SkinPalette,
    ui_state: &mut CrateUiState,
    query: &mut CrateQuery,
    nodes: &[CrateNode],
    tracks: &[CrateTrack],
    deck_a_key: Option<&str>,
) -> Vec<CrateAction> {
    let mut actions = Vec::new();

    // 1. TOP FILE-ORIENTED GIG-PREP TOOLBAR
    theme::card_frame(ui, palette, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.strong(egui::RichText::new("CRATE & GIG PREP WORKSPACE").color(palette.deck_a));
            ui.separator();
            if ui.button("+ Add Audio Files...").clicked() {
                actions.push(CrateAction::AddFilesDialog);
            }
            if ui.button("Scan Folder...").clicked() {
                actions.push(CrateAction::ScanFolderDialog);
            }
            ui.separator();
            if ui.button("Import Serato .crate").clicked() {
                actions.push(CrateAction::ImportSeratoDialog);
            }
            if ui
                .add_enabled(query.crate_id.is_some(), egui::Button::new("Export Serato"))
                .clicked()
            {
                actions.push(CrateAction::ExportSeratoDialog);
            }
            if ui.button("Import VDJ / M3U").clicked() {
                actions.push(CrateAction::ImportVdjDialog);
            }
            if ui
                .add_enabled(query.crate_id.is_some(), egui::Button::new("Export VDJ"))
                .clicked()
            {
                actions.push(CrateAction::ExportVdjDialog);
            }
            ui.separator();
            if ui
                .add_enabled(
                    query.crate_id.is_some(),
                    egui::Button::new("Batch Stem Crate"),
                )
                .on_hover_text("Separate stems into transparent .djcraft-stems/ beside each file")
                .clicked()
            {
                actions.push(CrateAction::BatchStemActiveCrate);
            }
            if ui.button("Batch Stem Folder...").clicked() {
                actions.push(CrateAction::BatchStemFolderDialog);
            }
        });
    });

    ui.add_space(6.0);

    let mut two_pane_actions = render_crate_two_pane_browser(
        ui, palette, ui_state, query, nodes, tracks, deck_a_key, false,
    );
    actions.append(&mut two_pane_actions);
    actions
}

/// Reusable 2-pane crate browser (Left Crate/Subcrate sidebar + Right Gig-Prep track table).
/// Used both in full-screen `CratesGigPrep` and in the lower half of `BoothAndCrate`.
#[allow(clippy::too_many_arguments)]
pub fn render_crate_two_pane_browser(
    ui: &mut egui::Ui,
    palette: &SkinPalette,
    ui_state: &mut CrateUiState,
    query: &mut CrateQuery,
    nodes: &[CrateNode],
    tracks: &[CrateTrack],
    deck_a_key: Option<&str>,
    compact: bool,
) -> Vec<CrateAction> {
    let mut actions = Vec::new();
    let avail_h = ui
        .available_height()
        .max(if compact { 180.0 } else { 320.0 });
    let sidebar_w = if compact { 235.0 } else { 270.0 };
    ui.horizontal_top(|ui| {
        // LEFT PANE: Hierarchical Crate / Subcrate Tree + Full CRUD
        ui.allocate_ui_with_layout(
            egui::vec2(sidebar_w, avail_h),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                theme::card_frame(ui, palette, |ui| {
                    ui.strong("Crates & Subcrates");
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut ui_state.new_crate_name)
                                .desired_width(135.0)
                                .hint_text("New crate name..."),
                        );
                        if ui.small_button("+ Root").clicked()
                            && !ui_state.new_crate_name.trim().is_empty()
                        {
                            actions.push(CrateAction::CreateCrate {
                                parent_id: None,
                                name: ui_state.new_crate_name.trim().to_string(),
                            });
                            ui_state.new_crate_name.clear();
                        }
                        if ui
                            .add_enabled(
                                query.crate_id.is_some()
                                    && !ui_state.new_crate_name.trim().is_empty(),
                                egui::Button::new("+ Sub").small(),
                            )
                            .clicked()
                        {
                            actions.push(CrateAction::CreateCrate {
                                parent_id: query.crate_id,
                                name: ui_state.new_crate_name.trim().to_string(),
                            });
                            ui_state.new_crate_name.clear();
                        }
                    });

                    // Active crate CRUD bar
                    if let Some(active_cid) = query.crate_id {
                        ui.horizontal(|ui| {
                            if ui_state.renaming_crate_id == Some(active_cid) {
                                ui.add(
                                    egui::TextEdit::singleline(&mut ui_state.rename_buf)
                                        .desired_width(120.0),
                                );
                                if ui.small_button("Save").clicked()
                                    && !ui_state.rename_buf.trim().is_empty()
                                {
                                    actions.push(CrateAction::RenameCrate {
                                        crate_id: active_cid,
                                        new_name: ui_state.rename_buf.trim().to_string(),
                                    });
                                    ui_state.renaming_crate_id = None;
                                }
                                if ui.small_button("X").clicked() {
                                    ui_state.renaming_crate_id = None;
                                }
                            } else {
                                if ui.small_button("Rename").clicked() {
                                    if let Some(n) = nodes.iter().find(|n| n.id == active_cid) {
                                        ui_state.rename_buf = n.name.clone();
                                        ui_state.renaming_crate_id = Some(active_cid);
                                    }
                                }
                                if ui.small_button("Duplicate").clicked() {
                                    actions.push(CrateAction::DuplicateCrate {
                                        crate_id: active_cid,
                                    });
                                }
                                if ui.small_button("Delete").clicked() {
                                    actions.push(CrateAction::DeleteCrate {
                                        crate_id: active_cid,
                                    });
                                }
                            }
                        });
                    }

                    ui.separator();
                    if ui_state.dragging_track_id.is_some() {
                        ui.label(
                            egui::RichText::new("Drop track onto any crate below to add it!")
                                .size(11.0)
                                .color(Color32::from_rgb(250, 204, 21)),
                        );
                    }

                    egui::ScrollArea::vertical()
                        .id_salt("crate_tree_scroll")
                        .max_height((avail_h - 130.0).max(160.0))
                        .show(ui, |ui| {
                            if ui
                                .selectable_label(query.crate_id.is_none(), "[ALL TRACKS]")
                                .clicked()
                            {
                                actions.push(CrateAction::SelectCrate(None));
                            }
                            for node in nodes {
                                let depth = node.full_path.matches('/').count();
                                let indent = "  ".repeat(depth);
                                let prefix = if depth > 0 { "|_ " } else { "" };
                                let label =
                                    format!("{indent}{prefix}{} ({})", node.name, node.track_count);
                                let selected = query.crate_id == Some(node.id);
                                let resp = ui.selectable_label(selected, label);
                                if resp.clicked() {
                                    actions.push(CrateAction::SelectCrate(Some(node.id)));
                                }
                                // Drag-and-drop track onto crate node in tree
                                if let Some(drag_tid) = ui_state.dragging_track_id {
                                    if resp.hovered() {
                                        ui.painter().rect_stroke(
                                            resp.rect,
                                            CornerRadius::same(4),
                                            Stroke::new(2.0, palette.deck_a),
                                            StrokeKind::Inside,
                                        );
                                        if ui.input(|i| i.pointer.any_released()) {
                                            actions.push(CrateAction::CopyTrackToCrate {
                                                crate_id: node.id,
                                                track_id: drag_tid,
                                            });
                                        }
                                    }
                                }
                            }
                        });
                });
            },
        );

        ui.add_space(6.0);

        // RIGHT PANE: Filter/Sort Bar + Gig-Prep Track Table with Drag-Drop Reordering
        ui.vertical(|ui| {
            theme::card_frame(ui, palette, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label("Search:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut query.search)
                                .desired_width(160.0)
                                .hint_text("Title, artist, genre, path..."),
                        )
                        .changed()
                    {
                        actions.push(CrateAction::QueryChanged);
                    }
                    ui.label("BPM:");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut ui_state.bpm_min_text)
                                .desired_width(42.0)
                                .hint_text("min"),
                        )
                        .changed()
                    {
                        actions.push(CrateAction::QueryChanged);
                    }
                    ui.label("-");
                    if ui
                        .add(
                            egui::TextEdit::singleline(&mut ui_state.bpm_max_text)
                                .desired_width(42.0)
                                .hint_text("max"),
                        )
                        .changed()
                    {
                        actions.push(CrateAction::QueryChanged);
                    }
                    let harm_lbl = match deck_a_key {
                        Some(k) => format!("Harmonic ({k})"),
                        None => "Harmonic A".to_string(),
                    };
                    if ui.checkbox(&mut ui_state.harmonic_deck_a, harm_lbl).changed() {
                        actions.push(CrateAction::QueryChanged);
                    }
                    if ui.checkbox(&mut query.include_subcrates, "Subcrates").changed() {
                        actions.push(CrateAction::QueryChanged);
                    }
                    if ui.checkbox(&mut query.analyzed_only, "Analyzed").changed() {
                        actions.push(CrateAction::QueryChanged);
                    }
                    egui::ComboBox::from_id_salt("gig_prep_sort")
                        .selected_text(format!("Sort: {}", query.sort_by.label()))
                        .show_ui(ui, |ui| {
                            for f in CrateSortField::ALL {
                                if ui.selectable_value(&mut query.sort_by, f, f.label()).changed() {
                                    actions.push(CrateAction::QueryChanged);
                                }
                            }
                        });
                    if ui
                        .small_button(if query.sort_asc { "ASC" } else { "DESC" })
                        .clicked()
                    {
                        query.sort_asc = !query.sort_asc;
                        actions.push(CrateAction::QueryChanged);
                    }
                    ui.monospace(format!("{} track(s)", tracks.len()));
                });

                ui.label(
                    egui::RichText::new(
                        "Tip: Drag the [::] handle on any row to reorder your gig setlist or drop onto a crate on the left. You can also drag MP3/WAV/FLAC files straight from Windows Explorer into this window.",
                    )
                    .size(11.0)
                    .color(palette.text_secondary),
                );
                ui.separator();

                // Column Header
                ui.horizontal(|ui| {
                    ui.monospace(format!(
                        "{:<5} {:<26} {:<18} {:<5} {:<7} {:<5} {:<6} {}",
                        "#", "TITLE", "ARTIST", "KEY", "BPM", "STEM", "RATE", "ACTIONS"
                    ));
                });
                ui.separator();

                let active_cid = query.crate_id;
                let mut drop_reorder: Option<(i64, usize)> = None;

                egui::ScrollArea::vertical()
                    .id_salt("crate_tracks_table")
                    .max_height((avail_h - 145.0).max(160.0))
                    .show(ui, |ui| {
                        if tracks.is_empty() {
                            ui.label("No tracks in this view. Drag files from Explorer or click '+ Add Audio Files...'.");
                        }
                        for (row_idx, t) in tracks.iter().enumerate() {
                            let row_resp = ui.horizontal(|ui| {
                                // Drag handle [::]
                                let (hrect, hresp) = ui.allocate_exact_size(
                                    egui::vec2(26.0, 20.0),
                                    Sense::click_and_drag(),
                                );
                                ui.painter().text(
                                    hrect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "::",
                                    egui::FontId::monospace(12.0),
                                    if ui_state.dragging_track_id == Some(t.id) {
                                        palette.deck_a
                                    } else {
                                        palette.text_secondary
                                    },
                                );
                                if hresp.drag_started() {
                                    ui_state.dragging_track_id = Some(t.id);
                                }

                                // Quick Up / Down order buttons when inside a crate
                                if let Some(cid) = active_cid {
                                    if ui.small_button("^").on_hover_text("Move up").clicked() {
                                        actions.push(CrateAction::MoveTrackBy {
                                            crate_id: cid,
                                            track_id: t.id,
                                            delta: -1,
                                        });
                                    }
                                    if ui.small_button("v").on_hover_text("Move down").clicked() {
                                        actions.push(CrateAction::MoveTrackBy {
                                            crate_id: cid,
                                            track_id: t.id,
                                            delta: 1,
                                        });
                                    }
                                }

                                let key = if t.musical_key.is_empty() { "--" } else { t.musical_key.as_str() };
                                let bpm = if t.bpm > 0.0 {
                                    format!("{:.1}", t.bpm)
                                } else {
                                    "---".to_string()
                                };
                                let stems_badge = if !t.path.starts_with("demo://")
                                    && has_sidecar_stems(Path::new(&t.path))
                                {
                                    "4-ST"
                                } else {
                                    "----"
                                };
                                let title_short: String = t.title.chars().take(24).collect();
                                let artist_short: String = t.artist.chars().take(16).collect();
                                ui.monospace(format!(
                                    "{:<3} {:<24} {:<16} {:<4} {:<6} {:<4}",
                                    row_idx + 1,
                                    title_short,
                                    artist_short,
                                    key,
                                    bpm,
                                    stems_badge
                                ));

                                // Star rating cycle (0..5)
                                let rate_lbl = if t.rating > 0 {
                                    format!("{}*", t.rating)
                                } else {
                                    "-*".to_string()
                                };
                                if ui.small_button(rate_lbl).on_hover_text("Cycle rating 0-5").clicked() {
                                    actions.push(CrateAction::SetRating {
                                        track_id: t.id,
                                        rating: (t.rating + 1) % 6,
                                    });
                                }

                                // Load to Decks A/B/C/D
                                for (lbl, deck_id) in [
                                    ("A", DeckId::DeckA),
                                    ("B", DeckId::DeckB),
                                    ("C", DeckId::DeckC),
                                    ("D", DeckId::DeckD),
                                ] {
                                    if ui.small_button(lbl).clicked() {
                                        actions.push(CrateAction::LoadToDeck {
                                            deck: deck_id,
                                            track: t.clone(),
                                        });
                                    }
                                }

                                if !t.path.starts_with("demo://") {
                                    if ui.small_button("BPM").clicked() {
                                        actions.push(CrateAction::AnalyzeBuiltin { track_id: t.id });
                                    }
                                    if ui.small_button("Det").on_hover_text("Run dettato sidecar").clicked() {
                                        actions.push(CrateAction::AnalyzeDettato { track_id: t.id });
                                    }
                                }

                                if let Some(cid) = active_cid {
                                    if ui.small_button("Rem").on_hover_text("Remove from this crate").clicked() {
                                        actions.push(CrateAction::RemoveTrackFromCrate {
                                            crate_id: cid,
                                            track_id: t.id,
                                        });
                                    }
                                } else if ui.small_button("Del").on_hover_text("Delete from library").clicked() {
                                    actions.push(CrateAction::DeleteTrack { track_id: t.id });
                                }
                            });

                            // Highlight drop target row when dragging to reorder
                            if let Some(drag_tid) = ui_state.dragging_track_id {
                                if drag_tid != t.id && row_resp.response.hovered() {
                                    ui.painter().line_segment(
                                        [row_resp.response.rect.left_top(), row_resp.response.rect.right_top()],
                                        Stroke::new(2.5, palette.deck_a),
                                    );
                                    if ui.input(|i| i.pointer.any_released()) {
                                        drop_reorder = Some((drag_tid, row_idx));
                                    }
                                }
                            }
                        }
                    });

                if let (Some(cid), Some((drag_tid, target_idx))) = (active_cid, drop_reorder) {
                    actions.push(CrateAction::ReorderTrack {
                        crate_id: cid,
                        track_id: drag_tid,
                        new_index: target_idx,
                    });
                }
                if ui.input(|i| i.pointer.any_released()) {
                    ui_state.dragging_track_id = None;
                }
            });
        });
    });

    actions
}
