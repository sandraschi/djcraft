# ClubCraft port log

Upstream: github.com/beneaze/ClubCraft (MIT OR Apache-2.0), read-only mirror `D:\Dev\repos\_upstream\ClubCraft`.
Plan: `docs/CLUBCRAFT_PARITY_PLAN.md`. Every ported file starts with a provenance header naming the upstream path and commit.

## 2026-11-28 - Phase 0 + 1 (live stemming), upstream `9afec47` (v0.3.5)

### Ported files

| DJCraft file | Upstream file | Changes |
|---|---|---|
| `crates/djcraft-stems/src/lib.rs` | `crates/stems/src/lib.rs` | paths to `crate::shared` / `crate::audio`; `forbid(unsafe_code)`; crate-level allow of `needless_range_loop`, `type_complexity` (style only, keeps upstream diff-able); new modules `audio`, `shared`, `resample`, `live`, `ops` |
| `crates/djcraft-stems/src/stft.rs` | `crates/stems/src/stft.rs` | none |
| `crates/djcraft-stems/src/pipeline.rs` | `crates/stems/src/pipeline.rs` | paths |
| `crates/djcraft-stems/src/model.rs` | `crates/stems/src/model.rs` | none |
| `crates/djcraft-stems/src/npy.rs` | `crates/stems/src/npy.rs` | none |
| `crates/djcraft-stems/src/roformer.rs` | `crates/stems/src/roformer.rs` | paths; env `CLUBCRAFT_ROFORMER_FP32` -> `DJCRAFT_ROFORMER_FP32` |
| `crates/djcraft-stems/src/cache.rs` | `crates/stems/src/cache.rs` | paths only. The FLAC Vorbis tags stay `CLUBCRAFT_STEMS/MODEL/SCALE/CHUNKS` on purpose, so stem caches are interchangeable with ClubCraft |
| `crates/djcraft-stems/src/backend/mod.rs` | `crates/stems/src/backend/mod.rs` | CoreML backend removed (macOS objc2 FFI needs `unsafe`; plan Phase 7); `BackendKind::CoreMl` returns an error; DirectML verdict file in the DJCraft data folder |
| `crates/djcraft-stems/src/backend/onnx.rs` | `crates/stems/src/backend/onnx.rs` | shared ORT CPU arena removed (raw ORT C API, `unsafe`): CPU sessions use per-session arenas, more peak RAM for `htdemucs_ft` on the CPU provider; env `CLUBCRAFT_ORT_*` -> `DJCRAFT_ORT_*` |
| `crates/djcraft-stems/src/resample.rs` | `crates/audio/src/resample.rs` | import path |
| `crates/djcraft-stems/tests/progressive.rs` | `crates/stems/tests/progressive.rs` | opt-in via `DJCRAFT_STEM_TESTS=1`, models from the installed pack |
| `crates/djcraft-stems/examples/separate.rs` | `crates/stems/examples/separate.rs` | paths; log filter `djcraft` |
| `crates/djcraft-stems/examples/backend_check.rs` | `crates/stems/examples/backend_check.rs` | paths |
| `crates/djcraft-audio/src/stem_track.rs` | `crates/core/src/stems.rs` (+ `StemKind` from `crates/core/src/analysis.rs`) | `STEM_SAMPLE_RATE`; `StemTrack::from_decoded_stems` for legacy sidecar WAVs |
| `crates/djcraft-audio/src/output.rs` (`residual_mix`, stem fade) | `crates/engine/src/deck.rs` (`Source::fill`) | formula and constants re-implemented inside DJCraft's existing deck loop (DJCraft engine kept, not replaced) |

Not ported: `backend/coreml.rs`, `build.rs` (macOS static ORT symbol), `tests/reference.rs` and `tests/roformer.rs` (need PyTorch reference tensors that are not in git).

### New DJCraft code

- `crates/djcraft-stems/src/shared.rs`, `audio.rs`: stand-ins for the `clubcraft-core` / `clubcraft-audio` items the separator uses; data in `%LOCALAPPDATA%\djcraft` (`DJCRAFT_HOME`), models in `...\djcraft\Models` (`DJCRAFT_MODELS`).
- `crates/djcraft-stems/src/live.rs`: per-deck live jobs (cache lookup by content hash, empty `StemTrack` handed to the deck at once, `separate_into` from the playhead, one separation per process, separators cached per quality, partial save on cancel).
- `crates/djcraft-stems/src/ops.rs`: `models_status`, `stems_status`, `separate_file`, shared by CLI and MCP.
- Deck: stems engage only when they change something (a gain off unity or a stem-scoped FX); otherwise the original plays with keylock as before.
- App: the deck STEMS button and track loading use native live separation; legacy `.djcraft-stems/` WAVs still load first.
- CLI: `stems-separate`, `stems-status`, `models-status`. MCP: `djcraft_stems_separate`, `djcraft_stems_status`, `djcraft_models_status`.

### Deviations from the plan

- Stem commands are not `djcraft-core` `Command` variants: core would then pull ONNX Runtime into every core user. CLI and MCP share `djcraft_stems::ops` instead.
- `symphonia` stays 0.5 in `djcraft-audio`; `djcraft-stems` uses 0.6 (FLAC cache only) as upstream.
- GPU: see below. Phase 1 ships on the ONNX Runtime CPU backend.

### Measured on Goliath (Ryzen 24-core, RTX 4090, Windows), synthetic 5:17 test track

| Run | Result |
|---|---|
| Fast, CPU, live from 1:00 | chunk under the playhead ready after 9.8 s; whole track 148 s (2.1x realtime) |
| Fast, CPU, 30 s clip, 8 / 16 / 20 ORT threads | 16.6 s / 18.9 s / 24.9 s (8 threads is best; memory-bound) |
| Progressive vs full, Fast | chunk ready after 2 of 6 model runs; 78-92 dB SDR agreement (16-bit chunk storage + CPU thread order), passes the upstream 70 dB criterion |
| Progressive vs full, High | chunk ready after 5 of 16 model runs; 87.6-97.2 dB SDR |
| DirectML (ORT 1.20, RTX 4090), self-check vs CPU | -107.4 dB (opt level 1, 3), -64.2 dB (opt 0): wrong output, with both the Windows DirectML 1.15.6 and NuGet 1.15.4 DLLs. The self-check correctly falls back to CPU |

### Open GPU options (not done)

1. Newer `ort` (ONNX Runtime 1.22+) and re-test DirectML.
2. CUDA execution provider: ORT 1.20 needs the CUDA 12 runtime + cuDNN 9; Goliath has CUDA 13.3 only.
3. Find the DirectML op that breaks HTDemucs (bisect the graph) and report it upstream to ClubCraft and ONNX Runtime.
