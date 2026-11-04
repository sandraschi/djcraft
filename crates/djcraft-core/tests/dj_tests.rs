use djcraft_core::DjOrchestrator;
use djcraft_audio::AudioEngine;

#[test]
fn test_djcraft_camelot_wheel() {
    assert!(AudioEngine::is_camelot_compatible("8A", "8A")); // Same
    assert!(AudioEngine::is_camelot_compatible("8A", "8B")); // Relative major
    assert!(AudioEngine::is_camelot_compatible("8A", "9A")); // +1 adjacent
    assert!(AudioEngine::is_camelot_compatible("8A", "7A")); // -1 adjacent
    assert!(AudioEngine::is_camelot_compatible("12A", "1A")); // Wrap-around
    assert!(!AudioEngine::is_camelot_compatible("8A", "10A")); // Distance 2
}

#[test]
fn test_djcraft_transition_planner() {
    let orch = DjOrchestrator::new();
    let plan = orch.plan_transition().expect("plan transition");
    assert!(plan.harmonic_compatible);
    assert_eq!(plan.bpm_delta, 2.0);
    assert_eq!(plan.recommended_transition_bars, 32);
}
