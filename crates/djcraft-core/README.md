# `djcraft-core` (L1 DJ Command Bus & Library Engine)

Pure Safe Rust (`#![forbid(unsafe_code)]`) L1 domain engine for **DJCraft Pro**:

- Unified `Command::dispatch` bus shared by `djcraft.exe` (GUI), `djcraft-cli.exe` (CLI), and `djcraft-mcp.exe` (MCP stdio).
- Native Camelot harmonic key analysis, BPM & phrase detection, 24-thread parallel crate analyzer, Smart Crates, duplicate detector, and live setlist history (`.txt`, `.cue`, `.csv`).
- Import/export adapters for Serato `.crate`/ID3, VirtualDJ, AlphaTheta Rekordbox XML, and Traktor Pro 4 NML.
- Hands-free AI Lighting engine (Art-Net 4 UDP `6454` and sACN E1.31 UDP `5568`) and MIDI profile/rule mapping (12 controller profiles + LED out + XML import).
