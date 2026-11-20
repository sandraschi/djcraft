use std::path::Path;

use djcraft_audio::{AudioEngine, DeckId, TrackMeta};
use djcraft_core::{parse_dettato_json, Command, DjOrchestrator};

#[test]
fn test_camelot_wheel_compatible_pairs() {
    assert!(AudioEngine::is_camelot_compatible("8A", "8A"));
    assert!(AudioEngine::is_camelot_compatible("8A", "8B"));
    assert!(AudioEngine::is_camelot_compatible("8A", "9A"));
    assert!(AudioEngine::is_camelot_compatible("8A", "7A"));
    assert!(AudioEngine::is_camelot_compatible("12A", "1A"));
    assert!(AudioEngine::is_camelot_compatible("1B", "12B"));
}

#[test]
fn test_camelot_wheel_rejects_clashes_and_invalid_keys() {
    assert!(!AudioEngine::is_camelot_compatible("8A", "10A"));
    assert!(!AudioEngine::is_camelot_compatible("8A", "9B"));
    assert!(!AudioEngine::is_camelot_compatible("13A", "1A"));
    assert!(!AudioEngine::is_camelot_compatible("0A", "1A"));
    assert!(!AudioEngine::is_camelot_compatible("", "8A"));
    assert!(!AudioEngine::is_camelot_compatible("8C", "8A"));
}

#[test]
fn test_camelot_compatible_keys_list() {
    let keys = AudioEngine::compatible_camelot_keys("1A");
    assert_eq!(keys, vec!["1A", "1B", "12A", "2A"]);
    assert!(AudioEngine::compatible_camelot_keys("invalid").is_empty());
}

#[test]
fn test_camelot_case_and_whitespace_tolerance() {
    assert!(AudioEngine::is_camelot_compatible(" 8a ", "9A"));
    assert_eq!(AudioEngine::parse_camelot("12b"), Some((12, 'B')));
}

#[test]
fn test_transition_planner_default_tracks() {
    let orch = DjOrchestrator::new();
    let plan = orch.plan_transition().expect("plan transition");
    assert!(plan.harmonic_compatible);
    assert_eq!(plan.bpm_delta, 2.0);
    assert_eq!(plan.recommended_transition_bars, 32);
}

#[test]
fn test_transition_planner_detects_key_clash() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::LoadTrack {
        deck: DeckId::DeckB,
        track: TrackMeta {
            title: "Clash Track".to_string(),
            artist: "Test".to_string(),
            bpm: 135.0,
            musical_key: "3B".to_string(),
            duration_seconds: 240.0,
        },
    })
    .expect("load deck b");
    let plan = orch.plan_transition().expect("plan");
    assert!(!plan.harmonic_compatible);
    assert_eq!(plan.recommended_transition_bars, 16);
    assert!(plan.suggested_eq_strategy.contains("Key clash"));
}

#[test]
fn test_audio_pcm_synthesizer_produces_signal() {
    let orch = DjOrchestrator::new();
    let pcm = AudioEngine::synthesize_deck_audio(&orch.deck_a, 0.5, 44100);
    assert_eq!(pcm.len(), 44100);
    assert!(pcm.iter().any(|&s| s != 0));
}

#[test]
fn test_audio_pcm_synthesizer_silent_when_paused_or_empty() {
    let mut deck = AudioEngine::create_deck(DeckId::DeckD);
    let empty_pcm = AudioEngine::synthesize_deck_audio(&deck, 0.1, 44100);
    assert!(empty_pcm.iter().all(|&s| s == 0));

    deck.loaded_track = Some(TrackMeta {
        title: "Paused".to_string(),
        artist: "Test".to_string(),
        bpm: 120.0,
        musical_key: "8A".to_string(),
        duration_seconds: 120.0,
    });
    deck.is_playing = false;
    let paused_pcm = AudioEngine::synthesize_deck_audio(&deck, 0.1, 44100);
    assert!(paused_pcm.iter().all(|&s| s == 0));
}

#[test]
fn test_audio_pcm_synthesizer_zero_volume_mutes_output() {
    let mut orch = DjOrchestrator::new();
    orch.deck_a.volume_fader = 0.0;
    let pcm = AudioEngine::synthesize_deck_audio(&orch.deck_a, 0.1, 44100);
    assert!(pcm.iter().all(|&s| s == 0));
}

#[test]
fn test_dispatch_play_pause_and_cue_return() {
    let mut orch = DjOrchestrator::new();
    assert!(orch.deck_a.is_playing);
    orch.deck_a.position_seconds = 42.5;

    let paused = orch
        .dispatch(Command::PlayPause {
            deck: DeckId::DeckA,
        })
        .expect("pause");
    assert!(paused.ok);
    assert!(!orch.deck_a.is_playing);

    let cued = orch
        .dispatch(Command::CueReturn {
            deck: DeckId::DeckA,
        })
        .expect("cue");
    assert!(cued.ok);
    assert_eq!(orch.deck_a.position_seconds, 0.0);
    assert!(orch.deck_a.is_cue_active);
}

#[test]
fn test_dispatch_stop_all_decks() {
    let mut orch = DjOrchestrator::new();
    orch.deck_a.is_playing = true;
    orch.deck_b.is_playing = true;
    orch.deck_c.is_playing = true;
    orch.deck_d.is_playing = true;

    let out = orch.dispatch(Command::StopAllDecks).expect("stop all");
    assert!(out.ok);
    assert!(!orch.deck_a.is_playing);
    assert!(!orch.deck_b.is_playing);
    assert!(!orch.deck_c.is_playing);
    assert!(!orch.deck_d.is_playing);
}

#[test]
fn test_dispatch_crossfader_and_pitch_bend_clamping() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::SetCrossfader { value: 5.0 })
        .expect("clamp high");
    assert_eq!(orch.crossfader, 1.0);
    orch.dispatch(Command::SetCrossfader { value: -5.0 })
        .expect("clamp low");
    assert_eq!(orch.crossfader, -1.0);

    orch.dispatch(Command::SetPitchBend {
        deck: DeckId::DeckA,
        pct: 99.0,
    })
    .expect("pitch clamp");
    assert_eq!(orch.deck_a.pitch_bend_pct, 16.0);
}

#[test]
fn test_dispatch_stem_gains_and_reset_all() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::SetStemGains {
        deck: DeckId::DeckA,
        vocals: 0.0,
        drums: 0.5,
        bass: 2.0,
        other: -1.0,
    })
    .expect("set stems");
    assert_eq!(orch.deck_a.stem_vocals_gain, 0.0);
    assert_eq!(orch.deck_a.stem_drums_gain, 0.5);
    assert_eq!(orch.deck_a.stem_bass_gain, 1.0);
    assert_eq!(orch.deck_a.stem_other_gain, 0.0);

    orch.dispatch(Command::ResetAllStems).expect("reset stems");
    assert_eq!(orch.deck_a.stem_vocals_gain, 1.0);
    assert_eq!(orch.deck_a.stem_other_gain, 1.0);
}

#[test]
fn test_dispatch_hot_cues_and_loop_bars() {
    let mut orch = DjOrchestrator::new();
    orch.deck_a.position_seconds = 16.25;
    orch.dispatch(Command::SetHotCue {
        deck: DeckId::DeckA,
        slot: 0,
    })
    .expect("hot cue 0");
    assert_eq!(orch.deck_a.hot_cue_seconds[0], Some(16.25));

    let bad_slot = orch.dispatch(Command::SetHotCue {
        deck: DeckId::DeckA,
        slot: 9,
    });
    assert!(bad_slot.is_err());

    orch.dispatch(Command::SetLoopBars {
        deck: DeckId::DeckA,
        bars: Some(8),
    })
    .expect("loop 8");
    assert_eq!(orch.deck_a.loop_bars, Some(8));
}

#[test]
fn test_dispatch_auto_sync_beatgrid() {
    let mut orch = DjOrchestrator::new();
    let out = orch.dispatch(Command::AutoSyncBeatgrid).expect("auto sync");
    assert!(out.ok);
    let plan = orch.plan_transition().expect("plan after sync");
    assert!(plan.bpm_delta.abs() < 0.01);
}

#[test]
fn test_dispatch_undo_and_redo_roundtrip() {
    let mut orch = DjOrchestrator::new();
    assert_eq!(orch.crossfader, 0.0);

    orch.dispatch(Command::SetCrossfader { value: 0.75 })
        .expect("set crossfader");
    assert_eq!(orch.crossfader, 0.75);
    assert_eq!(orch.undo_depth(), 1);

    let u = orch.dispatch(Command::Undo).expect("undo");
    assert!(u.ok);
    assert_eq!(orch.crossfader, 0.0);
    assert_eq!(orch.redo_depth(), 1);

    let r = orch.dispatch(Command::Redo).expect("redo");
    assert!(r.ok);
    assert_eq!(orch.crossfader, 0.75);
}

#[test]
fn test_dispatch_load_demo_crate_track_bounds() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::LoadDemoCrateTrack {
        deck: DeckId::DeckD,
        index: 3,
    })
    .expect("load crate track");
    assert_eq!(
        orch.deck_d.loaded_track.as_ref().map(|t| t.title.as_str()),
        Some("Velvet Acid Pulse")
    );

    let err = orch.dispatch(Command::LoadDemoCrateTrack {
        deck: DeckId::DeckD,
        index: 99,
    });
    assert!(err.is_err());
}

#[test]
fn test_dispatch_status_and_check_camelot() {
    let mut orch = DjOrchestrator::new();
    let status = orch.dispatch(Command::Status).expect("status");
    assert!(status.ok);
    assert!(status.data.get("deck_c").is_some());
    assert!(status.data.get("deck_d").is_some());

    let cam = orch
        .dispatch(Command::CheckCamelot {
            key_a: "8A".to_string(),
            key_b: "9A".to_string(),
        })
        .expect("camelot");
    assert_eq!(cam.data["compatible"], serde_json::json!(true));
}

#[test]
fn test_command_catalog_non_empty_and_unique_ids() {
    let catalog = Command::catalog();
    assert!(catalog.len() >= 15);
    let mut ids = std::collections::HashSet::new();
    for spec in &catalog {
        assert!(ids.insert(spec.id), "duplicate command id {}", spec.id);
    }
}

#[test]
fn test_parse_dettato_json_extracts_tuning_and_tracks() {
    let raw = r#"{
        "source": "lofi_cafe.mp3",
        "seconds": 30.0,
        "tuning_cents": 0.61,
        "drums": { "hits": 144, "model": "drum-kit" },
        "tracks": [
            { "name": "electric piano", "program": 4, "is_drum": false, "notes": 59 },
            { "name": "Drums", "program": null, "is_drum": true, "notes": 144 }
        ]
    }"#;
    let parsed = parse_dettato_json(
        raw,
        "fallback.mp3",
        Path::new("lofi_cafe.mid"),
        Some(Path::new("lofi_cafe.json")),
    )
    .expect("parse dettato json");
    assert_eq!(parsed.source, "lofi_cafe.mp3");
    assert_eq!(parsed.tuning_cents, Some(0.61));
    assert_eq!(parsed.drum_hits, 144);
    assert_eq!(parsed.drum_model.as_deref(), Some("drum-kit"));
    assert_eq!(parsed.tracks.len(), 2);
}

#[test]
fn test_dispatch_dettato_missing_audio_file_returns_error() {
    let mut orch = DjOrchestrator::new();
    let res = orch.dispatch(Command::AnalyzeTrackDettato {
        audio_path: "D:/nonexistent/missing_track_xyz.mp3".to_string(),
        output_dir: None,
        device: None,
    });
    assert!(res.is_err());
}

#[test]
fn test_new_session_resets_crossfader_and_preserves_undo() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::SetCrossfader { value: -0.8 })
        .expect("crossfader");
    orch.dispatch(Command::NewSession).expect("new session");
    assert_eq!(orch.crossfader, 0.0);
    assert!(orch.undo_depth() >= 2);
}
