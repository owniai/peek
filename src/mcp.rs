use std::fmt::Write;
use std::io::{BufRead, Write as IoWrite};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 message types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String, // always "2.0"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>, // string | integer | null; null for notifications
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

// Standard JSON-RPC error codes
pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL_ERROR: i64 = -32603;

// JSON-RPC protocol version (constant across all messages)
const JSONRPC_VERSION: &str = "2.0";

// ---------------------------------------------------------------------------
// StdioTransport
// ---------------------------------------------------------------------------

pub struct StdioTransport {
    stdin: std::io::Stdin,
    stdout: std::io::Stdout,
}

impl StdioTransport {
    pub fn new() -> Self {
        Self {
            stdin: std::io::stdin(),
            stdout: std::io::stdout(),
        }
    }

    pub fn read_request(&mut self) -> Result<Option<JsonRpcRequest>> {
        loop {
            let mut line = String::new();
            let bytes = self.stdin.lock().read_line(&mut line)?;
            if bytes == 0 {
                return Ok(None); // EOF — client disconnected
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue; // blank line — skip, don't treat as EOF
            }
            let request: JsonRpcRequest = serde_json::from_str(trimmed)?;
            return Ok(Some(request));
        }
    }

    pub fn write_response(&mut self, response: &JsonRpcResponse) -> Result<()> {
        let json = serde_json::to_string(response)?;
        let mut out = self.stdout.lock();
        writeln!(out, "{}", json)?;
        out.flush()?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// MCP session
// ---------------------------------------------------------------------------

pub struct McpSession {
    project_root: Option<std::path::PathBuf>,
    initialized: bool,
    registry: crate::registry::ParserRegistry,
}

impl McpSession {
    pub fn new() -> Self {
        Self {
            project_root: None,
            initialized: false,
            registry: crate::registry::ParserRegistry::default_registry(),
        }
    }

    pub fn handle_initialize(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        if self.initialized {
            return error_response(
                request.id.clone(),
                INVALID_REQUEST,
                "Session already initialized",
            );
        }
        self.initialized = true;

        let instructions = "peek finds code definitions and declarations by name using AST-level parsing — more precise than Grep. Supported languages: python, go, rust, javascript, typescript, java, csharp, php, c, cpp, kotlin, swift, ruby, dart, bash, lua, luau, objc.\n\n- `set_project_root` — set the project root directory (MUST be called before other peek tools)\n- `peek_def` — locate where a named symbol is defined or declared\n- `peek_outline` — survey a code file's definition structure before reading it\n\nUse regex patterns and kind/category filters to narrow results. Filter files by glob, language, or kind/category.\n\nFor text content, variable assignments, or all usages of a symbol — use Grep instead.";

        JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id: request.id.clone(),
            result: Some(json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "peek", "version": env!("CARGO_PKG_VERSION") },
                "instructions": instructions
            })),
            error: None,
        }
    }

    pub fn handle_tools_list(&self, request: &JsonRpcRequest) -> JsonRpcResponse {
        JsonRpcResponse {
            jsonrpc: JSONRPC_VERSION.into(),
            id: request.id.clone(),
            result: Some(json!({
                "tools": [
                    PEEK_DEF_TOOL.clone(),
                    PEEK_OUTLINE_TOOL.clone(),
                    SET_PROJECT_ROOT_TOOL.clone(),
                ]
            })),
            error: None,
        }
    }

    pub fn handle_tools_call(&mut self, request: &JsonRpcRequest) -> JsonRpcResponse {
        if !self.initialized {
            return error_response(
                request.id.clone(),
                INVALID_REQUEST,
                "Server not initialized",
            );
        }
        let tool_name = request
            .params
            .as_ref()
            .and_then(|p| p.get("name"))
            .and_then(|v| v.as_str());
        let tool_params = request.params.as_ref().and_then(|p| p.get("arguments"));

        match tool_name {
            Some("peek_def") => self.handle_peek_def(request.id.clone(), tool_params),
            Some("peek_outline") => self.handle_peek_outline(request.id.clone(), tool_params),
            Some("set_project_root") => {
                self.handle_set_project_root(request.id.clone(), tool_params)
            }
            Some(_) => error_response(request.id.clone(), METHOD_NOT_FOUND, "Unknown tool"),
            None => error_response(request.id.clone(), INVALID_PARAMS, "Missing tool name"),
        }
    }

    fn handle_set_project_root(
        &mut self,
        id: Option<Value>,
        params: Option<&Value>,
    ) -> JsonRpcResponse {
        let args = params.unwrap_or(&Value::Null);
        let root = args.get("root").and_then(|v| v.as_str());
        if root.is_none_or(|s| s.is_empty()) {
            return error_response(id, INVALID_PARAMS, "Missing required parameter: root");
        }
        let root = root.unwrap();
        let path = std::path::Path::new(root);
        if !path.is_absolute() {
            return error_response(id, INVALID_PARAMS, "root must be an absolute path");
        }
        self.project_root = Some(path.to_path_buf());
        let resolved = path.to_string_lossy();
        success_text_response(id, &format!("Project root set to {}", resolved))
    }

    fn validate_kinds(kind: Option<&str>) -> Result<Vec<crate::model::DefKind>, String> {
        let kinds = match kind {
            Some(tag) => crate::model::DefKind::kinds_from_tag(tag),
            None => crate::model::DefKind::all().to_vec(),
        };
        if let Some(kind_str) = kind {
            if kinds.is_empty() {
                let mut valid: Vec<&str> = crate::model::DefKind::all()
                    .iter()
                    .map(|k| k.display_tag())
                    .collect();
                valid.extend(
                    crate::model::Category::all()
                        .iter()
                        .map(|c| c.display_tag()),
                );
                return Err(format!(
                    "Unknown kind: {}. Valid kinds: {}",
                    kind_str,
                    valid.join(", ")
                ));
            }
        }
        Ok(kinds)
    }

    fn validate_language(language: Option<&str>) -> Result<Vec<String>, String> {
        let languages = match language {
            Some(l) => vec![l.to_string()],
            None => vec![],
        };
        if let Some(tag) = language {
            if crate::registry::resolve_language(tag).is_none() {
                return Err(format!(
                    "Unknown language: {}. Valid languages: {}",
                    tag,
                    crate::registry::KNOWN_LANGUAGES.join(", ")
                ));
            }
        }
        Ok(languages)
    }

    fn handle_peek_def(&mut self, id: Option<Value>, params: Option<&Value>) -> JsonRpcResponse {
        let args = params.unwrap_or(&Value::Null);

        let pattern = args.get("pattern").and_then(|v| v.as_str());
        if pattern.is_none() {
            return error_response(id, INVALID_PARAMS, "Missing required parameter: pattern");
        }
        let pattern = pattern.unwrap();

        if self.project_root.is_none() {
            return error_text_response(id, "Call set_project_root before searching.");
        }

        let path = args.get("path").and_then(|v| v.as_str());
        let kind = args.get("kind").and_then(|v| v.as_str());
        let glob = args.get("glob").and_then(|v| v.as_str());
        let language = args.get("language").and_then(|v| v.as_str());
        let case_insensitive = args
            .get("case_insensitive")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let word_match = args
            .get("word_match")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let head_limit = args
            .get("head_limit")
            .and_then(|v| v.as_u64())
            .map(|n| n as usize);

        let call_root = self.project_root.clone();

        // Parse pattern
        let case = if case_insensitive {
            crate::pattern::CaseSensitivity::Insensitive
        } else {
            crate::pattern::CaseSensitivity::Sensitive
        };
        let parsed = match crate::pattern::ParsedPattern::parse(pattern, case, word_match) {
            Ok(p) => p,
            Err(e) => {
                let msg = e.to_string();
                let stripped = strip_peek_prefix(&msg);
                return error_response(id, INVALID_PARAMS, stripped);
            }
        };
        let modes = vec![parsed.mode().clone()];

        let kinds = match Self::validate_kinds(kind) {
            Ok(k) => k,
            Err(msg) => return error_response(id, INVALID_PARAMS, &msg),
        };

        let search_paths: Vec<&std::path::Path> = match path {
            Some(p) => vec![std::path::Path::new(p)],
            None => vec![std::path::Path::new(".")],
        };

        let globs = match glob {
            Some(g) => vec![g.to_string()],
            None => vec![],
        };
        let languages = match Self::validate_language(language) {
            Ok(l) => l,
            Err(msg) => return error_response(id, INVALID_PARAMS, &msg),
        };

        let options = crate::pipeline::SearchOptions {
            hidden: false,
            no_ignore: false,
            max_depth: None,
            max_scope_depth: None,
            project_root: call_root.clone(),
        };

        let result = match crate::pipeline::search(
            &modes,
            &kinds,
            &search_paths,
            &globs,
            &languages,
            &options,
            &self.registry,
        ) {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                let stripped = strip_peek_prefix(&msg);
                return error_response(id, INTERNAL_ERROR, stripped);
            }
        };

        let mut definitions = result.definitions;
        let read_errors = result.read_errors;
        let parse_failures = result.parse_failures;

        if let Some(limit) = head_limit {
            let mut total = 0;
            let cutoff = definitions
                .iter()
                .position(|fd| {
                    total += fd.defs.len();
                    total > limit
                })
                .unwrap_or(definitions.len());
            definitions.truncate(cutoff);
            // Truncate the boundary FileDefs if it overshoots the limit
            if !definitions.is_empty() {
                let total_before = definitions
                    .iter()
                    .take(definitions.len() - 1)
                    .map(|fd| fd.defs.len())
                    .sum::<usize>();
                let remaining = limit - total_before;
                if remaining < definitions.last().unwrap().defs.len() {
                    definitions.last_mut().unwrap().defs.truncate(remaining);
                }
            }
        }

        let text = format_mcp_results(&definitions, &call_root);
        let base = resolve_base(&call_root);
        let text = append_error_summary(text, &read_errors, &parse_failures, &base);

        success_text_response(id, &text)
    }

    fn handle_peek_outline(
        &mut self,
        id: Option<Value>,
        params: Option<&Value>,
    ) -> JsonRpcResponse {
        let args = params.unwrap_or(&Value::Null);

        let path = args.get("path").and_then(|v| v.as_str());
        if path.is_none() {
            return error_response(id, INVALID_PARAMS, "Missing required parameter: path");
        }
        let path = path.unwrap();

        if self.project_root.is_none() {
            return error_text_response(id, "Call set_project_root before searching.");
        }

        let kind = args.get("kind").and_then(|v| v.as_str());
        let language = args.get("language").and_then(|v| v.as_str());

        let call_root = self.project_root.clone();

        let kinds = match Self::validate_kinds(kind) {
            Ok(k) => k,
            Err(msg) => return error_response(id, INVALID_PARAMS, &msg),
        };

        let languages = match Self::validate_language(language) {
            Ok(l) => l,
            Err(msg) => return error_response(id, INVALID_PARAMS, &msg),
        };

        let modes = vec![crate::parser::MatchMode::All];
        let search_paths: Vec<&std::path::Path> = vec![std::path::Path::new(path)];

        let options = crate::pipeline::SearchOptions {
            hidden: false,
            no_ignore: false,
            max_depth: None,
            max_scope_depth: None,
            project_root: call_root.clone(),
        };

        let result = match crate::pipeline::search(
            &modes,
            &kinds,
            &search_paths,
            &[],
            &languages,
            &options,
            &self.registry,
        ) {
            Ok(r) => r,
            Err(e) => {
                let msg = e.to_string();
                let stripped = strip_peek_prefix(&msg);
                return error_response(id, INTERNAL_ERROR, stripped);
            }
        };

        let definitions = result.definitions;
        let read_errors = result.read_errors;
        let parse_failures = result.parse_failures;

        let text = format_mcp_survey(&definitions, &call_root);
        let base = resolve_base(&call_root);
        let text = append_error_summary(text, &read_errors, &parse_failures, &base);

        success_text_response(id, &text)
    }
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

static SET_PROJECT_ROOT_TOOL: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "name": "set_project_root",
        "description": "Set the project root directory — all result paths are relative to this root. MUST be called before other peek tools. Set once per session; subsequent calls overwrite.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "root": {
                    "type": "string",
                    "description": "Absolute path to the project root directory."
                }
            },
            "required": ["root"]
        }
    })
});

static PEEK_DEF_TOOL: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "name": "peek_def",
        "description": "Find code definitions and declarations by name using AST-level parsing. More precise than Grep — no comments, strings, or partial matches.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The regular expression pattern to search for definition and declaration names. Supports full regex syntax (e.g., `MyClass`, `parse_*`, `run|execute`). Scope separators `.` and `\\` are regex-special; escape for exact scope match: `App\\.run`, `App\\\\Models`."
                },
                "path": {
                    "type": "string",
                    "description": "File or directory to search in. Defaults to project root."
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by definition kind. Common kinds: function, method, class, struct, enum, interface, trait, const, module, macro, alias, namespace, field, property, var. Category shortcuts expand to related kinds: callable — functions, methods, constructors · shape — classes, structs, enums · value — consts, fields, properties · contract — interfaces, traits, protocols · scope — namespaces, modules, packages."
                },
                "glob": {
                    "type": "string",
                    "description": "Glob pattern to filter files (e.g., `*.rs`, `*.{ts,tsx}`). Negate with `!` (e.g., `!*.test.*`)."
                },
                "language": {
                    "type": "string",
                    "description": "Filter by language and resolve ambiguous extensions. E.g., `.h` → c, cpp, or objc; `.lua` → lua or luau."
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Case-insensitive matching. Defaults to false."
                },
                "word_match": {
                    "type": "boolean",
                    "description": "Match whole identifier names only. run matches fn run() but not fn runtime(). Defaults to false."
                },
                "head_limit": {
                    "type": "number",
                    "description": "Limit output to first N results. Defaults to unlimited."
                }
            },
            "required": ["pattern"]
        }
    })
});

static PEEK_OUTLINE_TOOL: LazyLock<Value> = LazyLock::new(|| {
    json!({
        "name": "peek_outline",
        "description": "Survey a code file's definition structure — kinds, scopes, and signatures. Use before reading to understand layout. Not for searching by name (use peek_def) or reading content (use Read on the returned line range).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File or directory to list definitions and declarations for."
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by definition kind. Common kinds: function, method, class, struct, enum, interface, trait, const, module, macro, alias, namespace, field, property, var. Category shortcuts expand to related kinds: callable — functions, methods, constructors · shape — classes, structs, enums · value — consts, fields, properties · contract — interfaces, traits, protocols · scope — namespaces, modules, packages."
                },
                "language": {
                    "type": "string",
                    "description": "Filter by language and resolve ambiguous extensions. E.g., `.h` → c, cpp, or objc; `.lua` → lua or luau."
                }
            },
            "required": ["path"]
        }
    })
});

// ---------------------------------------------------------------------------
// Result formatting
// ---------------------------------------------------------------------------

fn resolve_base(project_root: &Option<std::path::PathBuf>) -> std::path::PathBuf {
    project_root
        .as_ref()
        .expect("project_root must be set via set_project_root before searching")
        .clone()
}

fn format_mcp_results(
    definitions: &[crate::model::FileDefs],
    project_root: &Option<std::path::PathBuf>,
) -> String {
    let base = resolve_base(project_root);
    let mut out = String::new();
    for fd in definitions {
        let file = crate::output::relativize_path(&fd.file, &base);
        for def in &fd.defs {
            out.push_str(&crate::output::format_def_line(&file, def, true));
            out.push('\n');
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn format_mcp_survey(
    definitions: &[crate::model::FileDefs],
    project_root: &Option<std::path::PathBuf>,
) -> String {
    let base = resolve_base(project_root);
    let mut out = String::new();
    for fd in definitions {
        let file = crate::output::relativize_path(&fd.file, &base);
        let mut max_end: u32 = 0;
        for def in &fd.defs {
            let is_contained = def.lines[1] <= max_end;
            if is_contained {
                let range = crate::output::format_line_range(def.lines[0], def.lines[1]);
                let sig =
                    crate::output::truncate_str(&def.signature, crate::output::MAX_SIGNATURE_LEN);
                let truncation = if def.signature.len() > crate::output::MAX_SIGNATURE_LEN {
                    " [truncated]"
                } else {
                    ""
                };
                out.push_str(&format!("  {} {}{}", range, sig, truncation));
                out.push('\n');
            } else {
                out.push_str(&crate::output::format_def_line(&file, def, true));
                out.push('\n');
            }
            if def.lines[1] > max_end {
                max_end = def.lines[1];
            }
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn strip_peek_prefix(msg: &str) -> &str {
    msg.strip_prefix("peek: ").unwrap_or(msg)
}

fn append_error_summary(
    text: String,
    read_errors: &[crate::pipeline::FileError],
    parse_failures: &[crate::pipeline::FileError],
    base: &Path,
) -> String {
    if read_errors.is_empty() && parse_failures.is_empty() {
        return text;
    }
    let mut out = text;
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str("--- errors ---\n");
    for err in read_errors {
        let path = crate::output::relativize_path(&err.path, base);
        let _ = write!(out, "read: {}: {}", path, err.message);
        out.push('\n');
    }
    for err in parse_failures {
        let path = crate::output::relativize_path(&err.path, base);
        let _ = write!(out, "parse: {}: {}", path, err.message);
        out.push('\n');
    }
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn error_response(id: Option<Value>, code: i64, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION.into(),
        id,
        result: None,
        error: Some(JsonRpcError {
            code,
            message: message.into(),
            data: None,
        }),
    }
}

fn error_text_response(id: Option<Value>, message: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION.into(),
        id,
        result: Some(json!({
            "content": [{ "type": "text", "text": message }],
            "isError": true
        })),
        error: None,
    }
}

fn success_text_response(id: Option<Value>, text: &str) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: JSONRPC_VERSION.into(),
        id,
        result: Some(json!({
            "content": [{ "type": "text", "text": if text.is_empty() { "No definitions found." } else { text } }]
        })),
        error: None,
    }
}

// ---------------------------------------------------------------------------
// MCP server entry point
// ---------------------------------------------------------------------------

pub fn serve() -> Result<()> {
    let mut transport = StdioTransport::new();
    let mut session = McpSession::new();

    loop {
        let request = match transport.read_request() {
            Ok(Some(r)) => r,
            Ok(None) => break, // EOF
            Err(e) => {
                let msg = e.to_string();
                let resp = error_response(None, PARSE_ERROR, &msg);
                transport.write_response(&resp)?;
                continue;
            }
        };

        let response = match request.method.as_str() {
            "initialize" => session.handle_initialize(&request),
            "notifications/initialized" => {
                // Notification — no response per spec
                continue;
            }
            "tools/list" => session.handle_tools_list(&request),
            "tools/call" => session.handle_tools_call(&request),
            "ping" => JsonRpcResponse {
                jsonrpc: JSONRPC_VERSION.into(),
                id: request.id.clone(),
                result: Some(json!({})),
                error: None,
            },
            _ => error_response(
                request.id.clone(),
                METHOD_NOT_FOUND,
                &format!("Method not found: {}", request.method),
            ),
        };

        transport.write_response(&response)?;
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn init_session() -> McpSession {
        let mut session = McpSession::new();
        session.initialized = true;
        session
    }

    // --- Session guard behavior ---

    #[test]
    fn tools_call_before_init_returns_error() {
        let mut session = McpSession::new();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(1)),
            method: "tools/call".into(),
            params: Some(json!({ "name": "peek_def", "arguments": { "pattern": "foo" } })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_REQUEST);
    }

    #[test]
    fn unknown_tool_returns_method_not_found() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(2)),
            method: "tools/call".into(),
            params: Some(json!({ "name": "nonexistent" })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, METHOD_NOT_FOUND);
    }

    // --- Required parameter validation ---

    #[test]
    fn peek_def_missing_pattern_returns_error() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(3)),
            method: "tools/call".into(),
            params: Some(json!({ "name": "peek_def", "arguments": {} })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
        assert!(resp.error.as_ref().unwrap().message.contains("pattern"));
    }

    #[test]
    fn peek_outline_missing_path_returns_error() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(4)),
            method: "tools/call".into(),
            params: Some(json!({ "name": "peek_outline", "arguments": {} })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
        assert!(resp.error.as_ref().unwrap().message.contains("path"));
    }

    #[test]
    fn peek_def_invalid_regex_returns_error() {
        let mut session = init_session();
        session.project_root = Some(std::path::PathBuf::from("C:\\dummy"));
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(5)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "peek_def",
                "arguments": { "pattern": "(unclosed" }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
    }

    #[test]
    fn peek_def_invalid_kind_returns_error() {
        let mut session = init_session();
        session.project_root = Some(std::path::PathBuf::from("C:\\dummy"));
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(6)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "peek_def",
                "arguments": { "pattern": "foo", "kind": "nonexistent_kind" }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
        assert!(
            resp.error
                .as_ref()
                .unwrap()
                .message
                .contains("Unknown kind")
        );
    }

    // --- set_project_root behavior ---

    #[test]
    fn set_project_root_returns_resolved_path() {
        let dir = tempfile::tempdir().unwrap();
        let abs = dir.path().to_string_lossy().to_string();
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(10)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "set_project_root",
                "arguments": { "root": abs }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert!(result.get("isError").is_none());
        let text = result["content"][0]["text"].as_str().unwrap();
        let expected = dir.path().to_string_lossy();
        assert!(
            text.contains(&*expected),
            "response should contain resolved path, got: {text}"
        );
    }

    #[test]
    fn set_project_root_missing_root_returns_error() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(11)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "set_project_root",
                "arguments": {}
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
    }

    #[test]
    fn set_project_root_rejects_relative_path() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(12)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "set_project_root",
                "arguments": { "root": "relative/path" }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.error.is_some());
        assert_eq!(resp.error.as_ref().unwrap().code, INVALID_PARAMS);
        assert!(resp.error.as_ref().unwrap().message.contains("absolute"));
    }

    // --- Pre-check: project_root must be set before searching ---

    #[test]
    fn peek_def_before_set_project_root_returns_is_error() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(15)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "peek_def",
                "arguments": { "pattern": "foo" }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], json!(true));
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("set_project_root"));
    }

    #[test]
    fn peek_outline_before_set_project_root_returns_is_error() {
        let mut session = init_session();
        let req = JsonRpcRequest {
            jsonrpc: JSONRPC_VERSION.into(),
            id: Some(json!(16)),
            method: "tools/call".into(),
            params: Some(json!({
                "name": "peek_outline",
                "arguments": { "path": "src/" }
            })),
        };
        let resp = session.handle_tools_call(&req);
        assert!(resp.result.is_some());
        let result = resp.result.unwrap();
        assert_eq!(result["isError"], json!(true));
        let text = result["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("set_project_root"));
    }
}
