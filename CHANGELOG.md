# Changelog

All notable changes to winget-tui are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versions match the `v<major>.<minor>.<patch>` tags in git.

---

## [0.10.0] – 2026-04-26

### Fixed
- Package list viewport row math: visible-row count is now derived from the
  rendered table area, so the selection always scrolls into view when you move
  past the bottom row.

---

## [0.9.1] – 2026-04-24

### Fixed
- Pinned packages now appear in the Upgrades view.  winget's second
  pin-summary table is parsed so blocking-pinned packages still show up and
  respond to the pinned filter.
- Detail pane now shows the installed version instead of the manifest (latest)
  version returned by `winget show`.
- `ensure_selection_visible` is called after cursor restore on refresh and
  after scrollbar click/drag so the viewport always follows the selection.
- PgUp/PgDn docs updated from "20 items" to "one page" to match the actual
  viewport-based behaviour.

---

## [0.9.0] – 2026-04-23

### Changed
- Internal: `truncate()` now uses `Cow<str>` to avoid allocating when the
  string is already short enough.

### Other
- Expanded unit-test coverage across the core state machine.

---

## [0.8.2] – 2026-04-23

### Fixed
- Pinned-package summary table no longer leaks into the installed-package
  list.
- Added missing help-overlay entries for `P` (cycle pin filter) and `c`
  (open changelog).
- Bumped CI action versions.

---

## [0.8.1] – 2026-04-22

### Added
- **Real-time local filter** — press `/` or `s` in Installed or Upgrades to
  filter the visible list instantly as you type.
- **CSV export** — press `e` to save the currently visible package list to a
  CSV file.

### Fixed
- Pin compatibility and status-feedback messages corrected.

### Performance
- Optimised version sorting to use numeric component comparison.
- Reduced per-frame UI render allocations.

---

## [0.8.0] – 2026-04-20

### Added
- **Pin management** — `p` pins or unpins the selected installed package
  (blocks it from being auto-upgraded).  `P` cycles the pin filter:
  All → Pinned only → Hide pinned.

### Fixed
- Regression in Installed view that hid some pinned packages.
- Improved package-detail fallbacks when manifest metadata is sparse.

---

## [0.7.0] – 2026-04-19

### Fixed
- Version sort now uses proper numeric-component comparison so `10.0` sorts
  after `9.0`.
- Corrected mouse-wheel scroll direction and package-row click hit-testing.
- `PageUp` / `PageDown` now jump by the actual viewport height instead of a
  hard-coded 20 rows.
- `is_truncated()` now detects both the Unicode `…` and ASCII `...` forms of
  truncated package IDs.
- Status message shown when homepage or changelog URL is unavailable.
- Parser robustness improvements for unusual winget output layouts.

### Performance
- `apply_filter` sorts Name and ID columns with `sort_by_cached_key` to avoid
  repeated string allocations.

---

## [0.6.0] – 2026-04-16

### Added
- **Sortable columns** — `S` cycles: Name↑ → Name↓ → ID↑ → ID↓ →
  Version↑ → Version↓ → off.
- **Version-specific install** — `I` prompts for a version string before
  installing.
- **Independently scrollable detail pane** — `Tab` / `Shift+Tab` moves
  keyboard focus between the package list and the detail panel; the detail
  panel has its own scroll position.
- **Startup configuration** — optional `config.toml` sets `default_view` and
  `default_source` at launch.
- **Release notes** — `c` opens the package's changelog URL in your browser.
- **Winget-not-found detection** — a clear error is shown before entering the
  TUI if `winget` is absent or not on `PATH`.

### Fixed
- Selection preserved across view refreshes: switching tabs no longer resets
  the highlighted package.

### Performance
- `sanitize_text` has a fast path that skips allocation for clean input.
- Avoid heap allocations in `find_column_ci` and `normalize_show_key`.
- `parse_show_output` uses a `Peekable` iterator, removing a temporary `Vec`.

### Other
- Release binary now built with full LTO, single codegen unit, and symbol
  stripping for a smaller, faster executable.

---

## [0.5.0] – 2026-04-15

### Added
- **UX overhaul** — winget-inspired colour theme, improved layout, and
  accessibility improvements (contributed by [@niels9001](https://github.com/niels9001)).

---

## [0.3.0] – 2026-03-31

### Added
- `SourceFilter::as_arg()` helper to centralise source-argument construction.

### Fixed
- Unicode display widths used in `truncate()` for correct CJK rendering.
- Detail-loading guard prevents loading details for truncated package IDs.
- Install/uninstall/upgrade blocked for truncated IDs.

---

## [0.2.3] – 2026-03-26

### Fixed
- `PackageDetail::merge_over` helper eliminates verbose field-by-field merges.
- Available version shown in the detail panel for upgradeable packages.
- Column index pre-computed once per table scan; `Vec<char>` removed from
  hot path.
- `detail_loading` flag reset when switching tabs.

---

## [0.2.2] – 2026-03-18

### Security
- Command injection in `open_url` fixed — only `http://` and `https://` URLs
  are forwarded to the shell.
- Terminal control-character sanitisation added to all winget output.
- CI script injection risks eliminated; third-party actions pinned to SHAs.

---

## [0.2.1] – 2026-03-17

### Fixed
- Key-hint clipping in narrow terminals.
- Detail loading for packages whose names contain non-ASCII characters.

---

## [0.2.0] – 2026-03-09

### Added
- Multi-select batch upgrade in the Upgrades view: `Space` toggles a package,
  `a` selects/deselects all, `U` upgrades all selected packages.

---

## [0.1.3] – 2026-02-13

### Fixed
- Mouse tab-click uses calculated positions instead of hard-coded offsets.
- Detail-panel race condition when scrolling quickly.
- Search cursor handling for non-ASCII input.
- Source filter applied correctly on the Upgrades tab.
- Parser hardened: filter instead of take_while, ID validation, truncated-ID
  handling.

---

## [0.1.2] – 2026-02-09

### Fixed
- Mouse click selects the correct package after the list has scrolled.
- Upgrade label, keyboard nav, and 7-Zip parsing edge cases.

---

## [0.1.1] – 2026-02-09

### Fixed
- Non-English locale support in winget output parsing.

---

## [0.1.0] – 2026-02-08

### Added
- Initial release: search, install, uninstall, upgrade, mouse support, and
  animated UI.
