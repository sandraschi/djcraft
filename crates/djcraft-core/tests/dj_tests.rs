use std::net::UdpSocket;
use std::path::Path;
use std::time::Duration;

use djcraft_audio::dsp::{
    detect_beatgrid, has_sidecar_stems, sample_varispeed, stem_dir_for_audio, DecodedAudio,
    ThreeBandEq, WaveformSummary, WsolaDeckState,
};
use djcraft_audio::{AudioEngine, DeckId, TrackMeta};
use djcraft_core::{
    action_to_command, builtin_profile_rules, encode_m3u, encode_osc_message,
    encode_serato_crate_bytes, encode_vdj_folder_xml, parse_dettato_json, parse_m3u,
    parse_natural_language_command, parse_raw_midi, parse_serato_crate_bytes, parse_vdj_folder_xml,
    plan_harmonic_setlist, send_deck_video_controls, Command, ControllerProfileId, CrateQuery,
    CrateSortField, CrateStore, CrateTrack, DjOrchestrator, MidiTargetAction, MixxxxxConfig,
    MixxxxxVideoControls, OscArg,
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

#[test]
fn test_three_band_kill_eq_attenuates_bass_and_full_kill() {
    let sr = 44100u32;
    let mut eq_flat = ThreeBandEq::new(sr);
    let mut eq_kill_low = ThreeBandEq::new(sr);
    let mut eq_kill_all = ThreeBandEq::new(sr);

    let mut energy_flat = 0.0_f32;
    let mut energy_kill_low = 0.0_f32;
    let mut energy_kill_all = 0.0_f32;
    for i in 0..2000 {
        let s = (2.0 * std::f32::consts::PI * 60.0 * i as f32 / sr as f32).sin();
        let (fl, _) = eq_flat.process_stereo(s, s, 1.0, 1.0, 1.0);
        let (kl, _) = eq_kill_low.process_stereo(s, s, 0.0, 1.0, 1.0);
        let (ka, _) = eq_kill_all.process_stereo(s, s, 0.0, 0.0, 0.0);
        if i > 500 {
            energy_flat += fl * fl;
            energy_kill_low += kl * kl;
            energy_kill_all += ka * ka;
        }
    }
    assert!(energy_flat > 100.0);
    assert!(
        energy_kill_low < energy_flat * 0.25,
        "Killing Low band must strongly attenuate 60 Hz"
    );
    assert!(energy_kill_all < 1e-6, "Killing all 3 bands must be silent");
}

#[test]
fn test_waveform_summary_from_decoded_and_eq_dispatch() {
    let sr = 44100u32;
    let sine: Vec<f32> = (0..sr as usize)
        .map(|i| (2.0 * std::f32::consts::PI * 100.0 * i as f32 / sr as f32).sin() * 0.8)
        .collect();
    let audio = DecodedAudio {
        sample_rate: sr,
        right: sine.clone(),
        left: sine,
        duration_seconds: 1.0,
    };
    let summary = WaveformSummary::from_decoded(&audio);
    assert!(!summary.low_peaks.is_empty());
    let (lo, _hi) = summary.sample_at(0.5);
    assert!(lo > 0.3);

    let mut orch = DjOrchestrator::new();
    orch.dispatch(Command::SetDeckEq {
        deck: DeckId::DeckA,
        low: 0.0,
        mid: 1.2,
        high: 3.0,
    })
    .expect("set eq");
    assert_eq!(orch.deck_a.eq_low, 0.0);
    assert_eq!(orch.deck_a.eq_high, 2.0);
    orch.dispatch(Command::ResetDeckEq {
        deck: DeckId::DeckA,
    })
    .expect("reset eq");
    assert_eq!(orch.deck_a.eq_low, 1.0);
}

#[test]
fn test_serato_crate_binary_roundtrip_utf16be() {
    let inputs = vec![
        r"D:\Music\House\Artist A - Track 1.mp3".to_string(),
        r"D:\Music\Techno\Café del Mar.flac".to_string(),
    ];
    let bytes = encode_serato_crate_bytes(&inputs);
    assert!(bytes.starts_with(b"vrsn"));
    let parsed = parse_serato_crate_bytes(&bytes, Some(r"D:\")).expect("parse serato");
    assert_eq!(parsed, inputs);
}

#[test]
fn test_vdj_folder_xml_and_m3u_roundtrip() {
    let tracks = vec![
        CrateTrack {
            id: 1,
            path: r#"D:\Music\Rock & Roll\Track "One".mp3"#.to_string(),
            title: "Track One".to_string(),
            artist: "Rock & Roll".to_string(),
            genre: "House".to_string(),
            bpm: 126.0,
            musical_key: "8A".to_string(),
            duration_seconds: 310.0,
            beatgrid_offset_sec: 0.0,
            tuning_cents: None,
            drum_hits: None,
            rating: 5,
            analyzed_at: None,
            crate_position: 0,
        },
        CrateTrack {
            id: 2,
            path: r"D:\Music\Deep\Track Two.wav".to_string(),
            title: "Track Two".to_string(),
            artist: "Deep".to_string(),
            genre: "Deep".to_string(),
            bpm: 122.0,
            musical_key: "7A".to_string(),
            duration_seconds: 280.0,
            beatgrid_offset_sec: 0.0,
            tuning_cents: None,
            drum_hits: None,
            rating: 4,
            analyzed_at: None,
            crate_position: 1,
        },
    ];
    let xml = encode_vdj_folder_xml(&tracks);
    let parsed_xml = parse_vdj_folder_xml(&xml);
    assert_eq!(parsed_xml.len(), 2);
    assert_eq!(parsed_xml[0], tracks[0].path);

    let m3u = encode_m3u(&tracks);
    let parsed_m3u = parse_m3u(&m3u);
    assert_eq!(
        parsed_m3u,
        vec![tracks[0].path.clone(), tracks[1].path.clone()]
    );
}

#[test]
fn test_crate_store_subcrates_filtering_and_sorting() {
    let store = CrateStore::open_in_memory().expect("in memory db");
    let house_id = store.ensure_crate_path("House").expect("house");
    let deep_id = store
        .create_subcrate(Some(house_id), "Deep House")
        .expect("deep");
    let tech_id = store.ensure_crate_path("Techno").expect("tech");

    let t1 = store
        .upsert_track("D:/a.mp3", "Alpha", "Artist A", "House", 124.0, "8A", 300.0)
        .expect("t1");
    let t2 = store
        .upsert_track(
            "D:/b.mp3",
            "Beta",
            "Artist B",
            "Deep House",
            128.0,
            "9A",
            360.0,
        )
        .expect("t2");
    let t3 = store
        .upsert_track(
            "D:/c.mp3", "Gamma", "Artist C", "Techno", 138.0, "3B", 400.0,
        )
        .expect("t3");

    store.add_track_to_crate(house_id, t1).expect("add t1");
    store.add_track_to_crate(deep_id, t2).expect("add t2");
    store.add_track_to_crate(tech_id, t3).expect("add t3");
    store.set_track_rating(t2, 5).expect("rate");

    // Recursive subcrate roll-up: House includes Deep House
    let house_all = store
        .query_tracks(&CrateQuery {
            crate_id: Some(house_id),
            include_subcrates: true,
            sort_by: CrateSortField::Bpm,
            sort_asc: false,
            ..Default::default()
        })
        .expect("query house");
    assert_eq!(house_all.len(), 2);
    assert_eq!(house_all[0].title, "Beta"); // 128 BPM first (desc)

    // Non-recursive: only direct tracks in House
    let house_direct = store
        .query_tracks(&CrateQuery {
            crate_id: Some(house_id),
            include_subcrates: false,
            ..Default::default()
        })
        .expect("query direct");
    assert_eq!(house_direct.len(), 1);
    assert_eq!(house_direct[0].title, "Alpha");

    // Camelot harmonic filter compatible with 8A -> Alpha (8A) + Beta (9A), excludes Gamma (3B)
    let harmonic = store
        .query_tracks(&CrateQuery {
            compatible_with_key: Some("8A".to_string()),
            ..Default::default()
        })
        .expect("harmonic");
    assert_eq!(harmonic.len(), 2);

    // BPM range filter 130..145 -> only Gamma
    let fast = store
        .query_tracks(&CrateQuery {
            bpm_min: Some(130.0),
            bpm_max: Some(145.0),
            ..Default::default()
        })
        .expect("fast");
    assert_eq!(fast.len(), 1);
    assert_eq!(fast[0].title, "Gamma");
}

#[test]
fn test_crate_store_serato_and_vdj_file_import_export() {
    let store = CrateStore::open_in_memory().expect("db");
    let tmp = std::env::temp_dir().join("djcraft_crate_test");
    let _ = std::fs::create_dir_all(&tmp);

    // Create a Serato subcrate file named "Club%%Peak Hour.crate"
    let serato_in = tmp.join("Club%%Peak Hour.crate");
    let raw = encode_serato_crate_bytes(&[r"D:\Sets\DJ X - Peak Anthem.mp3".to_string()]);
    std::fs::write(&serato_in, raw).expect("write serato");

    let summary = store
        .import_serato_crate(&serato_in)
        .expect("import serato");
    assert_eq!(summary.crate_path, "Club/Peak Hour");
    assert_eq!(summary.imported_tracks, 1);

    // Export that crate to VirtualDJ .vdjfolder and re-import
    let vdj_out = tmp.join("Peak Hour.vdjfolder");
    let exported = store
        .export_vdj_file(summary.crate_id, &vdj_out)
        .expect("export vdj");
    assert_eq!(exported, 1);

    let vdj_summary = store.import_vdj_file(&vdj_out).expect("import vdj");
    assert_eq!(vdj_summary.crate_path, "VirtualDJ/Peak Hour");
    assert_eq!(vdj_summary.imported_tracks, 1);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_midi_rane_twelve_and_seventy_two_profiles_and_autodetect() {
    assert_eq!(
        ControllerProfileId::auto_detect_from_port_name("RANE TWELVE MKII USB MIDI"),
        ControllerProfileId::RaneTwelveMk2
    );
    assert_eq!(
        ControllerProfileId::auto_detect_from_port_name("RANE SEVENTY-TWO MKII"),
        ControllerProfileId::RaneSeventyTwoMk2
    );
    assert_eq!(
        ControllerProfileId::auto_detect_from_port_name("Pioneer DDJ-FLX4"),
        ControllerProfileId::PioneerDdjFlx4
    );

    let r12 = builtin_profile_rules(ControllerProfileId::RaneTwelveMk2);
    assert!(r12.len() >= 20, "Rane Twelve MKII must map Decks 1-4");

    let r72 = builtin_profile_rules(ControllerProfileId::RaneSeventyTwoMk2);
    assert!(r72.iter().any(|r| r.action == MidiTargetAction::Crossfader));
    assert!(r72
        .iter()
        .any(|r| r.action == MidiTargetAction::MasterVolume));

    // Parse MAG FOUR crossfader CC 0x0B on channel 0 at value 127 (+1.0 full Deck B)
    let ev = parse_raw_midi(&[0xB0, 0x0B, 127]).expect("parse CC");
    let cmd = action_to_command(MidiTargetAction::Crossfader, ev.value, (1.0, 1.0, 1.0));
    let mut orch = DjOrchestrator::new();
    orch.dispatch(cmd).expect("dispatch midi crossfader");
    assert!((orch.crossfader - 1.0).abs() < 1e-6);
}

#[test]
fn test_transparent_sidecar_stems_layout_and_hidden_scan() {
    let tmp = std::env::temp_dir().join("djcraft_stems_test");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).expect("mkdir");

    let track = tmp.join("Artist - Track.mp3");
    std::fs::write(&track, b"fake").expect("write mp3");
    assert!(!has_sidecar_stems(&track));

    let sdir = stem_dir_for_audio(&track);
    assert!(sdir.to_string_lossy().contains(".djcraft-stems"));
    std::fs::create_dir_all(&sdir).expect("mkdir stems");
    for wav in ["vocals.wav", "drums.wav", "bass.wav", "other.wav"] {
        std::fs::write(sdir.join(wav), b"RIFF").expect("write stem wav");
    }
    assert!(has_sidecar_stems(&track));

    // Folder scan MUST ignore .djcraft-stems/ so stem WAVs never pollute crates!
    let store = CrateStore::open_in_memory().expect("db");
    let cid = store.ensure_crate_path("Scanned").expect("crate");
    let count = store.scan_audio_folder(cid, &tmp).expect("scan");
    assert_eq!(count, 1);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_ai_natural_language_commands_and_harmonic_setlist() {
    assert!(matches!(
        parse_natural_language_command("phase sync"),
        Some(Command::PhaseSyncBeatgrid)
    ));
    assert!(matches!(
        parse_natural_language_command("kill bass a"),
        Some(Command::SetDeckEq {
            deck: DeckId::DeckA,
            ..
        })
    ));
    assert!(matches!(
        parse_natural_language_command("stop all"),
        Some(Command::StopAllDecks)
    ));

    let store = CrateStore::open_in_memory().expect("db");
    store.seed_demo_if_empty().expect("seed");
    let setlist = plan_harmonic_setlist(&store, "8A", 4).expect("setlist");
    assert!(setlist.len() >= 3);
}

#[test]
fn test_crate_full_crud_and_manual_gig_reordering() {
    let store = CrateStore::open_in_memory().expect("db");
    let root = store.ensure_crate_path("Friday Gig").expect("root");
    let sub = store.create_subcrate(Some(root), "Warmup").expect("sub");

    let t1 = store
        .upsert_track("D:/1.mp3", "Track 1", "A", "", 120.0, "8A", 300.0)
        .expect("t1");
    let t2 = store
        .upsert_track("D:/2.mp3", "Track 2", "B", "", 124.0, "8A", 300.0)
        .expect("t2");
    let t3 = store
        .upsert_track("D:/3.mp3", "Track 3", "C", "", 128.0, "9A", 300.0)
        .expect("t3");
    store.add_track_to_crate(sub, t1).expect("add 1");
    store.add_track_to_crate(sub, t2).expect("add 2");
    store.add_track_to_crate(sub, t3).expect("add 3");

    // Reorder: move Track 3 to slot 0 (opener!)
    store.reorder_crate_track(sub, t3, 0).expect("reorder");
    let ordered = store
        .query_tracks(&CrateQuery {
            crate_id: Some(sub),
            include_subcrates: false,
            sort_by: CrateSortField::ManualOrder,
            sort_asc: true,
            ..Default::default()
        })
        .expect("query ordered");
    assert_eq!(
        ordered.iter().map(|t| t.title.as_str()).collect::<Vec<_>>(),
        vec!["Track 3", "Track 1", "Track 2"]
    );

    // Rename root crate -> descendant subcrate path updates automatically
    let renamed = store.rename_crate(root, "Saturday Club").expect("rename");
    assert_eq!(renamed, "Saturday Club");
    let nodes = store.list_crates().expect("list");
    assert!(nodes.iter().any(|n| n.full_path == "Saturday Club/Warmup"));

    // Duplicate subcrate preserves order
    let dup_id = store.duplicate_crate(sub, "Warmup Backup").expect("dup");
    let dup_tracks = store
        .query_tracks(&CrateQuery {
            crate_id: Some(dup_id),
            include_subcrates: false,
            sort_by: CrateSortField::ManualOrder,
            ..Default::default()
        })
        .expect("dup tracks");
    assert_eq!(dup_tracks[0].title, "Track 3");

    // Remove track & delete crate
    store.remove_track_from_crate(dup_id, t3).expect("remove");
    store.delete_crate(dup_id).expect("delete");
    let after = store.list_crates().expect("list after");
    assert!(!after.iter().any(|n| n.id == dup_id));
}

#[test]
fn test_dual_bus_pfl_pre_cue_and_split_mono_routing() {
    let mut orch = DjOrchestrator::new();
    // Mute Deck A channel fader so Master is silent, then enable PFL pre-cue on Deck A
    orch.dispatch(Command::SetDeckVolume {
        deck: DeckId::DeckA,
        volume: 0.0,
    })
    .expect("vol 0");
    let pfl_res = orch
        .dispatch(Command::TogglePflCue {
            deck: DeckId::DeckA,
        })
        .expect("toggle pfl");
    assert!(pfl_res.ok);
    assert!(orch.deck_a.pfl_cue);

    orch.dispatch(Command::SetHeadphoneMix {
        cue_volume: 1.0,
        cue_mix: 0.0, // 100% PFL
        split_cue: false,
    })
    .expect("set hp mix");

    let decks = [
        orch.deck_a.clone(),
        orch.deck_b.clone(),
        orch.deck_c.clone(),
        orch.deck_d.clone(),
    ];
    let mut master_energy = 0.0_f32;
    let mut cue_energy = 0.0_f32;
    for i in 0..400_u64 {
        let t = i as f64 / 44100.0;
        let ((ml, mr), (cl, cr)) = AudioEngine::mix_dual_bus_frame(
            &decks,
            orch.crossfader,
            orch.master_volume,
            orch.master_muted,
            orch.headphone_volume,
            orch.headphone_cue_mix,
            false,
            &[t, 0.0, 0.0, 0.0],
            i,
        );
        master_energy += ml.abs() + mr.abs();
        cue_energy += cl.abs() + cr.abs();
    }
    assert!(master_energy < 1e-6, "Master must be silent when fader=0");
    assert!(cue_energy > 1.0, "PFL Cue bus must hear Deck A pre-fader");

    // Split Mono Cue: Left ear = Cue Mono (non-zero), Right ear = Master Mono (silent)
    let mut split_l = 0.0_f32;
    let mut split_r = 0.0_f32;
    for i in 0..400_u64 {
        let t = i as f64 / 44100.0;
        let ((sl, sr), _) = AudioEngine::mix_dual_bus_frame(
            &decks,
            orch.crossfader,
            orch.master_volume,
            orch.master_muted,
            1.0,
            0.0,
            true,
            &[t, 0.0, 0.0, 0.0],
            i,
        );
        split_l += sl.abs();
        split_r += sr.abs();
    }
    assert!(split_l > 1.0, "Split Cue Left must carry PFL Cue");
    assert!(split_r < 1e-6, "Split Cue Right must carry silent Master");
}

#[test]
fn test_lossless_wav_mix_recorder_streaming_and_header_integrity() {
    use djcraft_audio::recorder::{ActiveRecording, RecordingFormat};

    let tmp_dir = std::env::temp_dir().join("djcraft_rec_test");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let wav_path = tmp_dir.join("live_mix_test.wav");

    let (rec, mut tap) =
        ActiveRecording::start(wav_path.clone(), 44100, RecordingFormat::Wav).expect("start rec");
    for i in 0..2048_usize {
        let s = ((i as f32 * 0.05).sin()) * 0.4;
        tap.push_frame(s, -s);
    }
    tap.flush();
    drop(tap);

    let report = rec.finish().expect("finish rec");
    assert_eq!(report.frames_written, 2048);
    assert_eq!(report.format, RecordingFormat::Wav);
    assert!(wav_path.is_file());

    let bytes = std::fs::read(&wav_path).expect("read wav");
    assert_eq!(bytes.len(), 44 + 2048 * 4);
    assert_eq!(&bytes[0..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WAVE");
    assert_eq!(&bytes[36..40], b"data");
    let data_len = u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]);
    assert_eq!(data_len, 2048 * 4);

    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_deck_b_start_seek_gen_vinyl_scratch_and_i18n() {
    use djcraft_core::i18n::{t, AppLocale, I18nKey};

    let mut orch = DjOrchestrator::new();
    let gen0 = orch.deck_b.seek_gen;
    orch.dispatch(Command::LoadDemoCrateTrack {
        deck: DeckId::DeckB,
        index: 1,
    })
    .expect("load b");
    assert!(orch.deck_b.seek_gen > gen0);
    assert!(!orch.deck_b.is_playing);

    // Clicking PlayPause on Deck B starts it cleanly
    let res = orch
        .dispatch(Command::PlayPause {
            deck: DeckId::DeckB,
        })
        .expect("play b");
    assert!(res.ok);
    assert!(orch.deck_b.is_playing);

    // Paused deck with vinyl scratch_velocity still synthesizes audio (baby scratch)
    orch.deck_b.is_playing = false;
    orch.deck_b.scratch_velocity = Some(-1.5);
    let mut energy = 0.0_f32;
    for i in 0..200_u64 {
        energy += AudioEngine::sample_deck_frame(&orch.deck_b, 1.0 + i as f64 / 44100.0, i).abs();
    }
    assert!(
        energy > 1.0,
        "Vinyl scratching must produce audio even when paused"
    );

    // i18n checks
    assert_eq!(t(I18nKey::LiveConsole, AppLocale::De), "LIVE-KONSOLE");
    assert_eq!(t(I18nKey::Play, AppLocale::De), "START");
    assert_eq!(AppLocale::De.short_label(), "DE");
}

#[test]
fn test_bipolar_color_filter_lpf_hpf_and_bypass() {
    use djcraft_audio::BipolarFilter;

    // 0.0 = transparent bypass
    let mut flt_flat = BipolarFilter::new(44100);
    let (l, r) = flt_flat.process_stereo(0.42, -0.42, 0.0);
    assert!((l - 0.42).abs() < 1e-6 && (r + 0.42).abs() < 1e-6);

    // High-frequency 6 kHz sine should be strongly attenuated by deep LPF (-0.9)
    let mut flt_lpf = BipolarFilter::new(44100);
    let mut lpf_energy = 0.0_f32;
    let mut dry_energy = 0.0_f32;
    for i in 0..800 {
        let s = (2.0 * std::f32::consts::PI * 6000.0 * i as f32 / 44100.0).sin();
        let (ol, _) = flt_lpf.process_stereo(s, s, -0.9);
        if i >= 200 {
            dry_energy += s.abs();
            lpf_energy += ol.abs();
        }
    }
    assert!(
        lpf_energy < dry_energy * 0.25,
        "Deep LPF must attenuate 6 kHz (lpf={lpf_energy}, dry={dry_energy})"
    );

    // Low-frequency 80 Hz sine should be strongly attenuated by sharp HPF (+0.9)
    let mut flt_hpf = BipolarFilter::new(44100);
    let mut hpf_energy = 0.0_f32;
    let mut low_dry = 0.0_f32;
    for i in 0..1200 {
        let s = (2.0 * std::f32::consts::PI * 80.0 * i as f32 / 44100.0).sin();
        let (ol, _) = flt_hpf.process_stereo(s, s, 0.9);
        if i >= 400 {
            low_dry += s.abs();
            hpf_energy += ol.abs();
        }
    }
    assert!(
        hpf_energy < low_dry * 0.25,
        "Sharp HPF must attenuate 80 Hz (hpf={hpf_energy}, dry={low_dry})"
    );
}

#[test]
fn test_all_12_fx_algorithms_and_3_packs_produce_finite_stereo() {
    use djcraft_audio::{FxKind, FxSlotParams, FxStemScope, FxTarget, FxUnitDsp, FxUnitParams};

    assert_eq!(FxKind::ALL.len(), 12);
    for kind in FxKind::ALL {
        let mut dsp = FxUnitDsp::new(44100);
        let params = FxUnitParams {
            enabled: true,
            target: FxTarget::DeckA,
            stem_scope: FxStemScope::FullDeck,
            beats: 0.5,
            wet_dry: 0.8,
            slots: [
                FxSlotParams {
                    kind,
                    enabled: true,
                    depth: 0.7,
                },
                FxSlotParams::default(),
                FxSlotParams::default(),
            ],
        };
        let mut energy = 0.0_f32;
        for i in 0..1024 {
            let s = (2.0 * std::f32::consts::PI * 440.0 * i as f32 / 44100.0).sin() * 0.5;
            let (ol, or) = dsp.process_stereo(s, s, &params, 128.0);
            assert!(
                ol.is_finite() && or.is_finite(),
                "Non-finite output for {kind:?}"
            );
            energy += ol.abs() + or.abs();
        }
        assert!(energy > 1.0, "FX {kind:?} produced silent output");
    }
}

#[test]
fn test_fx_commands_preset_load_and_json_roundtrip() {
    use djcraft_audio::{
        builtin_fx_presets, load_user_fx_presets_from_dir, save_fx_preset_to_dir, FxPackId,
        FxPackPreset, FxStemScope,
    };

    let mut orch = DjOrchestrator::new();
    let flt = orch
        .dispatch(Command::SetDeckFilter {
            deck: DeckId::DeckA,
            value: -0.65,
        })
        .expect("set filter");
    assert!(flt.ok);
    assert!((orch.deck_a.color_filter + 0.65).abs() < 1e-6);

    let tog = orch
        .dispatch(Command::ToggleFxUnit { unit: 0 })
        .expect("toggle fx1");
    assert!(tog.ok && orch.fx_units[0].enabled);

    let presets = builtin_fx_presets();
    assert!(presets.len() >= 6);
    let load = orch
        .dispatch(Command::LoadFxPreset {
            unit: 1,
            preset_name: "Vocal Dub Wash".to_string(),
        })
        .expect("load preset");
    assert!(load.ok);
    assert!(orch.fx_units[1].enabled);
    assert_eq!(orch.fx_units[1].stem_scope, FxStemScope::VocalsOnly);

    // JSON custom FX pack persistence roundtrip
    let tmp_dir = std::env::temp_dir().join("djcraft_fx_pack_test");
    let _ = std::fs::remove_dir_all(&tmp_dir);
    let custom = FxPackPreset {
        name: "Vienna Club Dub".to_string(),
        pack: FxPackId::ClubCore,
        beats: 0.75,
        wet_dry: 0.8,
        stem_scope: FxStemScope::VocalsOnly,
        slots: orch.fx_units[1].slots,
    };
    let saved_path = save_fx_preset_to_dir(&tmp_dir, &custom).expect("save fx preset");
    assert!(saved_path.is_file());
    let loaded = load_user_fx_presets_from_dir(&tmp_dir);
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].name, "Vienna Club Dub");
    let _ = std::fs::remove_dir_all(&tmp_dir);
}

#[test]
fn test_slip_mode_and_beatjump_dispatch() {
    let mut orch = DjOrchestrator::new();
    let slip_on = orch
        .dispatch(Command::ToggleSlipMode {
            deck: DeckId::DeckA,
        })
        .expect("slip on");
    assert!(slip_on.ok && orch.deck_a.slip_mode);

    let pos0 = orch.deck_a.position_seconds;
    let gen0 = orch.deck_a.seek_gen;
    let jump_fwd = orch
        .dispatch(Command::BeatJump {
            deck: DeckId::DeckA,
            beats: 4.0,
        })
        .expect("jump +4");
    assert!(jump_fwd.ok);
    assert!(orch.deck_a.position_seconds > pos0 + 1.5);
    assert!(orch.deck_a.seek_gen > gen0);

    let jump_back = orch
        .dispatch(Command::BeatJump {
            deck: DeckId::DeckA,
            beats: -4.0,
        })
        .expect("jump -4");
    assert!(jump_back.ok);
    assert!((orch.deck_a.position_seconds - pos0).abs() < 0.05);
}

#[test]
fn test_track_hot_cues_sqlite_persistence() {
    let store = CrateStore::open_in_memory().expect("db");
    let path = "demo://subterranean_dub";

    assert_eq!(store.get_track_hot_cues(path).expect("empty"), [None; 4]);

    store.set_track_hot_cue(path, 0, Some(12.5)).expect("set 0");
    store
        .set_track_hot_cue(path, 2, Some(64.25))
        .expect("set 2");
    assert_eq!(
        store.get_track_hot_cues(path).expect("loaded"),
        [Some(12.5), None, Some(64.25), None]
    );

    // Overwrite and clear
    store
        .set_track_hot_cue(path, 0, Some(15.0))
        .expect("overwrite 0");
    store.set_track_hot_cue(path, 2, None).expect("clear 2");
    assert_eq!(
        store.get_track_hot_cues(path).expect("updated"),
        [Some(15.0), None, None, None]
    );

    assert!(store.set_track_hot_cue(path, 4, Some(1.0)).is_err());
}
