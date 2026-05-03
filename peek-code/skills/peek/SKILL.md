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

## Pattern Syntax

- Uses Rust regex (same syntax as ripgrep). `.` and `\` are regex special characters — `peek App.run` treats the dot as a wildcard, matching `AppXrun`, `Apparun`, etc. in addition to the literal `App.run`; escape as `App\.run` for exact literal match, or use `-w` for whole-word matching
- Use `-e` for multiple OR patterns: `peek -e 'parse_*' -e 'build_*'`

## Examples

```bash
peek MyClass                             # search for symbol
peek 'MyClass::run'                      # scope-qualified search
peek 'run' src/lib.rs src/app.rs         # search within specific paths
peek src/main.rs                         # survey file definitions
peek -c src/                             # count definitions per file
peek -k function -g '!*test*' 'parse_*'  # filter by kind, exclude tests
peek -e 'parse_*' -e 'build_*' src/      # multiple OR patterns
peek -e 'main.rs'                        # disambiguate pattern from path
peek -k function 'parse_*' | grep -v tests::  # pipe to exclude scopes
```

## Options

- `-k, --kind <KIND>` — Filter by definition kind. Comma-separated: `function`, `class`, `struct`, `enum`, `type`, `trait`, `interface`, `const`, `record`, `delegate`, `event`, `object`, `protocol`, `actor`, `extension`, `mixin`, `module`, `macro`
- `-e, --regexp <PATTERN>` — Explicit pattern. Use to combine OR patterns (`-e 'a' -e 'b'`) or disambiguate from paths. When present, positional args become `[PATHS]` only
- `-w, --word-regexp` — Match as a whole word
- `-i, --ignore-case` — Case-insensitive matching
- `-S, --smart-case` — Case-insensitive unless pattern contains uppercase
- `-g, --glob <GLOB>` — File glob filter. Repeatable; `!` negates (e.g., `-g '*.rs' -g '!*test*'`)
- `--hidden` — Search hidden files and directories
- `--no-ignore` — Don't respect .gitignore and .ignore
- `-d, --max-depth <N>` — Max directory traversal depth
- `--json` — NDJSON output (see Output Formats)
- `-l, --files-with-matches` — Print which files matched
- `-c, --count` — Print match count per file
- `-H, --with-filename` — Always show file path prefix (default: suppressed for single file)
- `-I, --no-filename` — Never show file path prefix
- `-M, --no-messages` — Suppress non-fatal error messages
- `--no-signature` — Omit signature from default output
- `--path-separator <CHAR>` — Set path separator in output
- `-V, --version` — Print version
- `-h, --help` — Print help message

## Output Formats

Line numbers are 1-based across all formats.

**Default** — `[<path>:]<line_start>-<line_end> [[<kind>/<scope>]] <signature>`

```
src/main.rs:42-45 [[function/App::run]] pub fn run(&self) -> Result<()>
src/app.rs:10-12 [[struct/App]] struct App { /* fields */ }
```

Path prefix shown for multiple files, suppressed for single file. Use `-H` to always show, `-I` to always hide.

**`--json`** — NDJSON with `begin`/`match`/`end`/`summary` record types. `match` records always include `path`, `line_start`, `line_end`, `kind`, `scope`, `signature`. `summary` is always the last record (contains `matched`, `files`, `errors`).

## Exit Codes

Follows ripgrep convention:

- `0` — Matches found, no errors
- `1` — No matches found. **Not an error** — do not retry or report as failure
- `2` — Error occurred. Fatal errors to stderr with `peek:` prefix; non-fatal as `peek: <path>: <message>`

Priority: `2` (errors) > `1` (no matches) > `0` (success). `-M` suppresses stderr but does not change exit code.
