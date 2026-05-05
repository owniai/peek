---
name: peek
description: Use when searching for where a named symbol is defined or declared, or surveying a file's definitions and declarations before reading. More precise than text search for definition/declaration lookup. Not for general text search, variable assignments, or finding all usages of a symbol; use Grep instead.
---

# peek

A powerful AST-based definition and declaration search tool built on tree-sitter. Supports Python, Go, Rust, JavaScript, TypeScript, Java, C#, PHP, C, C++, Kotlin, Swift, Ruby, Dart, Bash, Lua — file type auto-detected from extension. `peek` is a CLI tool invoked via Bash, not a built-in tool.

## Usage

```
peek [OPTIONS] [<QUERY>] [PATHS]...
```

- ALWAYS use `peek <PATTERN>` to locate where a named symbol is defined or declared (AST-aware, more precise than text search)
- ALWAYS use `peek <FILE>` to survey definitions before reading a code file for the first time
- ALWAYS use `peek -c <DIR>` to count definitions per file before exploring an unfamiliar code directory
- Do NOT use `peek` for non-definition search (e.g., variable assignments, all usages) — use Grep instead
- Do NOT use `peek` to read code — it shows locations and signatures only; use `Read` on the returned line range
- For open-ended exploration requiring multiple rounds of searches, delegate to Agent with the `peek-code:peek` skill

## Query Semantics

Mode is selected by the first positional argument:
1. **Existing file or directory** → survey mode — lists all definitions in that target
2. **Anything else** → search mode — matches definition names by regex across the codebase
3. **`-e` flag present** → forces search mode; all positional arguments become scope paths only

`[PATHS]` narrows search scope in search mode. Defaults to current working directory.

Callable-kind bodies are not recursed into (Bash and Lua excepted).

## Pattern Syntax

- Uses Rust regex (same syntax as ripgrep). All regex special characters (`.`, `*`, `+`, `?`, `|`, `(`, `)`, `[`, `]`, `{`, `}`, `\`, `^`, `$`) lose literal meaning — escape with `\` for exact match, or use `-w` for whole-word matching
- Scope separators `.` and `\` are regex special — escape with `\` for exact match (e.g., `peek App\.run`, `peek App\\Models`)

## Examples

```bash
peek MyClass                             # search for symbol
peek 'MyClass::run'                      # scope-qualified search
peek 'run' src/lib.rs src/app.rs         # search within specific paths
peek src/main.rs                         # survey file definitions
peek -c src/                             # count definitions per file
peek -k callable 'parse_*'               # filter by category: functions/methods/constructors etc.
peek -k function -g '!*test*' 'parse_*'  # filter by kind, exclude tests
peek -e 'parse_*' -e 'build_*' src/      # multiple OR patterns
peek 'parse_*|build_*' src/              # equivalent to previous: regex alternation
peek -e 'main.rs'                        # disambiguate pattern from path
peek -D 1 src/                           # top-level definitions only
peek -k function 'parse_*' | grep -v tests::  # pipe to exclude scopes
```

## Options

- `-k, --kind <KIND>` — Filter by definition kind. Comma-separated specific kinds, or category tags that expand:
  - **Kinds**: `function`, `method`, `constructor`, `getter`, `setter`, `operator`, `destructor`, `subscript`, `class`, `struct`, `enum`, `union`, `record`, `object`, `actor`, `const`, `event`, `field`, `property`, `static`, `variant`, `interface`, `trait`, `protocol`, `mixin`, `extension`, `delegate`, `module`, `macro`, `alias`, `namespace`, `package`, `annotation`
  - **Categories**: `callable` → function/method/constructor/getter/setter/operator/destructor/subscript · `shape` → class/struct/enum/union/record/object/actor · `value` → const/event/field/property/static/variant · `contract` → interface/trait/protocol/mixin/extension/delegate
- `-e, --regexp <PATTERN>` — Explicit pattern. Use to combine OR patterns (`-e 'a' -e 'b'`) or disambiguate from paths. When present, positional args become `[PATHS]` only
- `-w, --word-regexp` — Match as a whole word
- `-i, --ignore-case` / `-S, --smart-case` — Case-insensitive / smart case (mutually exclusive, default: case-sensitive)
- `-g, --glob <GLOB>` — File glob filter. Repeatable; `!` negates (e.g., `-g '*.rs' -g '!*test*'`)
- `-D, --max-scope-depth <N>` — Filter by scope path depth (1 = top-level definitions only)
- `-l, --files-with-matches` / `-c, --count` — Print matching files / match count per file (mutually exclusive)
- `--json` — NDJSON output (see Output Formats)
- `--no-signature` — Omit signatures from output
- Additional flags (ripgrep-compatible): `--heading`/`--no-heading`, `--hidden`, `--no-ignore`, `-d, --max-depth <N>`, `-H, --with-filename`, `-I, --no-filename`, `-M, --no-messages`, `-V, --version`, `-h, --help`

## Output Formats

Line numbers are 1-based. Path prefix shown for multiple files, suppressed for single file (`-H` always on, `-I` always off). When stdout is a tty, `--heading` groups by file (path on own line). Signatures exceeding 256 characters are truncated with ` [truncated]`.

**Default** — `[<path>:]<line> [<kind>/<scope>] <signature>`

Single-line: `line` (e.g., `15`). Multi-line: `start-end` (e.g., `10-25`).

```
src/main.rs:42-45 [function/App::run] pub fn run(&self) -> Result<()>
src/app.rs:10 [struct/App] struct App { /* fields */ }
```

**Survey** — Definitions contained within the previous definition's range use abbreviated format (no path prefix, no `[kind/scope]`):

```
src/app.py:1-30 [class/App] class App:
3 MAX = 100
5-8 def __init__(self, name):
10-15 def run(self):
```

With `--no-signature`, contained definitions are entirely omitted.

**`--json`** — NDJSON: `begin`/`match`/`end`/`summary` records. `match` includes `path`, `line_start`, `line_end`, `kind`, `scope`, `signature`. `summary` (always last) includes `matched`, `files`, `errors`.

## Exit Codes

Follows ripgrep convention:

- `0` — Matches found, no errors
- `1` — No matches found. **Not an error** — do not retry or report as failure
- `2` — Error occurred. Fatal errors to stderr with `peek:` prefix; non-fatal as `peek: <path>: <message>`

Priority: `2` (errors) > `1` (no matches) > `0` (success). `-M` suppresses stderr but does not change exit code.
