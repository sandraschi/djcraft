# DJCraft

**DJCraft** is an ArtCraft-tier sovereign 4-deck digital mixing, real-time stem extraction, and harmonic transition studio designed to run locally on Goliath without cloud latency or subscription paywalls.

Features:
- **4-Deck Virtual Engine**: Direct fader control, pitch bends, transient-based BPM tracking, and master crossfader curves.
- **Real-Time Stem Isolation**: Dedicated stem gain multipliers for Vocals, Drums, Bass, and Other instrumental tracks.
- **Camelot Harmonic Wheel Intelligence**: Automated key clash prevention and harmonic compatibility algorithms ($8A \leftrightarrow 8B$, $8A \leftrightarrow 9A$, relative major/minor switches).
- **FastMCP Protocol**: Port `14118` interface for multi-agent autonomous performance swarms.

---

## Workspace Architecture

1. `crates/djcraft-audio`: DSP abstractions, Camelot wheel logic, and stem isolation structures.
2. `crates/djcraft-core`: 4-deck orchestrator, harmonic transition planner, and track library indexing.
3. `crates/djcraft-mcp`: FastMCP JSON-RPC server on port `14118`.
4. `crates/djcraft-app`: Native GUI (`eframe`/`egui`) and CLI tool (`djcraft-cli`).

---

## Verification & Usage

```powershell
# Run test suite
cargo test

# Headless smoke test
./target/debug/djcraft.exe --smoke

# CLI Telemetry
./target/debug/djcraft-cli.exe status
./target/debug/djcraft-cli.exe transition
./target/debug/djcraft-cli.exe camelot 8A 9A
```
