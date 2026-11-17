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

## Install / uninstall

The Python scripts install a per-user copy, create a native launcher (including desktop/start-menu entries where available), and do not require administrator privileges. If the release binary has not been built, the installer runs the host build script automatically.

```bash
python scripts/install.py
python scripts/uninstall.py
```

On Windows PowerShell, use:

```powershell
py -3 scripts/install.py
py -3 scripts/uninstall.py
```

The install locations are per-user: Linux uses `~/.local` and adds an SVG app icon, macOS installs `~/Applications/Forma.app` with a Desktop link when available, and Windows installs under `%LOCALAPPDATA%\Programs\Forma` with Start Menu and Desktop shortcuts.

The installer supports `--binary PATH` to install an existing executable. It refuses to overwrite conflicting files by default; `--force` permits replacing files at forma's dedicated install paths. The uninstaller verifies the installed executable and removes only managed files, leaving modified files and non-empty directories in place.

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
```

The desktop editor opens images into a preview workspace, suggests an edited PNG output name, supports quick exposure/contrast/saturation/vignette controls, operation history with undo/redo, and dark/light themes. Photoshop-style shortcuts use Ctrl on Windows/Linux and Cmd on macOS: `Ctrl/Cmd+O` focuses the input path, `Ctrl/Cmd+S` exports, `Ctrl/Cmd+Z` undoes, and `Ctrl/Cmd+Shift+Z` (or `Ctrl/Cmd+Y`) redoes. Large images are downscaled for responsive previews; export still applies operations at full resolution.

```bash
# ffmpeg helpers
cargo run -- ffmpeg-extract-frame --input-video in.mp4 --output-image frame.png --timestamp 00:00:03
cargo run -- ffmpeg-filter --input-video in.mp4 --output-video out.mp4 --vf "eq=contrast=1.15:saturation=1.2"
```

## Photo editing operations

- `grayscale`
- `invert`
- `autolevels`
- `exposure:<EV>` (for example `exposure:1.0`)
- `vignette:<0..1>` (for example `vignette:0.35`)
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

## GPU acceleration

The default build includes a `wgpu` compute path. Add `--gpu` to a photo pipeline to run supported per-pixel operations on the best available GPU adapter; other operations in the same pipeline continue on the CPU. Supported operations include grayscale, invert, brighten, contrast, saturation, gamma, exposure, and vignette. If no compatible GPU is available, the command reports the error rather than silently claiming acceleration.

```bash
cargo run -- cli --input in.jpg --output out.jpg --gpu \
  --op exposure:0.7 --op saturate:1.15 --op vignette:0.3
```

Build without the GPU dependencies with `cargo build --no-default-features`; in that build, use of `--gpu` returns an explanatory error.

## PSD and RAW handling

- `.psd` inputs are decoded through ffmpeg integration.
- Standard image decode failures automatically fall back to ffmpeg, enabling many RAW camera formats.
- PSD export is not implemented yet (export to PNG/JPEG/TIFF/WebP/AVIF).

## MCP tools

- `apply_pipeline`
- `recommend_pipeline`
- `video_edit`
