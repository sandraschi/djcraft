use std::net::UdpSocket;
use std::path::Path;
use std::time::Duration;

use djcraft_audio::{AudioEngine, DeckId, TrackMeta};
use djcraft_audio::dsp::{detect_beatgrid, sample_varispeed, DecodedAudio, WsolaDeckState};
use djcraft_core::{
    encode_osc_message, parse_dettato_json, send_deck_video_controls, Command, DjOrchestrator,
    MixxxxxConfig, MixxxxxVideoControls, OscArg,
};

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

#[test]
fn test_osc_message_encoding_alignment_and_endianness() {
    let ping = encode_osc_message("/mixxxxx/ping", &[]);
    assert_eq!(ping.len() % 4, 0);
    assert!(ping.starts_with(b"/mixxxxx/ping\0"));
    assert!(ping.ends_with(b",\0\0\0"));

    let vid = encode_osc_message("/deck/1/video_enabled", &[OscArg::Int(1)]);
    assert_eq!(vid.len() % 4, 0);
    assert_eq!(&vid[vid.len() - 4..], &1i32.to_be_bytes());

    let bright = encode_osc_message("/deck/2/video_brightness", &[OscArg::Float(0.75)]);
    assert_eq!(bright.len() % 4, 0);
    assert_eq!(&bright[bright.len() - 4..], &0.75f32.to_be_bytes());
}

#[test]
fn test_mixxxxx_video_controls_udp_loopback_roundtrip() {
    let receiver = UdpSocket::bind("127.0.0.1:0").expect("bind loopback");
    receiver
        .set_read_timeout(Some(Duration::from_millis(500)))
        .expect("timeout");
    let port = receiver.local_addr().expect("addr").port();

    let cfg = MixxxxxConfig {
        host: "127.0.0.1".to_string(),
        in_port: port,
        out_port: 0,
    };
    let sent = send_deck_video_controls(
        &cfg,
        DeckId::DeckB,
        &MixxxxxVideoControls {
            enabled: Some(true),
            fullscreen: None,
            brightness: Some(0.8),
            contrast: None,
            saturation: None,
        },
    )
    .expect("send video controls");
    assert_eq!(
        sent,
        vec!["/deck/2/video_enabled", "/deck/2/video_brightness"]
    );

    let mut buf = [0u8; 128];
    let (n1, _) = receiver.recv_from(&mut buf).expect("recv pkt 1");
    assert!(String::from_utf8_lossy(&buf[..n1]).starts_with("/deck/2/video_enabled"));
    let (n2, _) = receiver.recv_from(&mut buf).expect("recv pkt 2");
    assert!(String::from_utf8_lossy(&buf[..n2]).starts_with("/deck/2/video_brightness"));
}

#[test]
fn test_mixxxxx_video_controls_rejects_empty_payload() {
    let cfg = MixxxxxConfig::default();
    let err = send_deck_video_controls(&cfg, DeckId::DeckA, &MixxxxxVideoControls::default());
    assert!(err.is_err());
}

#[test]
fn test_master_loudness_mute_and_deck_volume_dispatch() {
    let mut orch = DjOrchestrator::new();
    assert!((orch.master_volume - 0.75).abs() < 1e-6);
    assert!(!orch.master_muted);

    orch.dispatch(Command::SetMasterVolume { volume: 1.5 })
        .expect("clamp high");
    assert_eq!(orch.master_volume, 1.0);

    orch.dispatch(Command::ToggleMasterMute).expect("mute");
    assert!(orch.master_muted);

    orch.dispatch(Command::SetDeckVolume {
        deck: DeckId::DeckA,
        volume: 0.4,
    })
    .expect("deck vol");
    assert!((orch.deck_a.volume_fader - 0.4).abs() < 1e-6);
}

#[test]
fn test_mix_4deck_frame_respects_mute_loudness_and_crossfader() {
    let orch = DjOrchestrator::new();
    let decks = [
        orch.deck_a.clone(),
        orch.deck_b.clone(),
        orch.deck_c.clone(),
        orch.deck_d.clone(),
    ];
    let times = [0.05, 0.05, 0.0, 0.0];

    let muted = AudioEngine::mix_4deck_frame(&decks, 0.0, 0.8, true, &times, 100);
    assert_eq!(muted, (0.0, 0.0));

    let loud = AudioEngine::mix_4deck_frame(&decks, -1.0, 1.0, false, &times, 100);
    let quiet = AudioEngine::mix_4deck_frame(&decks, -1.0, 0.25, false, &times, 100);
    assert!(loud.0.abs() > quiet.0.abs());

    // Deck A is playing, Deck B is paused -> full right crossfader (+1.0) should silence Deck A
    let right_only = AudioEngine::mix_4deck_frame(&decks, 1.0, 1.0, false, &times, 100);
    assert!(right_only.0.abs() < 1e-5);
}

#[test]
fn test_detect_beatgrid_recovers_128_bpm_click_track() {
    let sr = 44100usize;
    let duration_sec = 10usize;
    let total = sr * duration_sec;
    let mut left = vec![0.0_f32; total];
    let beat_samples = (sr as f64 * 60.0 / 128.0).round() as usize;
    let offset = sr / 4; // 0.25s downbeat offset
    let mut pos = offset;
    while pos + 400 < total {
        for k in 0..400 {
            let env = 1.0 - (k as f32 / 400.0);
            left[pos + k] = (2.0 * std::f32::consts::PI * 90.0 * k as f32 / sr as f32).sin() * env;
        }
        pos += beat_samples;
    }
    let audio = DecodedAudio {
        sample_rate: sr as u32,
        right: left.clone(),
        left,
        duration_seconds: duration_sec as f64,
    };
    let grid = detect_beatgrid(&audio);
    assert!(
        (grid.bpm - 128.0).abs() <= 2.5,
        "Expected ~128 BPM, got {}",
        grid.bpm
    );
    assert!(grid.confidence > 0.1);
}

#[test]
fn test_varispeed_catmull_rom_interpolation_bounds_and_accuracy() {
    let audio = DecodedAudio {
        sample_rate: 44100,
        left: vec![0.0, 0.5, 1.0, 0.5, 0.0],
        right: vec![0.0, -0.5, -1.0, -0.5, 0.0],
        duration_seconds: 5.0 / 44100.0,
    };
    let (l_mid, r_mid) = sample_varispeed(&audio, 2.0);
    assert!((l_mid - 1.0).abs() < 1e-5);
    assert!((r_mid + 1.0).abs() < 1e-5);
    assert_eq!(sample_varispeed(&audio, -1.0), (0.0, 0.0));
    assert_eq!(sample_varispeed(&audio, 100.0), (0.0, 0.0));
}

#[test]
fn test_wsola_keylock_produces_finite_audio_across_tempos() {
    let sr = 44100u32;
    let n = sr as usize * 2;
    let sine: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / sr as f32).sin() * 0.6)
        .collect();
    let audio = DecodedAudio {
        sample_rate: sr,
        right: sine.clone(),
        left: sine,
        duration_seconds: 2.0,
    };
    for ratio in [0.85_f64, 1.15_f64] {
        let mut wsola = WsolaDeckState::new(sr);
        let mut max_amp = 0.0_f32;
        for _ in 0..8000 {
            let (l, r) = wsola.next_sample(&audio, ratio);
            assert!(l.is_finite() && r.is_finite());
            max_amp = max_amp.max(l.abs());
        }
        assert!(max_amp > 0.1, "WSOLA at ratio {ratio} must emit audio");
    }
}

#[test]
fn test_dispatch_keylock_nudge_scale_and_phase_sync() {
    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::ToggleKeylock {
        deck: DeckId::DeckA,
    })
    .expect("keylock");
    assert!(orch.deck_a.keylock);

    orch.dispatch(Command::NudgeBeatgrid {
        deck: DeckId::DeckA,
        delta_ms: 25.0,
    })
    .expect("nudge");
    assert!((orch.deck_a.beatgrid_offset_sec - 0.025).abs() < 1e-6);

    orch.dispatch(Command::ScaleBpm {
        deck: DeckId::DeckB,
        multiplier: 0.5,
    })
    .expect("half bpm");
    assert!((orch.deck_b.loaded_track.as_ref().expect("track").bpm - 64.0).abs() < 1e-6);

    orch.dispatch(Command::ScaleBpm {
        deck: DeckId::DeckB,
        multiplier: 2.0,
    })
    .expect("double bpm");
    orch.deck_a.position_seconds = 10.3;
    let synced = orch
        .dispatch(Command::PhaseSyncBeatgrid)
        .expect("phase sync");
    assert!(synced.ok);
    let plan = orch.plan_transition().expect("plan");
    assert!(plan.bpm_delta.abs() < 0.01);
}
