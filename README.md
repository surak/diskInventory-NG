# Disk Inventory NG

<img src="docs/icon.png" width="128" align="right" alt="App icon">

A Rust re-imagining of [Disk Inventory X](https://www.derlien.com) — the
macOS disk usage utility that shows where your disk space has gone using
cushion treemaps (layout after KDirStat, inspired by WinDirStat).

## Features

- **Volume list** on startup (refreshes as volumes mount/unmount), or open any folder
- Fast **parallel scan** across all CPU cores, with live progress and cancel; hard links counted once,
  other volumes (`/Volumes`, `/System/Volumes`) not crossed
- **Three synchronized views** — selecting in one updates the others:
  - folder hierarchy with sizes and percentage bars
  - squarified **cushion treemap** (click to select, double-click / ⌘↓ to zoom in, ⌘↑ to zoom out)
  - **file kind** statistics, coloured like the treemap
- **Inspector** with details of the selected item
- **Selection list**: largest files, filtered by kind and searchable
- Reveal in Finder (⌘R), Open, Move to Trash (tree updates in place)

## Requirements

- macOS (uses `open -R` for Reveal in Finder and the macOS Trash)
- Rust (stable, 2024 edition)

## Build & run

```sh
cargo run --release            # start with the volume list
cargo run --release -- ~/Downloads   # scan a folder right away
```

## Building the macOS app

```sh
scripts/bundle-macos.sh              # native architecture
scripts/bundle-macos.sh universal    # arm64 + x86_64 (rustup target add x86_64-apple-darwin)
```

This produces `target/release/bundle/Disk Inventory NG.app` (ad-hoc signed),
which you can drag into `/Applications`. The icon — an isometric 3D cushion
treemap, in homage to the original — is drawn by the app itself
(`disk-inventory-ng --write-icon icon.png 1024`) and packed with `iconutil`.

To see the full disk, grant the app **Full Disk Access** in System Settings →
Privacy & Security; otherwise protected folders are skipped.

## Benchmarking

```sh
cargo run --release -- --scan-only ~/Library
```

## Project layout

| File | Purpose |
|------|---------|
| `src/scan.rs` | Recursive scanner, node tree, file kind statistics |
| `src/treemap.rs` | Squarified layout and cushion shading |
| `src/kinds.rs` | Colour palette for file kinds |
| `src/volumes.rs` | Mounted volume discovery |
| `src/app.rs` | egui user interface |
| `src/icon.rs` | Procedural isometric treemap app icon |
| `scripts/bundle-macos.sh` | Builds the `.app` bundle |

## Credits

Inspired by Disk Inventory X by Tjark Derlien. This is an independent
implementation and contains no code from the original.

## License

MIT — see [LICENSE](LICENSE).
