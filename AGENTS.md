# AGENTS.md — Guidance for AI Coding Assistants

This file provides project-specific guidance for AI coding assistants (GitHub Copilot, Copilot Workspace, Repo Assist, etc.) and human contributors who want to understand the codebase quickly.

## Project Summary

`winget-tui` is a terminal UI for Windows Package Manager (`winget`). It is a **single Rust binary** with no runtime dependencies beyond `winget.exe`. The UI is built with [Ratatui](https://ratatui.rs/). All winget interaction is via spawned subprocesses; there is no COM/WinRT API layer.

## Architecture

```
src/
  main.rs       — Terminal setup/teardown, tokio runtime, main event loop
  app.rs        — All shared state (App struct), async message passing (mpsc), view logic
  backend.rs    — WingetBackend trait: the abstraction that decouples app logic from winget
  cli_backend.rs — Concrete implementation: spawns winget.exe, parses tabular output
  handler.rs    — Keyboard and mouse input handling; dispatches to App methods
  models.rs     — Data types: Package, PackageDetail, Source, Operation, SourceFilter, OpResult
  ui.rs         — Ratatui rendering; draw_* functions; truncate() helper
```

## Key Conventions

### Code style
- **Rust stable** — no nightly features.
- `cargo fmt` and `cargo clippy -- -D warnings` must be clean before any PR.
- Comments only where the code needs clarification; no obvious-comment noise.
- Use `&'static str` for string constants that are known at compile time.

### Async model
- Tokio multi-thread runtime.
- Background work (winget calls) is spawned via `tokio::spawn`; results are sent back over an `UnboundedSender<AppMessage>` channel.
- `App::process_messages()` drains the channel on each UI tick — no `await` in the hot path.
- **Generation counters** (`view_generation`, `detail_generation`) are used to discard stale async results; always increment before spawning a new task.

### Parsing
- `CliBackend` parses winget's tab-aligned ASCII tables by detecting the separator line (`---`) and computing display-width column positions from the header.
- `detect_columns` + `extract_field` operate in **Unicode display width** (via `unicode-width`) so CJK characters are handled correctly.
- `PackageCols` / `SourceCols` structs pre-compute column indices once per table to avoid repeated case-insensitive comparisons per row.
- `clean_output` normalises `\r\n` and inline `\r` progress overwrites **before** any parsing.
- `sanitize_text` strips ASCII control characters from package metadata to prevent ANSI injection.

### UI rendering
- All drawing is in `ui.rs`; no rendering logic in other files.
- `truncate(s, max)` clips strings to a display-width budget and appends `…`. Fast path: if `s.len() <= max` (byte length), no Unicode scan is needed.
- The `App` struct carries layout regions (`LayoutRegions`) populated by the renderer and consumed by `handler.rs` for mouse hit-testing.

### Testing
- Unit tests live in `#[cfg(test)]` modules at the bottom of each source file.
- Tests must not require `winget.exe` to be installed; use the `MockBackend` in `app.rs` for integration-level tests.
- The integration test in `tests/parse_test.rs` is gated with `#[cfg(target_os = "windows")]` and requires a live winget install.
- Run with: `cargo test`

### Dependencies
- Keep the dependency list small. Every new crate must have a clear, specific justification.
- Prefer minor/patch bumps via `cargo update`; propose major version bumps in an issue first.
- Current direct dependencies: `ratatui`, `crossterm`, `tokio`, `serde`, `serde_json`, `anyhow`, `async-trait`, `unicode-width`.

## What Repo Assist Should Know

- **All 10 previously open Repo Assist PRs were merged on 2026-03-31** (PRs #69–#78). The main branch reflects all of those changes.
- The `pause: true` flag set in memory after that batch is now lifted — new PRs are welcome.
- Fixes to `cli_backend.rs` must preserve localisation support (the column-map uses multi-language header names).
- The backend trait (`WingetBackend`) is `async_trait`; mock implementations live in `app.rs` tests.
- The build CI (`build.yml`) only runs on `windows-latest` (releases); the quality CI (`ci.yml`) runs `cargo check`, `cargo fmt`, `cargo test`, and `cargo clippy` on every PR.
