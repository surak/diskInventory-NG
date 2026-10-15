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

## Build & run

```sh
cargo run --release            # start with the volume list
cargo run --release -- ~/Downloads   # scan a folder right away
```

## License

GPL-3.0-or-later, like the original.
