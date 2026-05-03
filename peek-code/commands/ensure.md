---
description: Ensure the peek CLI is installed and up to date. Installs if missing, updates if present.
disable-model-invocation: true
---

# peek-code:ensure

Run `peek --version` first. If peek is not found, install it. If installed, update to the latest version.

**crates.io crate:** `peek-code` (binary: `peek`). **MSRV:** 1.85.

## Install methods (try in order)

| Priority | Method | Command |
|----------|--------|---------|
| 1 | Homebrew (macOS/Linux) | `brew install owniai/tap/peek-code` |
| 2 | cargo-binstall | `cargo binstall peek-code -y` |
| 3 | cargo install | `cargo install peek-code --locked` |
| 4 | cargo install from git | `cargo install --git https://github.com/owniai/peek --locked` |

## Update methods

| Method | Command |
|--------|---------|
| Homebrew | `brew upgrade peek-code` |
| cargo-binstall | `cargo binstall peek-code -y` |
| cargo install | `cargo install peek-code --locked` |

## Steps

1. Run `peek --version`.
   - **Not found** → detect platform, try install methods in order (skip unavailable tools like brew on Windows).
   - **Found** → detect which package manager was used (check `brew list peek-code`, `cargo install --list`), then run the corresponding update command. The cargo crate name is `peek-code`.
2. Run `peek --version` again to confirm the final version.

## Troubleshooting

- **`peek` not found after install**: Verify `~/.cargo/bin` (or `%USERPROFILE%\.cargo\bin` on Windows) is in `$PATH`.
- **Rust not installed**: Ask the user whether to install Rust. If confirmed, help the user install it, then retry cargo methods.
- **peek-code skill 加载失败 (Unknown skill)**: 通过 `claude plugins install peek-code@vibewire` 安装插件后，当前会话无法立即加载 `peek-code:peek` skill，需重启会话后才能使用。
