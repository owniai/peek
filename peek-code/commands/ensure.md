---
description: Ensure the peek CLI is installed and up to date. Installs if missing, updates if present.
disable-model-invocation: true
---

# peek-code:ensure

Run `peek --version` first. If peek is not found, install it. If installed, update to the latest version.

## Install methods (try in order)

| Priority | Method | Command |
|----------|--------|---------|
| 1 | Homebrew (macOS/Linux) | `brew install owniai/tap/peek` |
| 2 | cargo-binstall | `cargo binstall peek -y` |
| 3 | cargo install | `cargo install peek --locked` |
| 4 | cargo install from git | `cargo install --git https://github.com/owniai/peek --locked` |

**crates.io package:** `peek-code` (binary: `peek`). **MSRV:** 1.85.

## Update methods

| Method | Command |
|--------|---------|
| Homebrew | `brew upgrade peek` |
| cargo-binstall | `cargo binstall peek -y` |
| cargo install | `cargo install peek --locked` |

## Steps

1. Run `peek --version`.
   - **Not found** → detect platform, try install methods in order (skip unavailable tools like brew on Windows).
   - **Found** → detect which package manager was used (check `brew list peek`, `cargo install --list`), then run the corresponding update command.
2. Run `peek --version` again to confirm the final version.

## Troubleshooting

- **`peek` not found after install**: Verify `~/.cargo/bin` (or `%USERPROFILE%\.cargo\bin` on Windows) is in `$PATH`.
- **Rust not installed**: Ask the user whether to install Rust. If confirmed, help the user install it, then retry cargo methods.
