---
name: peek-code
description: Use when exploring codebase structure, understanding what definitions exist in files, or searching for where a symbol is defined. Reading once is sufficient unless unsure how to use the tool.
---

# peek

`peek` searches for code definitions by name using tree-sitter AST parsing.

**Why not grep?** — understands code structure; returns only definition locations with no false positives from comments, strings, or references.

**Always prefer `peek` when:**
- Searching for a definition by name — no grep, use `peek`
- Exploring an unfamiliar file — no full-file Read, use `peek ... {path}` to survey definitions first
- Understanding a module's API surface — `peek` lists all public definitions at a glance
- Locating members within a scope — `peek "MyClass::.*"` finds all definitions under a class/module, no need to scan through the file

## Supported Languages

| Language | Extensions | Definition Kinds |
|----------|-----------|-----------------|
| Python | .py, .pyw | Function, Class, Type |
| Go | .go | Function, Struct, Interface, Type, Const |
| Rust | .rs | Function, Struct, Enum, Type, Trait, Const, Macro, Module |
| JavaScript | .js, .jsx | Function, Class, Const |
| TypeScript | .ts, .tsx, .mts, .cts | Function, Class, Const, Interface, Type, Enum |
| Java | .java | Class, Interface, Enum, Function, Const |
| C# | .cs | Class, Interface, Enum, Struct, Record, Delegate, Event, Function, Const |
| PHP | .php, .phtml, .phar | Function, Class, Interface, Trait, Enum, Const |
| C | .c | Function, Struct, Enum, Type, Const, Macro |
| C++ | .cpp, .cxx, .cc, .hpp, .hxx, .hh, .h | Function, Class, Struct, Enum, Type, Const, Macro |
| Kotlin | .kt, .kts | Class, Interface, Enum, Function, Object, Type, Const |
| Swift | .swift, .swiftinterface | Function, Class, Struct, Enum, Protocol, Type, Const, Actor, Extension |
| Ruby | .rb, .rake, .gemspec, .ru | Function, Class, Module, Const |
| Dart | .dart | Function, Class, Enum, Const, Type, Mixin, Extension |
| Bash | .sh, .bash | Function, Const |
| Lua | .lua | Function |

## Usage

```
peek [OPTIONS] <PATTERN> [FILES]...
```

## Arguments

| Argument | Description |
|----------|-------------|
| `<PATTERN>` | See Pattern section below |
| `[FILES]...` | Files or directories to search in. Default: current directory |

### Pattern

**Matching** — exact full name (not substring) by default. Regex wildcards enable fuzzy matching: `get_.*` matches `get_name` / `get_id` (prefix), `.*_handler` matches `click_handler` / `req_handler` (suffix), `foo.*bar` matches `foobar` / `foo_x_bar` (contains). The reserved pattern `...` lists all definitions without name filtering.

**Scope prefix** — narrows the search to definitions under a parent scope, using each language's native separator (`::` for Rust, `.` for Python). The scope and name parts support regex independently, split by the last separator. Escape regex special characters as needed: `\.` for literal `.`, `\\` for literal `\`.

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

1. **Explore directories before using peek** — Get the directory structure first (e.g. via `ls` or file tree), then `peek ... {path}` on individual files.
2. **Check signatures first** — Signatures include parameters, return types, and attributes — often enough to judge whether the implementation body is needed. Note: single-file search omits the file path prefix; use `-H` to force it when piping.
3. **Merge adjacent line ranges** — Adjacent definitions (e.g. lines 10-15 and 18-30) can be covered by a single Read call.
4. **`...` lists everything** — When unfamiliar with a file, `peek ... {path}` gives a complete overview of its definitions. Use `-H` if you need the path prefix for downstream processing.

> **Warning:** **Never** run `peek ... .` on the entire repo blindly. If you must search broadly, always pair with `--json` and pipe to JSON CLI tools (e.g. `jq`, `python`, `node`) for structured filtering to avoid flooding output.

### Scope search

5. **Scope-aware search locates nested definitions** — `peek "MyClass::.*"` finds all definitions in `MyClass`; `peek ".*::method"` finds `method` in any class/module (non-top-level only).
6. **`-k` filters by kind** — `-k interface` for interfaces only, `-k struct,class,enum` for data structures.
7. **`-g` excludes paths** — `-g '!*generated*' -g '!*vendor*'` skips generated and vendor directories.

### Pipe filtering & processing

8. **Filter out test code** — Rust/Go tests often coexist with source. Pipe through grep: `peek ... src/ | grep -v '#\[test\]' | grep -v 'tests::'`
9. **`--json` enables structured post-processing** — Pipe NDJSON output to JSON CLI tools (e.g. `jq`, `python`, `node`) for filtering, aggregation, or extraction. Use this whenever you need to query across results rather than scan raw output.
10. **`-l` for file-level discovery** — `peek -l {pattern} src/` lists matching files, then `peek ... {path}` on individual files for details.

## Exit Codes

| Code | Meaning |
|------|---------|
| `0` | Matches found |
| `1` | No matches (silent) |
| `2` | Error |
