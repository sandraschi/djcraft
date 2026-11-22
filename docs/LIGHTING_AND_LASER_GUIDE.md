# AI Lighting Assistant, Art-Net / sACN & Budget Gear Buying Guide

Back to [README.md](../README.md) | [CLI & MCP Reference](CLI_AND_MCP.md)

DJCraft includes a **hands-free AI Lighting Assistant** (`crates/djcraft-core/src/lighting.rs` + `AI Lighting (Art-Net/sACN)` dock tab) that transmits live **Art-Net 4 (UDP `6454`)** and **sACN E1.31 (UDP `5568`)** DMX512 frames synced to your Master BPM, beat phase, and deck energy.

---

## 1. Budget-Friendly Gear Buying Guide (So It Doesn't Break the Bank)

### A. The Bridge: Laptop -> Real DMX512 Lights (~EUR 45-120)
Your fixtures use 3-pin XLR DMX512 cables. To connect DJCraft's **Art-Net / sACN** over Ethernet/Wi-Fi without proprietary dongles:
- **Best Budget Art-Net Node (~EUR 50-80)**: **Showtec Net-2/3 Pocket**, **Eurolite Art-Net-DMX Node 1**, or **PKnight / Enttec ODE** (1-universe Ethernet-to-3-pin-XLR Art-Net box). Plugs into your router or laptop Ethernet port, zero drivers.
- **Zero-Cost Software Bridge (if you already own a cheap USB-DMX dongle like Enttec Open DMX / uDMX)**: Run free **QLC+** (`qlcplus.org`) in the background with `Art-Net Input` -> `USB-DMX Output`.
- **DMX Cables**: Buy proper **110-ohm DMX cables** (3-pin XLR, not mic cables, especially over 5m) + one **120-ohm DMX terminator** plug (~EUR 4) for the last fixture.

### B. Starter Club Rig (Scale One Piece at a Time)
1. **2x LED RGBW Wash / Par Bars (~EUR 35-60 each)**: e.g., **Stairville Wild Wash / LED Par 56 / Cameo Root Par 4**. Put them on DMX Ch `1-4` and `5-8` (4-channel RGBW mode). Instant room color!
2. **2x Mini Moving Heads (~EUR 80-140 each)**: e.g., **Stairville MH-x25 / Fun Generation PicoSpot / Cameo HydraBeam**. Set them to 8-channel mode (Pan, Tilt, Dimmer, RGBW, Strobe) on Ch `9-16` and `17-24`.
3. **1x Mini Haze / Fog Machine (~EUR 45-90)**: **Crucial!** Moving-head beams and lasers are invisible in clean air. A cheap water-based **mini hazer** (not thick party fog) makes a EUR 100 light look like a EUR 2,000 club rig.
4. **1x Entry DMX Show Laser (~EUR 80-160)**: e.g., **Laserworld EL-230RGB MK2** or **Cameo Wookie 200** with DMX mode.

---

## 2. Laser Safety (Important)
- Never aim any laser below **2.7m** height or into eyes/cameras/mirrors ("crowd scanning" requires MPE-certified gear + scan-fail hardware).
- Aim lasers **upward onto the ceiling/wall above people's heads**, or use a **galvo-safe beam zone**.
- In DJCraft, keep **Strobe** disabled in `Chill Warm` mode and use **`BLACKOUT`** if anything looks wrong.

---

## 3. Using the AI Lighting Assistant in DJCraft
1. Open the **`AI Lighting (Art-Net/sACN)`** lower dock tab.
2. Select **`Art-Net :6454`** (or `sACN :5568`), enter your Art-Net node's IP (or `255.255.255.255` for LAN broadcast), and check **`Transmit Live UDP`**.
3. Pick an AI Auto-Pilot Mode:
   - **`AI Auto Club`**: Smooth BPM-synced pan/tilt sweeps + RGBW washes + subtle drop strobes.
   - **`Chill Warm`**: Slow breathing amber/violet washes, zero strobe.
   - **`Techno Strobe`**: Sharp beat-locked cyan/red/white pulses + strobe bursts.
   - **`Laser Tunnel`**: Cyan/ice beam focus + laser DMX pattern sweeps.
   - **`Blackout`**: Immediate all-zero DMX frame.
4. Or control from CLI / MCP:
   ```powershell
   djcraft-cli lighting on --protocol artnet --target-ip 255.255.255.255 --mode techno --dimmer 0.85
   djcraft-cli lighting blackout
   ```
