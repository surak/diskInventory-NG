# Disk Inventory NG

A Rust re-imagining of [Disk Inventory X](https://www.derlien.com) — the
macOS disk usage utility that shows where your disk space has gone using
cushion treemaps (layout after KDirStat, inspired by WinDirStat).

## Features

- **Volume list** on startup (refreshes as volumes mount/unmount), or open any folder
- Background scan with live progress and cancel; hard links counted once,
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

## Project layout

| File | Purpose |
|------|---------|
| `src/scan.rs` | Recursive scanner, node tree, file kind statistics |
| `src/treemap.rs` | Squarified layout and cushion shading |
| `src/kinds.rs` | Colour palette for file kinds |
| `src/volumes.rs` | Mounted volume discovery |
| `src/app.rs` | egui user interface |

## Credits

Inspired by Disk Inventory X by Tjark Derlien. This is an independent
implementation and contains no code from the original.

## License

MIT — see [LICENSE](LICENSE).
