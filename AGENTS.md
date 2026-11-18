# Agent guide

## Project layout

- `src/args.rs` defines CLI commands and options.
- `src/app.rs` dispatches commands.
- `src/editing/` contains image IO, operations, the edit pipeline, AI presets, JavaScript plugins, and optional GPU compute.
- `src/integrations/` contains the GUI, MCP server, and ffmpeg video support.
- `plugins/` contains sample JavaScript image plugins.
- `scripts/` contains host build and per-user install/uninstall scripts.

## Development

- Format and test with `cargo fmt --check` and `cargo test --features gui`.
- Test the CPU-only configuration with `cargo test --no-default-features`.
- The default Cargo feature enables `wgpu` acceleration; GPU execution is explicitly requested with `--gpu`.
- Run `python scripts/build.py` to make the host release build with the GUI.
- Keep GPU-only code behind the `gpu` feature and preserve CPU-only builds.
- Keep installers restricted to per-user paths; uninstallers must not recursively remove directories containing unmanaged files.

## Editing conventions

- Keep image operations composable through `editing::Operation` and update its parser, CLI help, tests, and README when adding operations.
- Keep JavaScript plugins self-contained and bounded by the QuickJS sandbox limits; do not expose host filesystem, network, or process APIs.
- Prefer standard-library-only Python for build and install tooling.
- Keep integrations modular; do not move CLI, GUI, MCP, image, and video logic into `main.rs`.
