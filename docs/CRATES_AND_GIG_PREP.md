# Crates, Gig Prep & Serato / VirtualDJ Interop

Back to [README.md](../README.md)

---

## 1. Dedicated 2-Pane Crates & Gig Prep Workspace

Click **`CRATES & GIG PREP`** in the top bar to switch from the live mixer to a full-width, file-oriented library and setlist workspace backed by SQLite (`%LOCALAPPDATA%\djcraft\crates.db`).

### Left Pane: Hierarchical Crate & Subcrate Tree
- Unlimited nested subcrates (`House / Deep / Afterhours`) with live track counts.
- Full Crate CRUD toolbar:
  - **`+ Root`**: Create a top-level crate.
  - **`+ Sub`**: Create a nested subcrate under the selected crate.
  - **`Rename`**: Rename any crate and recursively rewrite all descendant `full_path` entries.
  - **`Duplicate`**: Clone a crate while preserving its exact manual gig track order.
  - **`Delete`**: Delete a crate (or subcrate subtree).
- **Drop Target**: Drag any track row from the right table onto any crate in the left tree to add it.

### Right Pane: Persistent Gig-Order Track Table
- **Manual Gig Order (`Gig Order (#)`)**: Reorder tracks inside any crate by dragging the `[::]` grip handle or clicking `^` / `v` nudge buttons. Order persists in SQLite (`crate_tracks.position`).
- **Filtering & Sorting**: Live search, BPM min/max filter, **`Harmonic (Deck A)`** Camelot key filter, star rating filter, and multi-column sorting (`Gig Order (#)`, `Title`, `Artist`, `BPM`, `Key`, `Rating`, `Date Added`).
- **One-Click Deck Actions**: Load any track directly onto `A`, `B`, `C`, or `D`, run Symphonia `BPM` or `dettato` (`Det`) analysis, adjust inline star rating, remove from crate (`Rem`), or delete from library (`Del`).

---

## 2. Drag-and-Drop Import & Folder Scanning

- **Windows Explorer Drop**: Drag audio files or folders directly from Windows Explorer onto the DJCraft window to import them into the currently selected crate.
- **Toolbar Actions**: `+ Add Audio Files...`, `Scan Folder...`, `Batch Stem Crate`, and `Batch Stem Folder...`.
- Sidecar stem directories (`.djcraft-stems/`) are automatically skipped during folder scans.

---

## 3. Serato DJ & VirtualDJ Interop

DJCraft reads and writes native crate formats directly in Safe Rust (`crates/djcraft-core/src/crates_db.rs`):

| Format | Direction | Details |
| :--- | :--- | :--- |
| **Serato DJ `.crate`** | Import & Export | Binary UTF-16BE TLV (`vrsn`, `otrk`, `ptrk`) + `%%` nested subcrate naming |
| **VirtualDJ `.vdjfolder`** | Import & Export | XML `<VirtualFolder>` / `<song path="..." />` |
| **Extended M3U8 (`.m3u` / `.m3u8`)** | Import & Export | Standard playlist interop |

### CLI Examples
```powershell
djcraft-cli crate-list
djcraft-cli crate-import-serato "D:\_Serato_\Subcrates\House%%Deep.crate"
djcraft-cli crate-export-serato 1 "D:\export.crate"
djcraft-cli crate-import-vdj "D:\VirtualDJ\Folders\Club.vdjfolder"
djcraft-cli crate-export-vdj 1 "D:\export.vdjfolder"
```
