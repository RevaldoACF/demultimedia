# demultimedia

A native Rust desktop app for shrinking photo file sizes and converting between image formats without touching a terminal or remembering ffmpeg flags.

Drop a folder or a bunch of photos onto the window, drag the quality and scale sliders, pick a target format, and hit process. By default it writes to a `demultimedia_out` subfolder so your originals stay untouched.

## What it does

- Lowers quality (1 to 100) and optionally downscales resolution (10% to 100%).
- Converts between `jpg`, `png`, `webp`, `gif`, `bmp`, `tiff`, and `svg`.
- Reads `jpg`, `jpeg`, `png`, `webp`, `gif`, `bmp`, `tiff`, `tif`, `ico`, `tga`, `qoi`, `pnm`, and `farbfeld`.
- Shows original size, new size, and percentage saved per file.
- Runs as a standalone native GUI binary with no console window.

## Download

Grab the prebuilt binary for your OS from the Releases page:

- Windows: `demultimedia-windows-x86_64.exe` or `.zip`
- Linux: `demultimedia-linux-x86_64`, `.deb`, `.rpm`, or `.tar.gz`
- macOS: `demultimedia-macos-aarch64.tar.gz` or `demultimedia-macos-x86_64.tar.gz`

## Build from source

```bash
cargo build --release
```

The binary lands at `target/release/demultimedia`. On Linux, `demultimedia.desktop` sets `Terminal=false` if you want to launch it from your app menu.

## Tests

```bash
cargo test
```
