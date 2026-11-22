//! In-app Set Automation (Macro Recorder & .djset.json Player) and Two-Room Autoplay UI.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use djcraft_audio::dsp::decode_audio_file;
use djcraft_audio::output::Room2AudioOutput;
use djcraft_core::ipc::{SetScript, SetStep};
use djcraft_core::{CrateTrack, IpcRequest};

use super::skins::SkinPalette;
use super::theme;

pub struct SetAutomationState {
    pub script: Option<SetScript>,
    pub script_path: String,
    pub playing: bool,
    pub speed: f64,
    pub started_at: Option<Instant>,
    pub next_step: usize,
    pub last_step_label: String,
    pub recording: bool,
    pub record_started_at: Option<Instant>,
    pub recorded_steps: Vec<SetStep>,
}

impl Default for SetAutomationState {
    fn default() -> Self {
        Self {
            script: None,
            script_path: "examples/demo_plex_set.djset.json".to_string(),
            playing: false,
            speed: 1.0,
            started_at: None,
            next_step: 0,
            last_step_label: "Idle — load a .djset.json or record your own macro.".to_string(),
            recording: false,
            record_started_at: None,
            recorded_steps: Vec::new(),
        }
    }
}

pub struct Room2AutoplayState {
    pub out: Room2AudioOutput,
    pub selected_device: Option<String>,
    pub crate_id: Option<i64>,
    pub tracks: Vec<CrateTrack>,
    pub current_idx: usize,
    pub enabled: bool,
    pub shuffle: bool,
    pub volume: f32,
    pub status: String,
}

impl Default for Room2AutoplayState {
    fn default() -> Self {
        Self {
            out: Room2AudioOutput::new(),
            selected_device: None,
            crate_id: None,
            tracks: Vec::new(),
            current_idx: 0,
            enabled: false,
            shuffle: false,
            volume: 0.85,
            status: "Room 2 idle — pick an output device and prepared crate.".to_string(),
        }
    }
}

impl super::DjCraftApp {
    pub(super) fn record_macro_step(&mut self, label: impl Into<String>, request: IpcRequest) {
        if !self.set_auto.recording {
            return;
        }
        let at_ms = self
            .set_auto
            .record_started_at
            .map_or(0, |t| t.elapsed().as_millis() as u64);
        self.set_auto.recorded_steps.push(SetStep {
            at_ms,
            label: Some(label.into()),
            request,
        });
    }

    pub(super) fn tick_set_automation(&mut self) {
        if !self.set_auto.playing {
            return;
        }
        let Some(ref script) = self.set_auto.script else {
            self.set_auto.playing = false;
            return;
        };
        let Some(started) = self.set_auto.started_at else {
            self.set_auto.playing = false;
            return;
        };

        let elapsed_ms = (started.elapsed().as_millis() as f64 * self.set_auto.speed) as u64;
        let mut to_run = Vec::new();
        while self.set_auto.next_step < script.steps.len()
            && script.steps[self.set_auto.next_step].at_ms <= elapsed_ms
        {
            to_run.push(script.steps[self.set_auto.next_step].clone());
            self.set_auto.next_step += 1;
        }

        let total_steps = script.steps.len();
        for step in to_run {
            let label = step
                .label
                .clone()
                .unwrap_or_else(|| format!("Step @ {}ms", step.at_ms));
            let resp = self.execute_ipc_request(step.request);
            self.set_auto.last_step_label = if resp.ok {
                label
            } else {
                format!("{label} (err: {})", resp.error.unwrap_or_default())
            };
        }

        if self.set_auto.next_step >= total_steps {
            self.set_auto.playing = false;
            self.set_auto.last_step_label = format!("Completed {total_steps} steps.");
        }
    }

    pub(super) fn tick_room2_autoplay(&mut self) {
        if !self.room2.enabled || self.room2.tracks.is_empty() {
            return;
        }
        let (playing, finished, _, _) = self.room2.out.snapshot();
        if !playing && finished {
            self.room2_play_next();
        }
    }

    fn room2_load_crate_tracks(&mut self, crate_id: i64) {
        self.room2.crate_id = Some(crate_id);
        self.room2.current_idx = 0;
        if let Some(ref store) = self.crate_store {
            let q = djcraft_core::CrateQuery {
                crate_id: Some(crate_id),
                ..Default::default()
            };
            match store.query_tracks(&q) {
                Ok(tracks) => {
                    self.room2.tracks = tracks;
                    self.room2.status = format!(
                        "Loaded {} track(s) for Room 2 autoplay.",
                        self.room2.tracks.len()
                    );
                }
                Err(e) => self.room2.status = format!("Crate query error: {e}"),
            }
        }
    }

    fn room2_play_index(&mut self, idx: usize) {
        if self.room2.tracks.is_empty() {
            self.room2.status = "No tracks in selected Room 2 crate.".to_string();
            return;
        }
        let safe_idx = idx % self.room2.tracks.len();
        self.room2.current_idx = safe_idx;
        let track = self.room2.tracks[safe_idx].clone();
        match decode_audio_file(Path::new(&track.path)) {
            Ok(decoded) => {
                self.room2.out.set_volume(self.room2.volume);
                self.room2.out.load_and_play(Arc::new(decoded));
                self.room2.enabled = true;
                self.room2.status = format!(
                    "Room 2 playing [{}/{}]: {} - {}",
                    safe_idx + 1,
                    self.room2.tracks.len(),
                    track.artist,
                    track.title
                );
            }
            Err(e) => {
                self.room2.status = format!("Decode error for '{}': {e}", track.title);
            }
        }
    }

    fn room2_play_next(&mut self) {
        if self.room2.tracks.is_empty() {
            return;
        }
        let next = if self.room2.shuffle && self.room2.tracks.len() > 1 {
            (self.room2.current_idx + 3) % self.room2.tracks.len()
        } else {
            (self.room2.current_idx + 1) % self.room2.tracks.len()
        };
        self.room2_play_index(next);
    }

    pub(crate) fn handle_macro_ipc(
        &mut self,
        action: &str,
        save_path: Option<&str>,
    ) -> djcraft_core::IpcResponse {
        match action.trim().to_lowercase().as_str() {
            "start" => {
                self.set_auto.recorded_steps.clear();
                self.set_auto.record_started_at = Some(Instant::now());
                self.set_auto.recording = true;
                self.set_auto.last_step_label =
                    "Recording live deck, FX, and mixer actions...".to_string();
                djcraft_core::IpcResponse::ok(serde_json::json!({
                    "recording": true,
                    "steps": 0
                }))
            }
            "stop" => {
                self.set_auto.recording = false;
                let steps = std::mem::take(&mut self.set_auto.recorded_steps);
                let sc = SetScript {
                    name: "Recorded DJCraft Set".to_string(),
                    description: Some("Recorded live in DJCraft".to_string()),
                    steps,
                };
                if let Some(p) = save_path.filter(|s| !s.trim().is_empty()) {
                    if let Ok(json) = serde_json::to_string_pretty(&sc) {
                        if let Err(e) = std::fs::write(p, json) {
                            return djcraft_core::IpcResponse::err(format!(
                                "Failed to write macro to '{p}': {e}"
                            ));
                        }
                    }
                }
                let step_count = sc.steps.len();
                let sc_json = serde_json::to_value(&sc).unwrap_or(serde_json::Value::Null);
                self.set_auto.script = Some(sc);
                self.set_auto.next_step = 0;
                djcraft_core::IpcResponse::ok(serde_json::json!({
                    "recording": false,
                    "steps": step_count,
                    "saved_to": save_path,
                    "script": sc_json
                }))
            }
            "status" => djcraft_core::IpcResponse::ok(serde_json::json!({
                "recording": self.set_auto.recording,
                "steps": self.set_auto.recorded_steps.len()
            })),
            other => djcraft_core::IpcResponse::err(format!(
                "Unknown macro action '{other}' (expected start, stop, status)"
            )),
        }
    }

    pub(crate) fn handle_set_playback_ipc(
        &mut self,
        action: &str,
        path: Option<&str>,
        speed: Option<f64>,
    ) -> djcraft_core::IpcResponse {
        if let Some(spd) = speed {
            self.set_auto.speed = spd.clamp(0.25, 10.0);
        }
        match action.trim().to_lowercase().as_str() {
            "start" => {
                if let Some(p) = path.filter(|s| !s.trim().is_empty()) {
                    match std::fs::read_to_string(p)
                        .map_err(|e| e.to_string())
                        .and_then(|raw| {
                            serde_json::from_str::<SetScript>(&raw).map_err(|e| e.to_string())
                        }) {
                        Ok(mut sc) => {
                            sc.steps.sort_by_key(|s| s.at_ms);
                            self.set_auto.script_path = p.to_string();
                            self.set_auto.script = Some(sc);
                        }
                        Err(e) => {
                            return djcraft_core::IpcResponse::err(format!(
                                "Failed to load set '{p}': {e}"
                            ));
                        }
                    }
                }
                let Some(ref sc) = self.set_auto.script else {
                    return djcraft_core::IpcResponse::err(
                        "No set loaded; pass --path <file.djset.json>".to_string(),
                    );
                };
                self.set_auto.next_step = 0;
                self.set_auto.started_at = Some(Instant::now());
                self.set_auto.playing = true;
                djcraft_core::IpcResponse::ok(serde_json::json!({
                    "playing": true,
                    "name": sc.name,
                    "total_steps": sc.steps.len(),
                    "speed": self.set_auto.speed
                }))
            }
            "stop" => {
                self.set_auto.playing = false;
                djcraft_core::IpcResponse::ok(serde_json::json!({
                    "playing": false,
                    "next_step": self.set_auto.next_step
                }))
            }
            "status" => djcraft_core::IpcResponse::ok(serde_json::json!({
                "playing": self.set_auto.playing,
                "speed": self.set_auto.speed,
                "next_step": self.set_auto.next_step,
                "total_steps": self.set_auto.script.as_ref().map_or(0, |s| s.steps.len()),
                "name": self.set_auto.script.as_ref().map(|s| s.name.clone()),
                "last_step_label": self.set_auto.last_step_label
            })),
            other => djcraft_core::IpcResponse::err(format!(
                "Unknown set-play action '{other}' (expected start, stop, status)"
            )),
        }
    }

    pub(crate) fn handle_room2_ipc(
        &mut self,
        action: &str,
        device: Option<&str>,
        crate_id: Option<i64>,
        volume: Option<f32>,
        shuffle: Option<bool>,
    ) -> djcraft_core::IpcResponse {
        if let Some(dev) = device {
            let target = if dev.trim().is_empty() || dev.eq_ignore_ascii_case("off") {
                None
            } else {
                Some(dev)
            };
            match self.room2.out.set_device(target) {
                Ok(msg) => {
                    self.room2.selected_device = target.map(ToString::to_string);
                    self.room2.status = msg;
                }
                Err(e) => return djcraft_core::IpcResponse::err(e),
            }
        }
        if let Some(cid) = crate_id {
            self.room2_load_crate_tracks(cid);
        }
        if let Some(vol) = volume {
            self.room2.volume = vol.clamp(0.0, 1.0);
            self.room2.out.set_volume(self.room2.volume);
        }
        if let Some(shuf) = shuffle {
            self.room2.shuffle = shuf;
        }

        match action.trim().to_lowercase().as_str() {
            "configure" | "status" => {}
            "play" => {
                let (_, _, _, dur_sec) = self.room2.out.snapshot();
                if dur_sec > 0.0 && crate_id.is_none() {
                    self.room2.out.set_playing(true);
                    self.room2.enabled = true;
                } else {
                    self.room2_play_index(self.room2.current_idx);
                }
            }
            "pause" => {
                self.room2.out.set_playing(false);
                self.room2.enabled = false;
            }
            "stop" => {
                self.room2.out.stop();
                self.room2.enabled = false;
            }
            "next" => self.room2_play_next(),
            other => {
                return djcraft_core::IpcResponse::err(format!(
                    "Unknown room2 action '{other}' (expected status, configure, play, pause, stop, next)"
                ));
            }
        }

        let (playing, finished, pos_sec, dur_sec) = self.room2.out.snapshot();
        djcraft_core::IpcResponse::ok(serde_json::json!({
            "enabled": self.room2.enabled,
            "playing": playing,
            "finished": finished,
            "pos_sec": pos_sec,
            "dur_sec": dur_sec,
            "device": self.room2.selected_device,
            "crate_id": self.room2.crate_id,
            "tracks_count": self.room2.tracks.len(),
            "current_idx": self.room2.current_idx,
            "volume": self.room2.volume,
            "shuffle": self.room2.shuffle,
            "status": self.room2.status
        }))
    }

    pub(super) fn render_set_and_rooms_dock(&mut self, ui: &mut egui::Ui, palette: &SkinPalette) {
        ui.columns(2, |cols| {
            // Left: Set Script Player & Macro Recorder
            theme::card_frame(&mut cols[0], palette, |ui| {
                ui.strong("Set Automation (Macro Recorder & .djset.json Player)");
                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.small_button("Load Demo: Plex Set").clicked() {
                        let p = Path::new("examples/demo_plex_set.djset.json");
                        match std::fs::read_to_string(p)
                            .map_err(|e| e.to_string())
                            .and_then(|s| {
                                serde_json::from_str::<SetScript>(&s).map_err(|e| e.to_string())
                            }) {
                            Ok(mut sc) => {
                                sc.steps.sort_by_key(|s| s.at_ms);
                                self.set_auto.script_path = p.display().to_string();
                                self.set_auto.last_step_label =
                                    format!("Loaded '{}' ({} steps)", sc.name, sc.steps.len());
                                self.set_auto.script = Some(sc);
                                self.set_auto.next_step = 0;
                                self.set_auto.playing = false;
                            }
                            Err(e) => {
                                self.set_auto.last_step_label = format!("Load failed: {e}");
                            }
                        }
                    }
                    if ui.small_button("Open .djset.json...").clicked() {
                        if let Some(p) = rfd::FileDialog::new()
                            .add_filter("DJCraft Set", &["json", "djset.json"])
                            .pick_file()
                        {
                            match std::fs::read_to_string(&p)
                                .map_err(|e| e.to_string())
                                .and_then(|s| {
                                    serde_json::from_str::<SetScript>(&s).map_err(|e| e.to_string())
                                }) {
                                Ok(mut sc) => {
                                    sc.steps.sort_by_key(|s| s.at_ms);
                                    self.set_auto.script_path = p.display().to_string();
                                    self.set_auto.last_step_label =
                                        format!("Loaded '{}' ({} steps)", sc.name, sc.steps.len());
                                    self.set_auto.script = Some(sc);
                                    self.set_auto.next_step = 0;
                                    self.set_auto.playing = false;
                                }
                                Err(e) => {
                                    self.set_auto.last_step_label = format!("Load failed: {e}");
                                }
                            }
                        }
                    }
                });

                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    let has_script = self.set_auto.script.is_some();
                    if self.set_auto.playing {
                        if ui.button("Stop Set").clicked() {
                            self.set_auto.playing = false;
                            self.set_auto.last_step_label = "Set playback stopped.".to_string();
                        }
                    } else if ui
                        .add_enabled(has_script, egui::Button::new("Play Set"))
                        .clicked()
                    {
                        self.set_auto.next_step = 0;
                        self.set_auto.started_at = Some(Instant::now());
                        self.set_auto.playing = true;
                        self.set_auto.last_step_label = "Playing set...".to_string();
                    }

                    ui.label("Speed:");
                    for (lbl, spd) in [("0.5x", 0.5), ("1.0x", 1.0), ("2.0x", 2.0)] {
                        let active = (self.set_auto.speed - spd).abs() < 0.05;
                        if ui.selectable_label(active, lbl).clicked() {
                            self.set_auto.speed = spd;
                        }
                    }
                });

                if let Some(ref sc) = self.set_auto.script {
                    let total = sc.steps.len().max(1);
                    let frac = (self.set_auto.next_step as f32 / total as f32).clamp(0.0, 1.0);
                    ui.add(egui::ProgressBar::new(frac).text(format!(
                        "{} / {} steps ({})",
                        self.set_auto.next_step,
                        sc.steps.len(),
                        sc.name
                    )));
                }
                ui.small(&self.set_auto.last_step_label);

                ui.separator();
                ui.horizontal_wrapped(|ui| {
                    if self.set_auto.recording {
                        if ui.button("Stop & Save Macro...").clicked() {
                            self.set_auto.recording = false;
                            let steps = std::mem::take(&mut self.set_auto.recorded_steps);
                            let sc = SetScript {
                                name: "Recorded DJCraft Set".to_string(),
                                description: Some("Recorded live in DJCraft UI".to_string()),
                                steps,
                            };
                            if let Some(p) = rfd::FileDialog::new()
                                .set_file_name("my_set.djset.json")
                                .save_file()
                            {
                                if let Ok(json) = serde_json::to_string_pretty(&sc) {
                                    let _ = std::fs::write(&p, json);
                                    self.set_auto.last_step_label = format!(
                                        "Saved {} recorded steps to {}",
                                        sc.steps.len(),
                                        p.display()
                                    );
                                }
                            }
                            self.set_auto.script = Some(sc);
                            self.set_auto.next_step = 0;
                        }
                        ui.colored_label(
                            egui::Color32::from_rgb(235, 64, 84),
                            format!("REC ({} steps)", self.set_auto.recorded_steps.len()),
                        );
                    } else if ui.small_button("REC Macro").clicked() {
                        self.set_auto.recorded_steps.clear();
                        self.set_auto.record_started_at = Some(Instant::now());
                        self.set_auto.recording = true;
                        self.set_auto.last_step_label =
                            "Recording live deck, FX, and mixer actions...".to_string();
                    }
                });
            });

            // Right: Two-Room Setup (Room 1 Live DJ | Room 2 Prepared Autoplay)
            theme::card_frame(&mut cols[1], palette, |ui| {
                ui.strong("Two-Room Setup (Room 1: Live DJ | Room 2: Autoplay)");
                ui.add_space(4.0);
                ui.small(format!(
                    "Room 1 (Main Floor): Decks A-D -> {}",
                    self.audio_out.device_name
                ));

                ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Room 2 Output:");
                    let current_dev = self
                        .room2
                        .selected_device
                        .clone()
                        .unwrap_or_else(|| "Off".to_string());
                    let mut picked: Option<Option<String>> = None;
                    egui::ComboBox::from_id_salt("room2_out_combo")
                        .selected_text(current_dev)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(self.room2.selected_device.is_none(), "Off")
                                .clicked()
                            {
                                picked = Some(None);
                            }
                            for dev in &self.audio_output_devices {
                                let sel =
                                    self.room2.selected_device.as_deref() == Some(dev.as_str());
                                if ui.selectable_label(sel, dev).clicked() {
                                    picked = Some(Some(dev.clone()));
                                }
                            }
                        });
                    if let Some(dev_opt) = picked {
                        match self.room2.out.set_device(dev_opt.as_deref()) {
                            Ok(msg) => {
                                self.room2.selected_device = dev_opt;
                                self.room2.status = msg;
                            }
                            Err(e) => self.room2.status = e,
                        }
                    }
                });

                ui.horizontal_wrapped(|ui| {
                    ui.label("Room 2 Crate:");
                    let crate_label = self
                        .room2
                        .crate_id
                        .and_then(|cid| {
                            self.crate_nodes
                                .iter()
                                .find(|c| c.id == cid)
                                .map(|c| c.name.clone())
                        })
                        .unwrap_or_else(|| "Select Crate...".to_string());
                    let mut pick_crate: Option<i64> = None;
                    egui::ComboBox::from_id_salt("room2_crate_combo")
                        .selected_text(crate_label)
                        .show_ui(ui, |ui| {
                            for node in &self.crate_nodes {
                                let sel = self.room2.crate_id == Some(node.id);
                                if ui
                                    .selectable_label(
                                        sel,
                                        format!("{} ({})", node.name, node.track_count),
                                    )
                                    .clicked()
                                {
                                    pick_crate = Some(node.id);
                                }
                            }
                        });
                    if let Some(cid) = pick_crate {
                        self.room2_load_crate_tracks(cid);
                    }
                });

                let (is_playing, _, pos_sec, dur_sec) = self.room2.out.snapshot();
                ui.horizontal_wrapped(|ui| {
                    if is_playing {
                        if ui.button("Pause Room 2").clicked() {
                            self.room2.out.set_playing(false);
                            self.room2.enabled = false;
                        }
                    } else if ui
                        .add_enabled(
                            !self.room2.tracks.is_empty(),
                            egui::Button::new("Play Room 2"),
                        )
                        .clicked()
                    {
                        if dur_sec > 0.0 {
                            self.room2.out.set_playing(true);
                            self.room2.enabled = true;
                        } else {
                            self.room2_play_index(self.room2.current_idx);
                        }
                    }
                    if ui
                        .add_enabled(!self.room2.tracks.is_empty(), egui::Button::new("Next"))
                        .clicked()
                    {
                        self.room2_play_next();
                    }
                    if ui.button("Stop").clicked() {
                        self.room2.out.stop();
                        self.room2.enabled = false;
                    }
                    ui.checkbox(&mut self.room2.shuffle, "Shuffle");
                });

                ui.horizontal(|ui| {
                    ui.label("Room 2 Vol:");
                    if ui
                        .add(egui::Slider::new(&mut self.room2.volume, 0.0..=1.0).show_value(false))
                        .changed()
                    {
                        self.room2.out.set_volume(self.room2.volume);
                    }
                });

                if dur_sec > 0.0 {
                    let frac = (pos_sec / dur_sec).clamp(0.0, 1.0) as f32;
                    ui.add(
                        egui::ProgressBar::new(frac)
                            .text(format!("{:.0}s / {:.0}s", pos_sec, dur_sec)),
                    );
                }
                ui.small(&self.room2.status);
            });
        });
    }
}
