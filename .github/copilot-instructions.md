# Copilot instructions for forma

- Prefer the structured CLI subcommands (`cli`, `gui`, `mcp`, `ffmpeg-*`) over ad-hoc argument parsing.
- Keep image operations composable and reusable via `Operation`.
- Preserve RAW fallback behavior through ffmpeg when adding new load paths.
- Any new AI behavior should remain local-first and deterministic unless explicitly configured otherwise.
- For MCP changes, keep JSON-RPC responses explicit and machine-readable.
