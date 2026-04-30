---
name: peek-code
description: Use when exploring codebase structure, understanding what definitions exist in files, or searching for where a symbol is defined. Reading once is sufficient unless unsure how to use the tool.
---

# peek

`peek` searches for code definitions by name using tree-sitter AST parsing. Unlike grep, it understands code structure — returns only definition locations with no false positives from comments, strings, or references. Supports scope-aware search (e.g. `MyClass::method`) and ripgrep-aligned CLI flags.

```
peek [OPTIONS] <PATTERN> [FILES]...
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<PATTERN>` | See Pattern section below |
| `[FILES]...` | Files or directories to search in. Default: current directory |

### Pattern

Matches by exact full name (not substring) by default. Regex wildcards are supported for fuzzy matching. The reserved pattern `...` lists all definitions without name filtering.

A scope prefix narrows the search to definitions under a specific parent scope, using each language's native separator (e.g. `::` for Rust, `.` for Python). Both the scope prefix and the name part support regex independently, split by the last separator. Since `.` and `\` are regex special characters, escape them as `\.` and `\\` when matching literal names.

## Options

| Option | Description |
|--------|-------------|
| `-k, --kind <KIND>` | Filter by definition kind. Comma-separated. Kinds: `function`, `class`, `struct`, `enum`, `type`, `trait`, `interface`, `const`, `record`, `delegate`, `event`, `object`, `protocol`, `actor`, `extension`, `mixin`, `module`, `macro` |
| `-i, --ignore-case` | Case-insensitive matching |
| `-S, --smart-case` | Case-insensitive unless pattern contains uppercase (default: case-sensitive) |
| `-g, --glob <GLOB>` | File glob filter. Repeatable. `!` prefix negates. Later overrides earlier. Example: `-g '*.rs' -g '!*test*'` |
| `-l, --files-with-matches` | Print only file paths |
| `-c, --count` | Print match count per file |
| `--json` | JSON output (ripgrep envelope format) |
| `-H, --with-filename` | Always print file path prefix (mutually exclusive with `-I`) |
| `-I, --no-filename` | Never print file path prefix (mutually exclusive with `-H`) |
| `--no-signature` | Suppress signature line |
| `--hidden` | Search hidden files and directories |
| `--no-ignore` | Don't respect .gitignore and .ignore |
| `-d, --max-depth <N>` | Max directory traversal depth |
| `-M, --no-messages` | Suppress non-fatal error messages |

## Output Formats

**Default** — `<path>:<start>-<end> [<kind>/<scope>] <signature>`
- Filename `<path>` is suppressed when searching a single file. Use `-H` to always show or `-I` to always hide.
- **`--no-signature`** — omits the signature part.
- `scope` is the fully qualified name including the definition itself (e.g. `App::run`). Top-level scope equals own name.

**`-l`** — file paths only, one per line. **`-c`** — `<path>:<count>` per file.

**`--json`** — NDJSON ripgrep envelope format (`begin`/`match`/`end`/`summary`). Orthogonal to `-l`/`-c` (they control whether `match` messages are emitted).

```json
{"type":"begin","data":{"path":"src/main.rs"}}
{"type":"match","data":{"path":"src/main.rs","line_start":42,"line_end":45,"kind":"function","scope":"App::run","signature":"pub fn run(&self) -> Result<()>"}}
{"type":"end","data":{"path":"src/main.rs","matched":1}}
{"type":"summary","data":{"matched":3,"files":2,"errors":0}}
```

## Usage Guide

### Explore before reading

1. **Explore directories before using peek** — Get the directory structure first (e.g. via `ls` or file tree), then `peek ... {path}` on individual files. Avoid `peek ... .` on the entire repo before knowing which directories matter.
2. **Check signatures first** — Signatures include parameters, return types, and attributes — often enough to judge whether the implementation body is needed. Note: single-file search omits the file path prefix; use `-H` to force it when piping.
3. **Merge adjacent line ranges** — Adjacent definitions (e.g. lines 10-15 and 18-30) can be covered by a single Read call.
4. **`...` lists everything** — When unfamiliar with a file, `peek ... {path}` gives a complete overview of its definitions. Use `-H` if you need the path prefix for downstream processing.

### Scope search

5. **Scope-aware search locates nested definitions** — `peek "MyClass::.*"` finds all definitions in `MyClass`; `peek ".*::method"` finds `method` in any class/module (non-top-level only).
6. **`-k` filters by kind** — `-k interface` for interfaces only, `-k struct,class,enum` for data structures.
7. **`-g` excludes paths** — `-g '!*generated*' -g '!*vendor*'` skips generated and vendor directories.

### Pipe filtering & processing

8. **Filter out test code** — Rust/Go tests often coexist with source. Pipe through grep: `peek ... src/ | grep -v '#\[test\]' | grep -v 'tests::'`
9. **`--json` + `jq` for structured queries** — `peek --json ... src/ | jq -r 'select(.type=="match") | "\(.data.path):\(.data.line_start) \(.data.scope)"'`. Extract scope list: `... | jq -r 'select(.type=="match") | .data.scope'`
10. **`-l` for file-level discovery** — `peek -l {pattern} src/` lists matching files, then `peek ... {path}` on individual files for details.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Matches found |
| `1` | No matches (silent) |
| `2` | Error |
