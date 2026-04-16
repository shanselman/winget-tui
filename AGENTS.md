# AGENTS.md — Project Orientation for AI Assistants and Contributors

This file gives AI coding assistants (GitHub Copilot, Copilot Workspace, Repo Assist) and
human contributors a quick mental model of `winget-tui` before they start writing code.

---

## Project Summary

`winget-tui` is a terminal UI (TUI) for the Windows Package Manager (`winget`). It lets
users search, install, uninstall, and upgrade Windows packages without leaving the terminal.

**Target platform**: Windows 10/11 with `winget` 1.4+ on PATH.  
**Language**: Rust (edition 2021, `async`/`await` via Tokio).  
**TUI framework**: [`ratatui`](https://github.com/ratatui-org/ratatui) + `crossterm`.  
**Binary size goal**: single, small, dependency-light binary (`profile.release` uses LTO +
single CGU + symbol stripping).

---

## Architecture

```
winget-tui
├── src/
│   ├── main.rs        Entry point — terminal setup/teardown, panic hook, event loop
│   ├── app.rs         App state, async message passing, sort/filter, focus management
│   ├── backend.rs     WingetBackend trait (abstraction for testability)
│   ├── cli_backend.rs CLI implementation — spawns winget.exe, parses stdout
│   ├── config.rs      Optional config file loader (config.toml, no extra deps)
│   ├── handler.rs     Keyboard + mouse event handling
│   ├── models.rs      Shared data types (Package, PackageDetail, Operation, …)
│   ├── theme.rs       Colour constants and styles
│   └── ui.rs          Ratatui rendering (all draw calls live here)
└── tests/
    └── parse_test.rs  Integration tests (Windows-only, gated with #[cfg(target_os = "windows")])
```

The `WingetBackend` trait (in `backend.rs`) abstracts all `winget` interaction. The only
concrete implementation is `CliBackend` (in `cli_backend.rs`), which shells out to
`winget.exe`. Unit tests use mock backends built with the same trait.

---

## Key Types and State

### `App` (app.rs)

Central state struct. Notable fields:

| Field | Type | Purpose |
|-------|------|---------|
| `mode` | `AppMode` | Current view: `Search`, `Installed`, or `Upgrades` |
| `input_mode` | `InputMode` | `Normal`, `Search`, or `VersionInput` |
| `focus` | `FocusZone` | `PackageList` or `DetailPanel` |
| `sort_field` / `sort_dir` | `SortField` / `SortDir` | Active sort column and direction |
| `source_filter` | `SourceFilter` | `All`, `Winget`, or `MsStore` |
| `selected_packages` | `HashSet<usize>` | Indices chosen for batch upgrade |
| `detail_scroll` | `usize` | Scroll offset of the detail panel |
| `view_generation` / `detail_generation` | `u64` | Generation counters to discard stale responses |

### `AppMode` / `FocusZone` / `InputMode`

Simple enums. `AppMode` drives which winget subcommand is run and which list is displayed.
`FocusZone` determines whether navigation keys move the package cursor or scroll the detail
pane. `InputMode` determines whether keyboard events go to the text input or the normal
handler.

### `Package` / `PackageDetail` (models.rs)

`Package` is the list row. `PackageDetail` is the expanded info from `winget show`.
`PackageDetail::merge_over` combines a stub (populated from the list) with a freshly loaded
detail so the panel shows instant data while the async call completes.

### `Operation` / `OpResult` (models.rs)

Enum of things the user can do (`Install`, `Uninstall`, `Upgrade`, `BatchUpgrade`).
`BatchUpgrade` holds a `Vec<String>` of IDs. `OpResult` carries success/failure + message.

---

## Async Model

All winget calls are spawned as Tokio tasks via `App::spawn_task`. They communicate back
through an `mpsc` channel (`AppMessage`). The event loop calls `app.process_messages()` each
tick to drain the channel.

**Generation counters** prevent stale data from a previous request overwriting a newer one:
- `view_generation` is incremented on every view switch or refresh.
- `detail_generation` is incremented whenever the selected package changes or the view
  switches.

A background task that finishes after the generation has advanced simply drops its result
(the `App::apply_packages_loaded` / `App::apply_detail_loaded` methods check the generation).

**Hot-path constraint**: `ui::draw` is called every frame (60 fps target). Do not allocate
or clone unnecessarily inside draw code — `truncate`, `sanitize_text`, and the column helpers
are performance-sensitive.

---

## Parsing Conventions (cli_backend.rs)

`winget list` and `winget upgrade` output fixed-width columns separated by a dashed separator
line. The parser:

1. Detects column boundaries from the `---- ---- ----` separator line
   (`find_table_separator`, `detect_columns`).
2. Slices each row at the column byte offsets (display-width columns — not char-width).
3. Strips trailing ANSI escape sequences and `…` truncation markers via `sanitize_text`.

`winget show` output is key: value pairs — parsed by `parse_show_output` using
`normalize_show_key` to canonicalise header names across locales.

**Localisation note**: Header names vary by system locale (`Name`, `Nom`, `Nome`, …). The
parser uses case-insensitive ASCII comparison and normalises known aliases. Test coverage
exists for EN/FR/ES/IT/PT locales.

---

## UI Rendering (ui.rs)

All rendering is in `ui::draw`. Key conventions:

- `truncate(s, width)` clips a string to a display-width budget. Always use it for
  user-supplied strings — package names/IDs can be arbitrary length.
- `sanitize_text(s)` strips ANSI control sequences before display.
- `LayoutRegions` stores `Rect`s for mouse hit-testing; updated every frame in `ui::draw`.
- The detail panel is independently scrollable — `app.detail_scroll` is the line offset.

---

## Config File (config.rs)

An optional `config.toml` is read at startup. No TOML library is used — the parser handles
only bare `key = "value"` lines.

**Windows path**: `%APPDATA%\winget-tui\config.toml`  
**Fallback path** (non-Windows / CI): `$HOME/.config/winget-tui/config.toml`

Supported keys:

```toml
default_view   = "installed"   # "installed" | "search" | "upgrades"
default_source = "all"         # "all" | "winget" | "msstore"
```

Unknown keys and malformed lines are silently ignored.

---

## Complete Keybinding Reference

| Mode | Key | Action |
|------|-----|--------|
| Normal | `q` / `Esc` | Quit / close dialog |
| Normal | `Ctrl+C` | Quit |
| Normal | `?` | Toggle help overlay |
| Normal | `Tab` / `Shift+Tab` | Toggle focus between package list and detail panel |
| Normal | `←` | Cycle views backwards (Upgrades → Installed → Search) |
| Normal | `→` | Cycle views forwards (Search → Installed → Upgrades) |
| Normal | `↑` / `k` | Move selection up (or scroll detail panel up when focused) |
| Normal | `↓` / `j` | Move selection down (or scroll detail panel down when focused) |
| Normal | `PgUp` | Jump 20 rows up (or scroll detail by page) |
| Normal | `PgDn` | Jump 20 rows down (or scroll detail by page) |
| Normal | `Home` | Jump to first item (or scroll detail to top) |
| Normal | `End` | Jump to last item (or scroll detail to bottom) |
| Normal | `Enter` | Load detail for selected package |
| Normal | `/` or `s` | Switch to Search view and enter search input mode |
| Normal | `f` | Cycle source filter (All → winget → msstore → All) |
| Normal | `r` | Refresh current view |
| Normal | `i` | Install selected package (latest version) |
| Normal | `I` | Install selected package — prompts for a specific version |
| Normal | `u` | Upgrade selected package |
| Normal | `U` | Batch-upgrade all selected packages (Upgrades view only) |
| Normal | `x` | Uninstall selected package |
| Normal | `Space` | Toggle selection of current package for batch upgrade (Upgrades view) |
| Normal | `a` | Select / deselect all packages for batch upgrade (Upgrades view) |
| Normal | `o` | Open package homepage in default browser |
| Normal | `c` | Open release notes / changelog URL in default browser |
| Normal | `S` | Cycle sort: Name↑ → Name↓ → ID↑ → ID↓ → Version↑ → Version↓ → off |
| Search input | `Enter` | Submit search query |
| Search input | `Backspace` | Delete last character |
| Search input | `Esc` | Cancel search, return to normal mode |
| Version input | `Enter` | Confirm install with entered version |
| Version input | `Backspace` | Delete last character |
| Version input | `Esc` | Cancel version input |
| Confirm dialog | `y` / `Y` | Confirm operation |
| Confirm dialog | `n` / `N` / `Esc` | Cancel operation |
| Help overlay | `?` / `Esc` | Close help overlay |

---

## Testing Conventions

- **Unit tests** live in `#[cfg(test)]` modules at the bottom of each source file.
- **Integration tests** are in `tests/parse_test.rs` and are gated with
  `#[cfg(target_os = "windows")]` — they require a real `winget` binary.
- **Mock backend**: unit tests that touch `App` create a `MockBackend` struct implementing
  `WingetBackend`. See `src/handler.rs` and `src/app.rs` for examples.
- **Run tests**: `cargo test` (Linux/CI skips platform-gated integration tests automatically).
- **CI gates**: `cargo check --all-targets`, `cargo fmt -- --check`, `cargo test`,
  `cargo clippy -- -D warnings`. All must pass before merging.

---

## Dependency Policy

**No new dependencies without discussion.** The project values a small, auditable dependency
tree. Before adding a crate, open an issue describing the use case and why the standard
library or an existing dependency cannot cover it. Accepted additions:

- Must have a clear, scoped purpose.
- Must be widely used and actively maintained.
- Must not pull in large transitive trees.

---

## Contribution Notes

- Match existing Rust formatting: `cargo fmt` (rustfmt defaults).
- `cargo clippy -- -D warnings` must be clean.
- Keep PRs small and focused — one concern per PR.
- Document public items that aren't self-evident.
- New keybindings must be added to the help overlay in `ui.rs`, the keybindings table in
  `README.md`, and the reference table in this file.
