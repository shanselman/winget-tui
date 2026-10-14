# Contributing to winget-tui

Thanks for your interest in improving `winget-tui`! This project values
**stability**, **correctness**, and **minimal dependencies**, so contributions
are easiest to merge when they are small, focused, and well-tested.

## Before you start

- Read [`AGENTS.md`](AGENTS.md) for an overview of the architecture, key state
  and data flow, and parsing/UI conventions used throughout the codebase.
- Read the [README](README.md) for features, keybindings, and configuration.
- For anything beyond a small fix, consider opening an issue first to discuss
  the approach — this avoids wasted effort on changes that don't fit the
  project's direction.

## Development setup

You'll need the Rust toolchain (`cargo`, `rustc`) and, on Windows, Visual
Studio Build Tools with the C++ workload and a Windows SDK. See the
[README's "Run from source for development" section](README.md#run-from-source-for-development)
for full setup steps.

```powershell
git clone https://github.com/shanselman/winget-tui.git
cd winget-tui
cargo run
```

## Before opening a pull request

Run the same checks CI runs:

```powershell
cargo check --all-targets
cargo fmt -- --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Notes:

- `tests\parse_test.rs` is a Windows-only integration test; Linux environments
  will still build and run the rest of the suite.
- A minimum supported Rust version (currently checked via the `msrv` CI job)
  is enforced — avoid relying on very recent language or standard library
  features.

## Contribution guidelines

- **Keep changes small and behavior-safe.** Prefer several small, focused
  pull requests over one large one.
- **Add tests** when adjusting parser logic (`cli_backend.rs`), selection
  behavior, or input handling (`handler.rs`).
- **Avoid new dependencies** unless there is a clear, discussed payoff — open
  an issue first if you think one is needed.
- **Match existing style.** Use the semantic color/style helpers in
  `theme.rs` instead of hard-coded colors, and the Unicode display-width
  helpers in `ui.rs` when truncating or aligning text.
- **Stay locale-aware.** `cli_backend.rs` parses `winget` CLI output and must
  not assume English-only text.
- **Update documentation** (README, `AGENTS.md`, or this file) when your
  change affects setup, keybindings, configuration, or architecture.

## Reporting bugs and requesting features

Please open an issue with:

- For bugs: steps to reproduce, expected vs. actual behavior, your
  `winget-tui` version, Windows version, and `winget --version` output.
- For features: the problem you're trying to solve and, if you have one, a
  proposed approach.

## Code of conduct

Be respectful and constructive. This is a small, community-maintained
project — patience and clear communication go a long way.
