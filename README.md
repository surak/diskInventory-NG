<p align="center">
  <img src="docs/icon.png" width="160" alt="Disk Inventory NG icon">
</p>

<h1 align="center">Disk Inventory NG</h1>

<p align="center">
  <b>See where your disk space has gone.</b><br>
  A fast, native disk usage visualizer for macOS, written in Rust —<br>
  a modern take on the classic <a href="https://www.derlien.com">Disk Inventory X</a>.
</p>

<p align="center">
  <a href="../../releases/latest"><img alt="Download" src="https://img.shields.io/badge/download-DMG-blue?logo=apple"></a>
  <a href="LICENSE"><img alt="MIT License" src="https://img.shields.io/badge/license-MIT-green"></a>
  <img alt="macOS 11+" src="https://img.shields.io/badge/macOS-11%2B-lightgrey?logo=apple">
  <img alt="Rust" src="https://img.shields.io/badge/made%20with-Rust-orange?logo=rust">
</p>

<p align="center">
  <img src="docs/screenshot.png" alt="Disk Inventory NG showing the folder hierarchy, a cushion treemap and file kind statistics">
</p>

---

Disk Inventory NG scans a volume or folder and draws every file as a
rectangle in a **cushion treemap**: the bigger the file, the bigger the
rectangle; the colour tells you what kind of file it is. Huge forgotten
downloads, caches and VM images jump right out.

## ✨ Features

- **Cushion treemap** — squarified layout with 3D cushion shading
  (the [KDirStat](https://kdirstat.sourceforge.net/) algorithm), so nested folders stay readable.
- **Three synchronized views** — folder hierarchy, treemap and file-kind
  statistics. Select a file anywhere and the other views follow.
- **Fast** — scans directories in parallel on all CPU cores
  (≈1.4 million entries in under 7 seconds on an Apple Silicon Mac, warm cache).
- **File kinds** — every extension with its total size and file count,
  coloured like the treemap. Click one to list its files.
- **Selection list** — the largest files, filterable by kind and searchable by name.
- **Inspector** — size, share of the total, file count, kind and age of the selection.
- **Zoom** into any folder (double-click or <kbd>⌘</kbd><kbd>↓</kbd>) and back out (<kbd>⌘</kbd><kbd>↑</kbd>).
- **Act on it** — Reveal in Finder (<kbd>⌘</kbd><kbd>R</kbd>), Open, or Move to Trash;
  the view updates instantly without a rescan.
- **Volume list** on launch that follows disks as they are mounted and unmounted.
- Counts hard links once and doesn't wander into other volumes.

## 📦 Install

1. Download the latest **`DiskInventoryNG-x.y.z.dmg`** from
   [Releases](../../releases/latest).
2. Open it and drag **Disk Inventory NG** onto **Applications**.

<p align="center"><img src="docs/dmg.png" width="560" alt="The installer disk image"></p>

> [!NOTE]
> Release builds are ad-hoc signed, not notarized. On first launch,
> right-click the app and choose **Open**, or run
> `xattr -dr com.apple.quarantine "/Applications/Disk Inventory NG.app"`.

> [!TIP]
> To scan your whole disk, give the app **Full Disk Access** in
> *System Settings → Privacy & Security*. Without it, macOS hides
> protected folders (Mail, Safari, …) and they are skipped.

## 🛠 Build from source

Requires macOS 11+ and a stable [Rust](https://rustup.rs) toolchain.

```sh
git clone https://github.com/surak/diskInventory-NG.git
cd diskInventory-NG
cargo run --release                 # start with the volume list
cargo run --release -- ~/Downloads  # or scan a folder right away
```

### App bundle and DMG

```sh
scripts/bundle-macos.sh             # → target/release/bundle/Disk Inventory NG.app
scripts/make-dmg.sh                 # → target/release/bundle/DiskInventoryNG-<version>.dmg
scripts/make-dmg.sh universal       # arm64 + x86_64 (rustup target add x86_64-apple-darwin)
```

`make-dmg.sh` lays out the installer window through Finder, so the first run
may ask to allow your terminal to control Finder.

The app icon — an isometric 3D treemap, in homage to the original — is not a
bitmap in the repo: the app draws it itself with its own treemap renderer
(`disk-inventory-ng --write-icon icon.png 1024`).

### Benchmark the scanner

```sh
cargo run --release -- --scan-only ~/Library
```

## 🧭 How it works

| File | Purpose |
|------|---------|
| [`src/scan.rs`](src/scan.rs) | Parallel directory walk (rayon), flattened into an arena tree with per-kind statistics |
| [`src/treemap.rs`](src/treemap.rs) | Squarified layout and cushion shading, rendered to a texture |
| [`src/app.rs`](src/app.rs) | The [egui](https://github.com/emilk/egui) user interface |
| [`src/icon.rs`](src/icon.rs) | Procedural isometric treemap app icon |
| [`src/volumes.rs`](src/volumes.rs) | Mounted volume discovery |
| [`src/kinds.rs`](src/kinds.rs) | File kind colour palette |
| [`scripts/`](scripts) | `.app` bundling and DMG creation |

Sizes are *allocated* sizes on disk (blocks × 512), like Finder's "on disk" figure.

## 🙏 Credits

- [Disk Inventory X](https://www.derlien.com) by Tjark Derlien — the original
  macOS app this project is inspired by.
- [KDirStat](https://kdirstat.sourceforge.net/) and
  [WinDirStat](https://windirstat.net/) for the cushion treemap approach.
- *Cushion Treemaps* (van Wijk & van de Wetering, 1999) and
  *Squarified Treemaps* (Bruls, Huizing & van Wijk, 2000).

Disk Inventory NG is an independent implementation and contains no code from
the original projects.

## 📄 License

[MIT](LICENSE) © 2026 Alexandre Strube
