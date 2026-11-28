# DJCraft - ClubCraft parity plan (live stemming first)

Date: 2026-11-28
Status: plan, not started
Source studied: `beneaze/ClubCraft` v0.3.5 (`9afec47`, 2026-11-24), read-only mirror at `D:\Dev\repos\_upstream\ClubCraft` (refreshed by `mcp-central-docs/scripts/refresh-upstream-mirrors.ps1 -All`).
Context: `mcp-central-docs/research/2026-11-28-craft-tracker-survey.md` (ClubCraft is an ArtCraft community candidate, #2339).

## 1. Why

ClubCraft is the strongest open DJ app in the Craft family and overlaps DJCraft head-on. Its core advantage is real stem separation done natively in Rust, progressively, while the track plays. DJCraft's PRD promises "real-time 4-stem separation", but today it shells out to Python `demucs` (`DEMUCS_BIN` or `stems-mcp/.venv`) as an offline batch job and plays sidecar WAVs. This plan takes everything DJCraft lacks from ClubCraft, starting with live stemming, and keeps everything DJCraft already does better.

## 2. Licence and attribution (decides what we may copy)

| Item | Licence | Consequence |
|---|---|---|
| ClubCraft code | MIT OR Apache-2.0 (same as DJCraft) | Port code directly. Keep ClubCraft's copyright line in every ported file header and in `NOTICE`; add rows to `ATTRIBUTION.md` with the upstream commit each port came from. |
| Demucs v4 weights (`htdemucs`, `htdemucs_ft`, `htdemucs_6s`) | MIT (Meta) | Ship or download freely. |
| Mel-Band RoFormer vocal checkpoint (Kimberley Jensen, HF) | MIT | Same. |
| ZFTurbo Mel-Band RoFormer model code (export only) | MIT | Only needed if we ever re-export; we will not. |
| All-In-One structure model (Kim, Nam) | MIT | Optional section model. |
| COCOLA mashup re-ranker | MIT weights, trained partly on MoisesDB (non-commercial) | **Excluded by default.** Rule-based mashup score only; COCOLA stays an opt-in local file. |
| Signalsmith Stretch | MIT | Use the crates.io wrapper `signalsmith-stretch` (colinmarc, 0.1.3); no vendored C++ in DJCraft. |
| ONNX Runtime, DirectML | MIT / Microsoft package terms | Runtime libraries, listed in `ATTRIBUTION.md`. |
| ClubCraft DJ.Studio importer | MIT; rebuilt clean-room in Oct 2026 per its `docs/clean-room.md` | Port the current version only (never anything from its git history before the clean-room rebuild). |

DJ.Studio, VirtualDJ and Mixed In Key are trademarks of their owners; keep ClubCraft's non-affiliation line in `NOTICE`.

## 3. Feature matrix

Legend: **Have** = DJCraft has it at ClubCraft level, **Partial** = exists but weaker, **Gap** = missing.

| Area | ClubCraft | DJCraft today | Status | Action |
|---|---|---|---|---|
| Stem separation engine | Native Rust STFT/iSTFT + HTDemucs/RoFormer on ONNX Runtime (DirectML GPU on Windows, CPU fallback) or CoreML (macOS) | Python `demucs` subprocess, `htdemucs` only | Gap | Port `crates/stems` as `djcraft-stems` (Phase 1) |
| Live / progressive stemming | `separate_into` fills a shared `StemTrack` chunk by chunk from the playhead; Fast: chunk under playhead after ~2 model runs | None (offline batch, then reload) | Gap | Phase 1 core deliverable |
| Stem playback math | Residual trick: `out = r*orig + sum((g_s - r)*stem_s)`; all gains 1 = bit-exact original; ramp on missing chunks with lookahead | Plain `sum(g_s * stem_s)` replaces the original whenever stems exist (loses the residual, never bit-exact) | Partial | Port `deck.rs` `Source` logic into `djcraft-audio` |
| Quality presets | Fast / High / Ultra / Best (RoFormer vocals) | One (`htdemucs`) | Gap | Phase 1 (Fast, High), Phase 2 (Ultra, Best) |
| Stem cache | 8-channel 16-bit FLAC `<hash>.stems.flac`, content-hash identity, partial-chunk resume | 4 float WAVs in `.djcraft-stems/` sidecars | Partial | New cache + keep reading old sidecars (no data loss) |
| Model delivery | Model packs on `models-v<N>` releases, size + SHA-256 checked, staged install, user models folder | Needs a Python venv | Gap | Port `models.rs`; mirror the ONNX pack to our own release |
| Background job system | GPU queue (separation) + CPU pool (analysis), priorities, cross-process `heavy.lock` | Ad hoc threads | Partial | Port `jobs.rs` + `pipeline.rs` pattern |
| Beat grid | 738-line constant-tempo detector, uses stems when present | Onset-energy autocorrelation | Partial | Port `analysis/beatgrid.rs` (Phase 2) |
| Key | Dedicated detector (365 lines) | Krumhansl-Schmuckler via Goertzel chroma | Partial | Port, benchmark both on imported Rekordbox/Traktor keys |
| Loudness | EBU R128 / BS.1770-4 integrated | "LUFS-style" auto-gain | Partial | Port `loudness.rs` |
| Energy 1-10 | Yes | No | Gap | Port `energy.rs` |
| Structure / sections | Bar features + phrase segmentation + optional All-In-One model | 8/16-bar RMS novelty phrases | Partial | Port `structure.rs`, `allinone.rs`, `labels.rs` |
| Automatic hot cues | 8 cues: Start, Mix In, Vocal, Build, Drop, Break, Mix Out (+1) | Manual, 4 cue pads | Gap | Port `cues.rs`; widen DJCraft to 8 hot cues |
| Vocal map | 16th-note vocal activity from the vocal stem | No | Gap | Port `vocals.rs` (needed by mashups and the Vocal cue) |
| Keylock / key shift | Signalsmith Stretch, key shift, match key | WSOLA keylock | Partial | `signalsmith-stretch` crate; keep WSOLA as low-CPU fallback |
| Stem pads and stem echo | Acapella / instrumental / drums-only pads, stem echo send | Stem knobs, stem-scoped FX | Partial | Add pads + per-stem echo send |
| Loop roll, CDJ cue modes, hold, CUP | Yes | Loops, slip, beatjump; cue behaviours partly | Partial | Port deck transport details |
| Automix | Plays a planned mix with real mix points | Queue crossfade | Partial | Port `automix.rs` + `conductor.rs` |
| RAM pinning | Tracks in use kept resident | No | Gap | Port `pin.rs` minus the unix-only `mlock` (Windows no-op) |
| Output routing | Multichannel or separate master/cue devices, clock-drift correction, alignment, recovery | Dual-device PFL via ring buffer, split mono cue | Partial | Port `output.rs` ideas (Phase 3) |
| Studio (DJ.Studio-style mix designer) | Timeline, lanes, transitions with per-stem automation curves, presets, harmonize (simulated annealing over key, tempo, energy arc), savepoints, duplicate, offline render WAV/FLAC/MP3 + cue sheet | None | Gap | New Studio mode (Phase 4) |
| Sample lanes on timeline | Yes | Sampler pads (live) | Gap | Phase 4 |
| Mashup finder | Vocal units x instrumental slots x key shifts, rule score | None | Gap | Phase 5 (no COCOLA by default) |
| Extract range / stems to WAV | Yes | No | Gap | Phase 5 |
| DJ.Studio import | Library cues + projects as mixes | No | Gap | Phase 5 |
| VirtualDJ export | Beat grids, keys, hot cues, automix points into `database.xml` | `.vdjfolder` crates, VDJ mapping import | Partial | Phase 5 |
| MCP | Progress notifications, cancellation, heavy lock shared with app, `.library-changed` signal | Shared `Command::dispatch`, no progress/cancel | Partial | Phase 6 |

### DJCraft keeps (ClubCraft does not have these)

4 decks, video DJ and 3-monitor booth, DVS timecode vinyl, platter scratch engine, 4x8 sampler, beatmaker and chopper, UDP beat link, DMX lighting and laser, 6 skins, MIDI host with 8 profiles + VDJ/Mixxx XML import, Rekordbox XML and Traktor NML interop, smart crates, booth AI chat, WASM build. Nothing here is removed; ClubCraft features are added beside them.

## 4. Live stemming - target architecture

```
djcraft-core      (UI-free)  Command::{StemSeparate, StemCancel, StemStatus, ModelsInstall, ModelsStatus}
     |
djcraft-audio     (UI-free)  StemTrack (chunked, OnceLock per chunk, atomic ready count)
     |                       deck Source: residual mix, lookahead ramp, stem sends
     |
djcraft-stems     (NEW, UI-free, native only)
                             stft.rs, pipeline.rs (apply_model semantics), roformer.rs,
                             backend/onnx.rs (DirectML -> CPU), cache.rs (8ch i16 FLAC),
                             model.rs (manifests), models download + verify
     |
djcraft-app / djcraft-mcp / djcraft-cli  -> job queue (GPU queue + CPU pool), progress UI
djcraft-wasm  -> must NOT depend on djcraft-stems (ORT is not wasm); xtask layers enforces it
```

Flow when a track is loaded on a deck:

1. Hash the file; if `<hash>.stems.flac` exists, map it into a `StemTrack` (instant).
2. Else if legacy `.djcraft-stems/<track>/{vocals,drums,bass,other}.wav` exist, load them into a `StemTrack` (`from_planar`). Old sidecars keep working; an optional "Convert legacy stems" command rewrites them to the new cache and leaves the WAVs alone.
3. Else enqueue `separate_into(Quality::Fast)` on the GPU queue at top priority, segments ordered outward from the playhead. Chunks publish as they complete; the deck plays the original immediately and stem knobs take effect per chunk, ramping in where the next region is ready (ClubCraft `STEM_LOOKAHEAD`/`STEM_RAMP`).
4. When Fast completes, optionally queue High in the background and swap `Arc<StemTrack>` at a chunk boundary (setting: "Upgrade live stems to High").
5. Persist partial results (`save_partial_stem_track`) so a reload resumes, not restarts.

Rules:
- Separation threads run at background priority, and ORT intra-op threads are capped (setting, default cores - 2) so the cpal callback never starves.
- Exactly one GPU separation at a time, shared across app, CLI and MCP through a `heavy.lock` file in the data folder.
- Models live in `%LOCALAPPDATA%\djcraft\Models` (override `DJCRAFT_MODELS`), outside the app folder, so they survive app updates.
- The Python `demucs` subprocess path stays only as a hidden fallback for one release, then is deleted with a CHANGELOG note.
- The synthetic demo stems in `djcraft-audio/src/lib.rs` stay for `--smoke` and demo mode, clearly labelled.

Backends, in order:
- Windows: ORT DirectML (any DX12 GPU, including Goliath's RTX 4090) -> ORT CPU.
- Linux: ORT CPU first; CUDA execution provider as an optional cargo feature later.
- macOS: CoreML port needs `objc2` FFI with `unsafe`; it goes into a separate opt-in crate `djcraft-stems-coreml` with a documented `unsafe` exception, Phase 7. Until then macOS uses ORT CPU.

`#![forbid(unsafe_code)]` stays on every DJCraft crate. ClubCraft's only `unsafe` in the ONNX backend is an optional shared ORT arena allocator; v1 drops it and measures RAM. If RAM is a problem, the allocator goes into a one-file exception crate, never into `djcraft-stems` itself.

Dependency pins to reconcile while porting: `ort =2.0.0-rc.9` (pre-release, pinned exactly as upstream), `realfft 3.5`, `flacenc 0.5`, `symphonia` (DJCraft 0.5, ClubCraft 0.6: move DJCraft to 0.6 in Phase 0), `cpal` (DJCraft 0.15, ClubCraft 0.18: port logic, do not swap engines).

## 5. Phases

Timelines are AI-assisted working days. Every phase ends with `cargo run -p xtask -- ci` green and the gates listed.

### Phase 0 - groundwork (0.5 day)
- Add `NOTICE` lines and `ATTRIBUTION.md` rows (models, Signalsmith, ORT, DirectML, ClubCraft commit `9afec47`).
- Port log `docs/CLUBCRAFT_PORT_LOG.md`: one row per ported file with upstream path and commit.
- Bump `symphonia` to 0.6; create empty `crates/djcraft-stems` with `forbid(unsafe_code)` and lints; extend `xtask layers` so `djcraft-stems` stays UI-free and `djcraft-wasm` never depends on it.
- Model hosting: download ClubCraft's ONNX pack once, verify its SHA-256 against their `model-packs.json`, re-publish on a `sandraschi/djcraft` release `models-v1` (MIT allows), record both URLs.
- Gate: workspace builds; wasm build still green.

### Phase 1 - live stemming core (3 days)
- Port `stft.rs`, `pipeline.rs`, `model.rs`, `backend/{mod,onnx}.rs`, `cache.rs`, `lib.rs` (Fast, High), `StemTrack` + `StemChunk`.
- Port the deck `Source` residual mix and lookahead ramp into `djcraft-audio`; replace the plain stem sum in `output.rs`; keep stem-scoped FX and stem crossfader modes working on top.
- Job queue: GPU queue with playhead-first ordering, CPU pool, `heavy.lock`, progress events into the UI.
- Models: Settings > Stems > Download models (progress, size + checksum, staging folder, Install from file...).
- Legacy sidecar loading + optional convert command.
- New commands on the bus, CLI subcommands (`djcraft-cli stems separate|status|cancel`, `models install|status`) and MCP tools with the same names.
- Gates (tests): residual unity bit-exact; all gains 0 = silence; progressive output == full `separate` output (Fast/High); FLAC cache round trip; legacy WAV sidecar loads; deck plays with missing chunks and ramps without clicks; `djcraft-wasm` builds without ORT.
- Gate (measured on Goliath, recorded in the PR): time to first ready chunk at the playhead on DirectML for Fast and High; zero audio underruns during a full separation with two decks playing.

### Phase 2 - analysis upgrade (2 days)
- Port `beatgrid.rs`, `key.rs`, `loudness.rs`, `energy.rs`, `structure.rs`, `cues.rs`, `vocals.rs`, `waveform.rs`, `frontend.rs`, `dsp.rs`; optional `allinone.rs` + `labels.rs` with the model in the pack.
- Ultra and Best quality presets (RoFormer vocals).
- Hot cues widen from 4 to 8 per deck; auto cues fill empty slots only, never overwrite user cues.
- Benchmark: compare old vs new BPM/key against keys and grids already stored in imported Rekordbox XML / Traktor NML for the same files (comparison of results, allowed by the clean-room rules); keep the winner per metric, old code stays as fallback.
- Gates: analysis is deterministic on fixtures; existing `track_cues` rows unchanged after re-analysis.

### Phase 3 - live deck parity (1.5 days)
- `signalsmith-stretch` keylock and key shift / match key; WSOLA kept as "Low CPU" option.
- Stem pads (acapella, instrumental, drums only), per-stem echo send, loop roll, CDJ / hold / CUP cue modes.
- Automix from planned mix points (`automix.rs`, `conductor.rs`), RAM pinning (`pin.rs`), output clock-drift handling ideas from `audio-output.md`.
- Gates: engine tests ported from ClubCraft `engine/tests` where the API matches; kittest for the new pads.

### Phase 4 - Studio mode (4-5 days)
- New top-level "Studio" view next to the live booth: timeline lanes, clips, transitions with per-stem and per-EQ-band automation curves (`studio_timeline.rs`, `studio_transition.rs`, `curve_edit.rs`), presets (stem blend, bass swap, crossfade, vocal over, filter out, echo out, cut, mashup), harmonize (simulated annealing over Camelot key, tempo, energy arc), savepoints, duplicate, sample lanes.
- Offline render through the same deck/mixer code (`render.rs`) to WAV/FLAC/MP3 with a cue sheet.
- Studio mix -> live automix handoff.
- Gates: render of a 3-track fixture mix is deterministic (hash); undo/redo across all edits.

### Phase 5 - interop, mashups, extract (2 days)
- DJ.Studio importer (current clean-room version only) and VirtualDJ `database.xml` export of grids, keys, hot cues, automix points.
- Mashup finder with rule-based score (COCOLA opt-in only, off by default, with the MoisesDB note in the UI).
- Extract a range, the full sound or chosen stems to WAV.
- Gates: importer fixtures from ClubCraft tests; VDJ export round-trips through DJCraft's existing VDJ reader.

### Phase 6 - MCP/CLI parity, docs, release (1 day)
- MCP: `notifications/progress` for separation/analysis/render, `notifications/cancelled`, `.library-changed` signal so the open app reloads; protocol versions as in ClubCraft.
- Update README, PRD (the "real-time stem separation" claim becomes true), `target-app-parity.md`, `AUDIO_AND_MIXING.md`, `gaps.md`, CHANGELOG, Help suite.
- Release: plain exes as release assets (no zip as the only download); models stay a separate `models-v1` release.

### Phase 7 - optional
- `djcraft-stems-coreml` (macOS GPU) as an opt-in crate with a documented `unsafe` exception.
- CUDA execution provider feature for Linux/Windows if DirectML underperforms on the 4090.
- Upstream: offer ClubCraft anything we improve (4-deck stem playback, DVS + stems) as PRs.

Total: about 14-15 working days to full parity; live stemming alone (Phases 0-1) about 3.5 days.

## 6. Risks and how we handle them

| Risk | Handling |
|---|---|
| `ort` is a pinned release candidate | Pin exactly like upstream; a later bump is its own PR with the progressive-equality test as the gate. |
| DirectML speed on the 4090 unknown | Measure in Phase 1; CUDA EP fallback in Phase 7; Fast preset for live, High in background. |
| ClubCraft keeps moving (v0.3.5 now) | Port log with commits; monthly diff of upstream `crates/stems` and `crates/analysis` against the log. |
| Model pack hosting disappears upstream | Our own `models-v1` mirror with checksums from day one. |
| RAM with 4 decks x stems | 8ch 16-bit chunks (~half of f32), pin only loaded tracks, measure with 4 decks in Phase 1. |
| Engine differences (cpal 0.15 vs 0.18, DJCraft mixer mutex vs ClubCraft lock-free rings) | Port DSP and stem logic, not the engine; a lock-free rework is out of scope here. |
| Old sidecar stems | Read forever; conversion is opt-in and never deletes the WAVs. |
| Licence drift | `ATTRIBUTION.md` + `NOTICE` updated in the same commit as each port; COCOLA stays opt-in. |
