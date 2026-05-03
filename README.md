# peek

A CLI tool that searches for code definitions (functions, classes, structs, etc.) by name across multiple programming languages, using tree-sitter AST parsing.

## Features

- **16 languages** — Python, Go, Rust, JavaScript, TypeScript, Java, C#, PHP, C, C++, Kotlin, Swift, Ruby, Dart, Bash, Lua
- **Fast** — Parallel AST extraction with rayon, persistent mmap cache invalidated by mtime + file size
- **Scope-aware** — Search within scopes like `MyClass::method`, `Module.function`
- **ripgrep-aligned CLI** — `-i`, `-S`, `-g`, `-l`, `-c`, `-e`, `-w`, `--hidden`, `--no-ignore`, `--json` work like ripgrep
- **Claude Code plugin** — Install via plugin, use peek directly inside Claude Code sessions

## Installation

### Claude Code Plugin (recommended)

Install the [Claude Code](https://docs.anthropic.com/en/docs/claude-code) plugin, then run the ensure command to install or update the `peek` binary:

```bash
claude plugins install peek-code@vibewire
```

After installing the plugin, run `/peek-code:ensure` in a Claude Code session. This detects your platform and installs `peek` via the best available method (Homebrew, cargo-binstall, or cargo install).

To update `peek` later, run `/peek-code:ensure` again.

### Standalone

**Pre-built binaries** (GitHub Releases):

Download from [Releases](https://github.com/owniai/peek/releases) for your platform.

**Homebrew** (macOS/Linux):

```bash
brew install owniai/tap/peek-code
```

**cargo-binstall**:

```bash
cargo binstall peek-code -y
```

**From source**:

```bash
cargo install peek-code --locked
```

> **Note:** The crate name on crates.io is `peek-code`; the installed binary is `peek`.

## Usage

Search for a definition by name:

```bash
peek my_function
```

Search in specific paths:

```bash
peek MyClass src/
```

Filter by definition kind:

```bash
peek -k function,class MyFunc
```

Scope-aware search (`.` and `::` are regex metacharacters — escape for exact match):

```bash
peek 'MyClass::method'     # Rust, C++ (:: scope separator)
peek 'MyClass\.method'     # Python, Go, JS, etc. (. scope separator)
peek '.*::run'             # Regex: any method named "run"
```

Multi-pattern search with OR semantics:

```bash
peek -e 'get_.*' -e 'set_.*' src/
```

Case control:

```bash
peek -i myfunction         # Case-insensitive
peek -S MyFunction         # Smart case: case-sensitive when pattern has uppercase
peek -w my_function        # Word boundary match only
```

JSON output (ripgrep-compatible envelope format):

```bash
peek --json MyClass
```

Glob filtering:

```bash
peek -g '*.rs' -g '!*test*' my_func
```

List all definitions in a file:

```bash
peek ... src/lib.rs
```

## Command Line Options

```
Usage: peek [OPTIONS] [PATTERN] [FILES]...

Arguments:
  [PATTERN]   Definition name to search for (optional when -e is used)
  [FILES]...  Files or directories to search (default: current directory)

Options:
  -k, --kind <KIND>            Definition types (comma-separated: function,class,struct,...)
  -e, --regexp <REGEXP>        Search patterns, repeatable with OR semantics
  -w, --word-regexp            Match whole words only
  -i, --ignore-case            Case-insensitive matching
  -S, --smart-case             Case-insensitive unless pattern has uppercase
  -g, --glob <GLOB>            File glob filters (repeatable, ! negates)
      --hidden                 Search hidden files and directories
      --no-ignore              Don't respect .gitignore and .ignore
  -d, --max-depth <DEPTH>      Max directory traversal depth
      --json                   JSON output (ripgrep envelope format)
  -l, --files-with-matches     Only show file paths
  -c, --count                  Show match count per file
  -H, --with-filename          Always show file path prefix
  -I, --no-filename            Never show file path prefix
  -M, --no-messages            Suppress error messages
      --no-signature           Suppress signature output
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
