---
name: peek
description: Use when searching for where a named symbol is defined or declared, or surveying a file's definitions and declarations before reading. More precise than text search for definition/declaration lookup. Not for general text search, variable assignments, or finding all usages of a symbol; use Grep instead.
---

# peek

A powerful AST-based definition and declaration search tool built on tree-sitter. Supports Python, Go, Rust, JavaScript, TypeScript, Java, C#, PHP, C, C++, Kotlin, Swift, Ruby, Dart, Bash, Lua (auto-detected). `peek` is a CLI tool invoked via Bash commands, not a native tool.

## Usage

```
peek [OPTIONS] [<PATTERN>] [FILES]...
```

- ALWAYS use `peek <pattern>` to locate where a named symbol is defined or declared — do NOT use Grep for definition/declaration lookup; `peek` is AST-aware and more precise than text search
- ALWAYS run `peek ... <file>` before reading an unfamiliar code file — returns all definitions and declarations in that file (survey mode)
- ALWAYS run `peek -c ... <dir>` before exploring an unfamiliar directory — shows definition/declaration counts per file, then survey selected files individually
- `<PATTERN>` matches against fully qualified scope using ripgrep-aligned regex (e.g., `MyClass::run`, `MyClass::.*`). `.` and `\` are metacharacters — use `App\.run` to match exactly `App.run`
- Filter files with `-g`/`--glob` flag (e.g., `-g '*.rs'`, `-g '!*test*'`)
- To filter test definitions (recommended), use `-g '!*test*'` to exclude test files, or pipe through `grep -v` to skip test scopes (e.g., `peek ... src/lib.rs | grep -v tests::`)
- Do NOT use `peek` for non-definition/declaration text search (variable assignments, string literals, comments, references, import/include resolution, or finding all usages) — use Grep instead
- Do NOT use `peek` to read code — peek shows definition and declaration locations and signatures. Use `peek` to locate the definition or declaration, then `Read` the returned line range
- For open-ended exploration requiring multiple rounds of searches, use Agent instead — instruct subagents to invoke the `peek-code:peek` skill first

## Options

- `-k, --kind <KIND>` — Filter by definition kind. Comma-separated: `function`, `class`, `struct`, `enum`, `type`, `trait`, `interface`, `const`, `record`, `delegate`, `event`, `object`, `protocol`, `actor`, `extension`, `mixin`, `module`, `macro`
- `-e, --regexp <PATTERN>` — Pattern via flag. Repeatable with OR semantics (e.g. `-e 'get_.*' -e 'set_.*'`). Like ripgrep, positional args become file/directory paths when this flag is used
- `-w, --word-regexp` — Match pattern as a whole word
- `-i, --ignore-case` — Case-insensitive matching
- `-S, --smart-case` — Case-insensitive unless pattern contains uppercase
- `-g, --glob <GLOB>` — File glob filter. Repeatable; `!` negates (e.g. `-g '*.rs' -g '!*test*'`)
- `--hidden` — Search hidden files and directories
- `--no-ignore` — Don't respect .gitignore and .ignore
- `-d, --max-depth <N>` — Max directory traversal depth
- `--json` — NDJSON output (see Output Formats)
- `-l, --files-with-matches` — Print only file paths
- `-c, --count` — Print match count per file
- `-H, --with-filename` — Always print file path prefix (default: suppressed for single file, shown for multiple)
- `-I, --no-filename` — Never print file path prefix
- `-M, --no-messages` — Suppress non-fatal error messages
- `--no-signature` — Omit signature from default output
- `-h, --help` — Print help message with all available options

## Output Formats

Line numbers are 1-based across all formats.

**Default** — `[<path>:]<line_start>-<line_end> [[<kind>/<scope>]] <signature>`

Path prefix is shown when searching multiple files, suppressed for a single file. Use `-H` to always show or `-I` to always hide.

**`--json`** — NDJSON, one JSON object per line. `-l`/`-c` control whether `match` records are emitted; envelope structure is always the same.

```json
{"type":"begin","data":{"path":"src/main.rs"}}
{"type":"match","data":{"path":"src/main.rs","line_start":42,"line_end":45,"kind":"function","scope":"App::run","signature":"pub fn run(&self) -> Result<()>"}}
{"type":"end","data":{"path":"src/main.rs","matched":1}}
{"type":"summary","data":{"matched":3,"files":2,"errors":0}}
```

- `summary` is always the last record, even when `matched` is 0
- `signature` is always included in `match` records; `--no-signature` has no effect in JSON mode
- `errors` in `summary` counts file read errors and parse failures; individual details go to stderr only

## Exit Codes

Follows ripgrep convention:
- `0` — Matches found, no errors
- `1` — No matches found. **Not an error** — do not retry or report as failure
- `2` — Error occurred. Fatal errors printed to stderr with `peek:` prefix; non-fatal errors (permission denied, parse failures) as `peek: <path>: <message>`

Priority: `2` (errors present) > `1` (no matches) > `0` (success). `--no-messages` / `-M` suppresses stderr output but does **not** change the exit code.
