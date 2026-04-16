# winget-tui

**New design and visual research by [@niels9001](https://github.com/niels9001).**

A terminal user interface for [Windows Package Manager (winget)](https://github.com/microsoft/winget-cli). Search, install, uninstall, and upgrade Windows packages without leaving your terminal.

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)

![winget-tui screenshot](img/wingettui.png)

## Features

- **Search & Discover** — Find packages across all winget sources
- **Installed Packages** — View everything installed on your system
- **Upgrade Management** — See available updates at a glance; batch-upgrade with `Space`/`a`/`U`
- **Source Filtering** — Filter by source (winget, msstore, or all)
- **Sortable Columns** — Sort by Name, ID, or Version (ascending or descending) with `S`
- **Package Details** — Scrollable panel with publisher, description, license, homepage, and changelog link
- **Version Selection** — Install a specific version with `I`
- **Configurable** — Optional `config.toml` for default view and source
- **Keyboard-Driven** — Vim-style navigation, no mouse needed
- **Non-Blocking** — Install/uninstall/upgrade run in the background
- **Single Binary** — No runtime dependencies beyond winget itself

## Prerequisites

- Windows 10/11
- [winget](https://github.com/microsoft/winget-cli) 1.4+ installed
- A terminal with Unicode support (Windows Terminal recommended)

## Installation

### From source

```sh
git clone https://github.com/shanselman/winget-tui.git
cd winget-tui
cargo build --release
```

The binary will be at `target\release\winget-tui.exe`.

## Usage

```sh
winget-tui
```

### Keybindings

| Key | Action |
|-----|--------|
| `↑` / `k` | Move up |
| `↓` / `j` | Move down |
| `PgUp` / `PgDn` | Jump 20 items |
| `Home` / `End` | Jump to first / last |
| `Tab` / `Shift+Tab` | Toggle focus between package list and detail panel |
| `←` / `→` | Cycle views backwards / forwards |
| `/` or `s` | Focus search input |
| `Enter` | Submit search / show details |
| `f` | Cycle source filter (All → winget → msstore) |
| `r` | Refresh current view |
| `i` | Install selected package (latest version) |
| `I` | Install selected package at a specific version |
| `u` | Upgrade selected package |
| `U` | Batch-upgrade all selected packages (Upgrades view) |
| `x` | Uninstall selected package |
| `Space` | Toggle selection for batch upgrade (Upgrades view) |
| `a` | Select / deselect all for batch upgrade (Upgrades view) |
| `o` | Open package homepage in browser |
| `c` | Open release notes / changelog URL in browser |
| `S` | Cycle sort (Name↑ → Name↓ → ID↑ → ID↓ → Version↑ → Version↓ → off) |
| `?` | Toggle help overlay |
| `q` / `Esc` | Quit / close dialog |
| `Ctrl+C` | Quit |

### Mouse Support

- **Click** on tabs to switch views (Search / Installed / Upgrades)
- **Click** on the filter bar to cycle source filters
- **Click** on the search bar to start typing a search
- **Click** on a package row to select it and load details
- **Scroll wheel** over the package list to navigate up/down
- **Right-click** a package to select and load its details
- **Click & drag** the scrollbar to scrub through the list

### Views

- **Installed** (default) — Lists all packages installed on your system
- **Search** — Search the winget repository for new packages
- **Upgrades** — Shows packages with available updates

## Configuration

Create an optional config file at `%APPDATA%\winget-tui\config.toml` to set startup defaults:

```toml
# Default view when winget-tui starts: "installed" | "search" | "upgrades"
default_view = "installed"

# Default source filter: "all" | "winget" | "msstore"
default_source = "all"
```

All keys are optional. A missing or malformed file is silently ignored.

## Architecture

```
winget-tui
├── src/
│   ├── main.rs          # Entry point, terminal setup/teardown
│   ├── app.rs           # App state, message passing, async coordination
│   ├── backend.rs       # WingetBackend trait (abstraction layer)
│   ├── cli_backend.rs   # CLI implementation (shells out to winget.exe)
│   ├── config.rs        # Optional config file loader (no extra dependencies)
│   ├── handler.rs       # Keyboard and mouse input handling
│   ├── models.rs        # Data types (Package, Source, Operation, etc.)
│   └── ui.rs            # Ratatui rendering (all UI components)
└── Cargo.toml
```

The backend is behind a trait (`WingetBackend`) to allow future implementations (e.g., COM API for better performance).

## License

MIT
