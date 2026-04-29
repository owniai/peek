# peek

A CLI tool that searches for code definitions (functions, classes, structs, etc.) by name across multiple programming languages, using tree-sitter AST parsing.

## Features

- **16 languages** — Python, Go, Rust, JavaScript, TypeScript, Java, C#, PHP, C, C++, Kotlin, Swift, Ruby, Dart, Bash, Lua
- **Fast** — Two-phase search: regex screening + parallel AST extraction with rayon
- **Cached** — Persistent `.peek-cache/` with mmap reads, invalidated by mtime + file size
- **Scope-aware** — Search within scopes like `MyClass::method`, `Module.function`
- **ripgrep-aligned CLI** — `-i`, `-S`, `-g`, `-l`, `-c`, `--hidden`, `--no-ignore`, `--json` work like ripgrep

## Installation

**Pre-built binaries** (GitHub Releases):

Download from [Releases](https://github.com/owniai/peek/releases) for your platform.

**From source**:

```bash
cargo install peek --locked
```

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

Scope-aware search:

```bash
peek 'MyClass::method'     # Rust, C++
peek 'MyClass.method'      # Python, Go, JS, etc.
```

Case-insensitive search:

```bash
peek -i myfunction
peek -S MyFunction         # smart case: case-sensitive when pattern has uppercase
```

JSON output (ripgrep-compatible envelope format):

```bash
peek --json MyClass
```

Glob filtering:

```bash
peek -g '*.rs' -g '!*test*' my_func
```

## Command Line Options

```
Usage: peek [OPTIONS] <PATTERN> [FILES]...

Arguments:
  <PATTERN>   Definition name (exact or fuzzy matching)
  [FILES]...  Files or directories to search (default: current directory)

Options:
  -k, --kind <KIND>            Definition types (comma-separated: function,class,struct)
  -i, --ignore-case            Ignore case
  -S, --smart-case             Smart case: ignore case unless pattern has uppercase
      --no-signature           Suppress signature output
  -l, --files-with-matches     Only show file paths
  -c, --count                  Show match count per file
      --hidden                 Search hidden files and directories
      --no-ignore              Don't respect .gitignore and .ignore
  -d, --max-depth <DEPTH>      Max directory traversal depth
      --json                   JSON output (ripgrep envelope format)
  -M, --no-messages            Suppress error messages
  -g, --glob <GLOB>            File glob filters
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE).
