# demultimedia

A tiny native Rust desktop app for fixing uppercase photo extensions like `.JPG` or `.HEIC` without opening a terminal or writing a bash loop.

Cameras and phones still dump files with screamed extensions. Pick a folder (or drop it on the window), check the preview list, and click once to lowercase them.

## What it does right now

- Scans a folder (and subfolders if checked) for photo files with uppercase or mixed-case extensions.
- Leaves the filename stem alone (`IMG_001.JPG` becomes `IMG_001.jpg`).
- Skips files that would collide with an existing lowercase file in the same folder instead of overwriting anything.
- Handles case-insensitive filesystems by hopping through a temporary filename when needed.
- Runs as a native GUI binary with no console window.

Default extensions covered: `jpg`, `jpeg`, `png`, `webp`, `gif`, `bmp`, `tiff`, `tif`, `heic`, `heif`, `avif`, `raw`, `cr2`, `nef`, `arw`, `dng`.

## Build and run

You need Rust (`cargo`) installed.

```bash
cargo run --release
```

Or build the binary and launch it from your file manager:

```bash
cargo build --release
```

The binary lands at `target/release/demultimedia`. On Linux, `demultimedia.desktop` sets `Terminal=false` if you want to drop it into `~/.local/share/applications/`.

## Tests

```bash
cargo test
```
