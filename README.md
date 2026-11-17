# forma

`forma` is an open-source **photo + video editing toolkit** built around creative freedom:

- **CLI** for automation and scripting
- **Native GUI** (feature-gated)
- **MCP server** for tool integrations
- **ffmpeg-powered video pipeline**
- **Photoshop (`.psd`) input support**
- **RAW decode fallback**
- **Local AI + Copilot-style smart suggestions**

It is designed as a serious foundation toward Lightroom/Premiere/Photoshop-class workflows while remaining lightweight and hackable.

## Open source / licensing

- License: **MIT** (`LICENSE`)
- `Cargo.lock` is ignored (dependency intent lives in `Cargo.toml`)

## Build on Windows, macOS, or Linux

Install the Rust toolchain (Cargo) and Python 3, then run the same host-detecting build script on any supported system:

```bash
python scripts/build.py
```

PowerShell on Windows:

```bash
py -3 scripts/build.py
```

The script detects Windows, macOS, or Linux and builds a release binary with the native GUI enabled. The binary is written to `target/release/` (`forma.exe` on Windows). Use `--no-gui` to build the CLI only, or `--debug` for a faster debug build:

```bash
python scripts/build.py --no-gui
python scripts/build.py --debug
```

GitHub Actions also builds and tests the project on Windows, macOS, and Linux.

To build directly with Cargo instead, use `cargo build --release --features gui`.

## Project structure

The app is now modular (not a single giant `main.rs`):

- `src/main.rs` – bootstrap
- `src/args.rs` – CLI schema
- `src/app.rs` – command dispatcher
- `src/editing/*` – image ops, parsing, AI presets, pipeline, PSD/RAW loaders
- `src/integrations/*` – GUI, MCP server, ffmpeg video integration

## Commands

```bash
# Photo pipeline
cargo run -- cli --input in.jpg --output out.jpg --op autolevels --op contrast:10

# Photo pipeline with AI and Copilot-style suggestion
cargo run -- cli --input in.psd --output out.png --ai copilot-smart --copilot-suggest

# Advanced video pipeline
cargo run -- video \
  --input in.mp4 \
  --output out.mp4 \
  --trim-start 00:00:02 \
  --trim-end 00:00:20 \
  --resize 1920:1080 \
  --fps 30 \
  --color-grade cinematic \
  --overlay-image watermark.png \
  --overlay-pos 24:24 \
  --codec libx264 \
  --crf 19 \
  --preset slow

# Show available image operations
cargo run -- ops

# PSD metadata probe
cargo run -- psd-info --input shot.psd

# Start MCP server
cargo run -- mcp

# Launch GUI (requires --features gui)
cargo run --features gui -- gui

# ffmpeg helpers
cargo run -- ffmpeg-extract-frame --input-video in.mp4 --output-image frame.png --timestamp 00:00:03
cargo run -- ffmpeg-filter --input-video in.mp4 --output-video out.mp4 --vf "eq=contrast=1.15:saturation=1.2"
```

## Photo editing operations

- `grayscale`
- `invert`
- `autolevels`
- `brighten:<int>`
- `contrast:<float>`
- `blur:<float>`
- `huerotate:<int>`
- `saturate:<float>`
- `gamma:<float>`
- `unsharpen:<sigma>,<threshold>`
- `rotate90`, `rotate180`, `rotate270`
- `flip-horizontal`, `flip-vertical`
- `crop:<x>,<y>,<w>,<h>`
- `resize:<w>,<h>[,<filter>]`

Resize filters: `nearest`, `triangle`, `catmullrom`, `gaussian`, `lanczos3`

## AI modes

- `enhance`
- `denoise`
- `dramatic`
- `copilot-smart`

These run locally as deterministic operation stacks.

## PSD and RAW handling

- `.psd` inputs are decoded through ffmpeg integration.
- Standard image decode failures automatically fall back to ffmpeg, enabling many RAW camera formats.
- PSD export is not implemented yet (export to PNG/JPEG/TIFF/WebP/AVIF).

## MCP tools

- `apply_pipeline`
- `recommend_pipeline`
- `video_edit`
