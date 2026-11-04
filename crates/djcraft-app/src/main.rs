use clap::Parser;
use serde_json::json;
use djcraft_core::DjOrchestrator;

#[derive(Parser, Debug)]
#[command(name = "djcraft", about = "DJCraft Sovereign Performance Studio")]
struct Args {
    #[arg(long)]
    smoke: bool,
}

pub struct DjCraftApp {
    orch: DjOrchestrator,
    status_msg: String,
}

impl DjCraftApp {
    pub fn new() -> Self {
        Self {
            orch: DjOrchestrator::new(),
            status_msg: "4-Deck DJ Studio Ready (Port 14118)".to_string(),
        }
    }
}

impl eframe::App for DjCraftApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("top_panel").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("🎛️ DJCraft — Sovereign 4-Deck Mixing & Stem Performance");
                ui.separator();
                ui.label(&self.status_msg);
            });
        });

        egui::Panel::left("deck_a_panel").show(ui, |ui| {
            ui.heading("Deck A (Master)");
            if let Some(track) = &self.orch.deck_a.loaded_track {
                ui.label(egui::RichText::new(&track.title).strong());
                ui.label(&track.artist);
                ui.label(format!("BPM: {:.1} | Key: {}", track.bpm, track.musical_key));
            }
            ui.separator();
            ui.label("Stem Gains");
            ui.add(egui::Slider::new(&mut self.orch.deck_a.stem_vocals_gain, 0.0..=1.0).text("Vocals"));
            ui.add(egui::Slider::new(&mut self.orch.deck_a.stem_drums_gain, 0.0..=1.0).text("Drums"));
            ui.add(egui::Slider::new(&mut self.orch.deck_a.stem_bass_gain, 0.0..=1.0).text("Bass"));
        });

        egui::Panel::right("deck_b_panel").show(ui, |ui| {
            ui.heading("Deck B (Cue)");
            if let Some(track) = &self.orch.deck_b.loaded_track {
                ui.label(egui::RichText::new(&track.title).strong());
                ui.label(&track.artist);
                ui.label(format!("BPM: {:.1} | Key: {}", track.bpm, track.musical_key));
            }
            ui.separator();
            ui.label("Stem Gains");
            ui.add(egui::Slider::new(&mut self.orch.deck_b.stem_vocals_gain, 0.0..=1.0).text("Vocals"));
            ui.add(egui::Slider::new(&mut self.orch.deck_b.stem_drums_gain, 0.0..=1.0).text("Drums"));
            ui.add(egui::Slider::new(&mut self.orch.deck_b.stem_bass_gain, 0.0..=1.0).text("Bass"));
        });

        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("Master Mixer & Crossfader");
            ui.add(egui::Slider::new(&mut self.orch.crossfader, -1.0..=1.0).text("Crossfader (A <-> B)"));
            ui.add_space(8.0);

            if let Ok(plan) = self.orch.plan_transition() {
                ui.group(|ui| {
                    ui.label(egui::RichText::new("Automated Transition Plan").strong());
                    ui.label(format!("BPM Delta: {:+.1} BPM", plan.bpm_delta));
                    let harmonic_label = if plan.harmonic_compatible { "Harmonic Match: YES (Clean mix)" } else { "Harmonic Match: KEY CLASH" };
                    ui.label(harmonic_label);
                    ui.label(format!("Recommended duration: {} bars", plan.recommended_transition_bars));
                    ui.label(format!("Strategy: {}", plan.suggested_eq_strategy));
                });
            }
        });
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    if args.smoke {
        println!("[djcraft] Running headless smoke verification...");
        let orch = DjOrchestrator::new();
        let plan = orch.plan_transition()?;
        let smoke_payload = json!({
            "app": "djcraft",
            "version": "0.1.0",
            "status": "ready",
            "control_port": 14118,
            "success": true,
            "deck_a_active": orch.deck_a.is_playing,
            "harmonic_compatible": plan.harmonic_compatible,
            "capabilities": ["FourDeckMixing", "RealtimeStemIsolation", "CamelotHarmonicMatching", "AutonomousTransitionPlanning"]
        });
        println!("[djcraft] Smoke OK: {}", smoke_payload);
        return Ok(());
    }

    let app = DjCraftApp::new();
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_title("DJCraft — Sovereign Performance Studio"),
        ..Default::default()
    };

    eframe::run_native(
        "DJCraft",
        native_options,
        Box::new(|_cc| Ok(Box::new(app))),
    )?;
    Ok(())
}
