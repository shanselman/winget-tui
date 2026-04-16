# AGENTS.md

This repository is a small Rust TUI for the Windows Package Manager (`winget`).
It is keyboard-first, async, and intentionally lightweight.

## Architecture

```text
src/
  main.rs         startup/shutdown and terminal lifecycle
  app.rs          App state, async coordination, selection, sorting, scrolling
  backend.rs      WingetBackend trait
  cli_backend.rs  winget CLI parsing and command execution
  config.rs       optional startup defaults from config.toml
  handler.rs      keyboard and mouse event routing
  models.rs       Package, PackageDetail, Operation, sorting enums
  theme.rs        semantic colors and UI styles
  ui.rs           all ratatui rendering
```

## Key state and flow

- `App` is the central state container.
- `AppMode` switches between `Search`, `Installed`, and `Upgrades`.
- `FocusZone` switches keyboard focus between the package list and detail panel.
- `InputMode` distinguishes normal navigation from search or version-entry prompts.
- Background work returns via `AppMessage` over a Tokio unbounded channel.

The app uses generation counters to discard stale async responses when the user
switches views or changes selection quickly. Preserve that pattern when adding
new background loads.

## Parsing and data conventions

- `cli_backend.rs` is locale-aware; avoid hard-coding English-only assumptions.
- Keep using the existing helpers for column detection, footer detection,
  normalization, and sanitization.
- Package IDs ending with `...` or `…` are considered truncated and should not
  be used with exact winget detail/install flows.

## UI conventions

- Prefer semantic helpers from `theme.rs` over hard-coded colors.
- Use Unicode display width helpers in `ui.rs` when truncating or aligning text.
- The detail pane and list pane both maintain scroll/focus state; avoid changes
  that break keyboard navigation consistency.

## Configuration

Optional config file:

- Windows: `%APPDATA%\winget-tui\config.toml`
- Fallback for non-Windows/dev: `$HOME/.config/winget-tui/config.toml`

Supported keys:

```toml
default_view = "installed"   # installed | search | upgrades
default_source = "all"       # all | winget | msstore
```

## Keybindings

- `Left` / `Right`: switch views
- `Tab` / `Shift+Tab`: toggle focus between list and details
- `/` or `s`: in Installed/Upgrades — open local filter bar (real-time, Esc to clear); in Search — focus winget search input
- `f`: cycle source filter
- `i`: install
- `I`: install a specific version
- `u`: upgrade
- `x`: uninstall
- `Space`, `a`, `U`: multi-select and batch upgrade in Upgrades view
- `o`: open homepage
- `c`: open changelog / release notes
- `S`: cycle sorting
- `?`: help

## Local filter

In Installed and Upgrades views, pressing `/` opens an in-place filter bar.
Characters are matched case-insensitively against package **name** and **ID** as
you type.  Press `Enter` to keep the filter and return to Normal mode, or `Esc`
to clear it and restore the full list.  The filter bar stays visible whenever a
filter is active.  Switching views (`Left` / `Right`) clears the filter.

## Testing and validation

Use the same commands as CI:

```powershell
cargo check --all-targets
cargo fmt -- --check
cargo test
cargo clippy -- -D warnings
```

The integration test in `tests\parse_test.rs` is Windows-only; Linux CI still
builds and runs the rest of the suite.

## Contribution guidance

- Keep changes small and behavior-safe.
- Add tests when adjusting parser logic, selection behavior, or input handling.
- Avoid new dependencies unless there is a clear payoff.
