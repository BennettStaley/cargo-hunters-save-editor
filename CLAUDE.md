# CLAUDE.md

Guidance for Claude Code (claude.ai/code) when working in this repository.

## What this is

A local, offline desktop editor for the Cargo Hunters game save (`offline.save`,
a JSON file under `%USERPROFILE%\AppData\LocalLow\OrderOfMeta\Cargo Hunters\`).
It edits inventory, the robot's body parts and worn gear, repairs/refills items,
splits/moves/deletes, adds catalog items, and edits character XP/skills. The UI
is a Tarkov-style screen: a robot paperdoll (body parts on a silhouette + gear
slots) on the left, the stash grid on the right.

**The app is built with Tauri 2 — a Rust core + a SolidJS/TypeScript web
frontend — and ships as a single Windows `.exe` running on the system WebView2.**
It replaced an earlier Python/Tkinter tool, which has been removed (the rewrite
was validated against it during the port; see "Architecture & invariants").

Everything lives under `app/`.

## Layout

```
app/
  engine/            ch_engine — pure-Rust save engine (NO Tauri dep)
    src/io.rs          load/serialize/backup/atomic-write (byte-faithful)
    src/model.rs       source access, container discovery, occupancy, catalog CSV, slot classify
    src/ops.rs         mutations: set fields, move, repair, split, add, delete, account/skills
    src/snapshot.rs    the read-only view the frontend consumes (+ catalog entries)
    src/bin/oracle.rs  dev CLI: dump snapshot/catalog JSON for the frontend mock; roundtrip/op self-checks
  src-tauri/         the Tauri app: thin #[tauri::command] wrappers over ch_engine
    src/lib.rs         commands + in-memory editing session (staged edits, validated save)
  src/               SolidJS frontend
    api.ts             typed bindings over the Tauri commands (+ browser dev fallback)
    icons.ts           VisualName -> Icon_* sprite resolver
    layout.ts          grid placement (true footprint = max(catalog, BaseComponent)) + condition frac
    components/        Paperdoll, VaultGrid, ItemTile, EditBar, CatalogBrowser, CharacterPanel
  public/sprites/    game icon + rig sprites (BodyHUD.png is the paperdoll silhouette)
```

Root holds `all_items_detailed.csv` — the item catalog, embedded into the exe
via `include_str!`. The repo is otherwise Python-free (the original Python tool
and the migration oracle were removed; they live in git history).

## Commands

Run from `app/` (PATH needs Node + `~/.cargo/bin`):
```pwsh
npm install                 # once
npm run tauri dev           # dev (hot-reload frontend + Rust)
npm run tauri build         # single exe -> src-tauri/target/release/app.exe (+ MSI/NSIS installers)
npx tsc --noEmit            # frontend type-check
```

Engine tests (from `app/engine/`):
```pwsh
cargo test --release        # serializer byte-fidelity, round-trip idempotency, ops, slot classify
cargo build --release --bin oracle   # dev CLI to dump snapshot/catalog fixtures for the browser mock
```

## Architecture & invariants

**The save engine is the crown jewel; correctness is non-negotiable (never
corrupt a save).** Two things enforce this:

1. **Byte-faithful serialization.** `serde_json` is built with
   `arbitrary_precision` (numbers kept as their original literal — no float
   reformatting, no >2^53 int loss; plain serde_json's float *parser* disagrees
   with the game/Python by 1 ULP on some values) **and** `preserve_order` (key
   order). A custom 4-space `PrettyFormatter` + no trailing newline matches the
   shape of `json.dumps(indent=4, ensure_ascii=False)`. Untouched numbers are
   preserved byte-for-byte — strictly more faithful than the old Python tool,
   whose load/dump round-trip could shift floats.
2. **Tests.** `cargo test` in `app/engine` covers the serializer's byte-fidelity
   (tricky float, >2^53 int, key order, empty containers, unicode), round-trip
   idempotency, and the edit ops. During the rewrite the engine was also proven
   byte-for-byte against the original Python engine via a differential oracle;
   that scaffolding was removed once the port was complete (it's in git history).
   **Any new or changed mutation must come with a Rust unit test.**

Other conventions:
- Save-format logic lives in `ch_engine` (pure Rust, no Tauri). `src-tauri` only
  wraps it as commands and holds the editing session. The frontend is pure view:
  icon resolution + grid layout happen in TS from the snapshot's raw data.
- Edits are **staged in memory**; nothing touches disk until `save_game`, which
  writes a timestamped backup, writes atomically, then **re-reads and validates**
  the on-disk file equals the working copy before clearing the dirty flag.
- Items live in three sources (`inventory`/`equipment`/`shelter`). Body parts and
  worn gear are top-level equipment items classified by `VisualName`
  (`BodyParts/{Heads,Torsos,LeftArms,RightArms,LeftLegs,RightLegs}`, `Outfits/*`,
  `Weapons/*`, `Tools/*`, `Items/Droid` …). The vault is the backpack's children
  on an 8×28 grid; positions default a missing `Position` axis to 0 and treat
  negatives as off-grid (matching the game).
- New bundled assets: add sprites under `app/public/sprites/`; the catalog CSV is
  embedded from the repo-root `all_items_detailed.csv` via `include_str!`.
