# Asset attribution

Every non-code asset in this repository is listed here with its author, source and licence. `cargo xtask assets` (part of `cargo xtask ci`) fails if an asset file is missing from this table.

**Policy (mandatory):** DJCraft contains no proprietary vendor iconography, images, templates, themes, sounds, fonts or other assets. Every asset is original work by DJCraft contributors or openly licensed material on the storytold/craft-contrib licence allowlist.

| Asset | Author | Source | Licence | Notes |
|---|---|---|---|---|
| `docs/brochure/djcraft-brochure.pdf` | DJCraft contributors | created for DJCraft | MIT OR Apache-2.0 | domain brochure |
| `docs/images/live-console-battle.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Battle Console dark theme screenshot |
| `docs/images/booth-crate-split.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Booth + Crate split workspace screenshot |
| `docs/images/live-console-shocking-pink-4deck.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | 4-Deck Shocking Pink theme screenshot |
| `docs/images/daylight-booth.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Daylight Booth light theme screenshot |
| `docs/images/crates-gig-prep.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | 2-pane Crates & Gig Prep screenshot |
| `docs/images/djcraft-battle.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Battle theme screenshot |
| `docs/images/djcraft-crates.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Crates view screenshot |
| `docs/images/djcraft-lcars.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | LCARS-style skin screenshot |
| `docs/images/djcraft-settings.png` | DJCraft contributors | captured from DJCraft | MIT OR Apache-2.0 | Settings window screenshot |

## Third-party code (ported)

| Code | Where in DJCraft | Upstream | Authors | Licence |
|---|---|---|---|---|
| ClubCraft stem separation (`crates/stems`: STFT/iSTFT, Demucs `apply_model` pipeline, Mel-Band RoFormer wrapper, ONNX Runtime backend, FLAC stem cache, examples, progressive test) | `crates/djcraft-stems` | github.com/beneaze/ClubCraft @ `9afec47` | The ClubCraft contributors | MIT OR Apache-2.0 |
| ClubCraft offline resampler (`crates/audio/src/resample.rs`) | `crates/djcraft-stems/src/resample.rs` | same | The ClubCraft contributors | MIT OR Apache-2.0 |
| ClubCraft progressive stem buffer (`crates/core/src/stems.rs`) and `StemKind` | `crates/djcraft-audio/src/stem_track.rs` | same | The ClubCraft contributors | MIT OR Apache-2.0 |
| Residual stem mix formula and stem fade constants (`crates/engine/src/deck.rs`) | `crates/djcraft-audio/src/output.rs` (`residual_mix`) | same | The ClubCraft contributors | MIT OR Apache-2.0 |

Every ported file starts with a provenance header; the file-by-file changes are in `docs/CLUBCRAFT_PORT_LOG.md`.

## Models (not in git; downloaded separately)

Release `models-v1` of this repository mirrors ClubCraft's `models-v1` ONNX pack without COCOLA.

| Model | Used for | Authors | Licence |
|---|---|---|---|
| Demucs v4 `htdemucs`, `htdemucs_ft` | Stem separation (Fast; High, Ultra, drums/bass in Best) | Meta Platforms (Rouard, Massa, Defossez) | MIT |
| Mel-Band RoFormer vocal model | Vocals in Best quality | Kimberley Jensen (architecture: Wang, Lu, Won) | MIT |
| All-In-One structure model | Section detection (planned, Phase 2) | Taejun Kim, Juhan Nam | MIT |
| Model JSON manifests | Model metadata | The ClubCraft contributors | MIT OR Apache-2.0 |

COCOLA (ClubCraft's mashup re-ranker) is deliberately not included: its training data contains MoisesDB, which is licensed for non-commercial research only.

## Runtime libraries

| Library | Platform | Licence |
|---|---|---|
| ONNX Runtime (via the `ort` crate, prebuilt binaries) | Windows, Linux, macOS | MIT (Microsoft) |
