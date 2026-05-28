mod claude;
mod codex;
mod cursor;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

// ---------------------------------------------------------------------------
// Location
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    Global,
    Local,
}

// ---------------------------------------------------------------------------
// FileAction — describes what a register/unregister operation did
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileAction {
    Created(PathBuf),
    Updated(PathBuf),
    Unchanged(PathBuf),
    NotFound(PathBuf),
}

impl FileAction {
    pub fn path(&self) -> &PathBuf {
        match self {
            FileAction::Created(p)
            | FileAction::Updated(p)
            | FileAction::Unchanged(p)
            | FileAction::NotFound(p) => p,
        }
    }
}

impl std::fmt::Display for FileAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            FileAction::Created(_) => "created",
            FileAction::Updated(_) => "updated",
            FileAction::Unchanged(_) => "unchanged",
            FileAction::NotFound(_) => "not-found",
        };
        write!(f, "{} {}", label, self.path().display())
    }
}

// ---------------------------------------------------------------------------
// Target trait
// ---------------------------------------------------------------------------

pub trait Target {
    fn id(&self) -> &str;
    fn display_name(&self) -> &str;
    fn supports_local(&self) -> bool;
    fn register(&self, location: Location) -> Result<Vec<FileAction>>;
    fn unregister(&self, location: Location) -> Result<Vec<FileAction>>;
    fn config_paths(&self, location: Location) -> Vec<PathBuf>;
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

fn all_targets() -> Vec<Box<dyn Target>> {
    vec![
        Box::new(claude::ClaudeTarget),
        Box::new(cursor::CursorTarget),
        Box::new(codex::CodexTarget),
    ]
}

fn find_target(id: &str) -> Option<Box<dyn Target>> {
    match id {
        "claude" => Some(Box::new(claude::ClaudeTarget)),
        "cursor" => Some(Box::new(cursor::CursorTarget)),
        "codex" => Some(Box::new(codex::CodexTarget)),
        _ => None,
    }
}

fn resolve_location(local: bool, target: &dyn Target) -> Result<Location> {
    let location = if local {
        Location::Local
    } else {
        Location::Global
    };
    if location == Location::Local && !target.supports_local() {
        anyhow::bail!(
            "{} does not support local (project-level) registration",
            target.display_name()
        );
    }
    Ok(location)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn home_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        std::env::var("USERPROFILE")
            .map(PathBuf::from)
            .ok()
            .or_else(|| {
                std::env::var("HOMEDRIVE")
                    .ok()
                    .zip(std::env::var("HOMEPATH").ok())
                    .map(|(d, p)| PathBuf::from(format!("{}{}", d, p)))
            })
            .expect("could not determine home directory")
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::env::var("HOME")
            .map(PathBuf::from)
            .expect("could not determine home directory")
    }
}

/// Read a JSON file. Returns `{}` when missing. Backs up to `.backup` on parse error.
fn read_json_file(path: &Path) -> Value {
    match std::fs::read_to_string(path) {
        Ok(content) => match serde_json::from_str(&content) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("peek: warning: could not parse {}: {}", path.display(), e);
                let _ = std::fs::copy(path, path.with_extension("json.backup"));
                Value::Object(serde_json::Map::new())
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Value::Object(serde_json::Map::new()),
        Err(e) => {
            eprintln!("peek: warning: could not read {}: {}", path.display(), e);
            Value::Object(serde_json::Map::new())
        }
    }
}

/// Write a file atomically: write to temp file then rename.
fn atomic_write(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension("tmp");
    std::fs::write(&tmp_path, content)?;
    std::fs::rename(&tmp_path, path)?;
    Ok(())
}

/// Write JSON with pretty-printing and trailing newline.
fn write_json_file(path: &Path, data: &Value) -> Result<()> {
    let content = serde_json::to_string_pretty(data)? + "\n";
    atomic_write(path, &content)
}

/// Build the base MCP server entry for peek.
fn mcp_entry() -> Value {
    serde_json::json!({
        "type": "stdio",
        "command": "peek",
        "args": ["mcp"]
    })
}

/// Upsert `mcpServers.<name>` in config. Returns true if the config changed.
fn upsert_mcp_server(config: &mut Value, name: &str, entry: Value) -> bool {
    let servers = config
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));

    if let Some(existing) = servers.get(name) {
        if existing == &entry {
            return false;
        }
    }

    servers
        .as_object_mut()
        .unwrap()
        .insert(name.to_string(), entry);
    true
}

/// Remove `mcpServers.<name>` from config. Returns true if something was removed.
/// Cleans up empty `mcpServers` key.
fn remove_mcp_server(config: &mut Value, name: &str) -> bool {
    let obj = match config.as_object_mut() {
        Some(o) => o,
        None => return false,
    };
    let servers = match obj.get_mut("mcpServers") {
        Some(Value::Object(s)) => s,
        _ => return false,
    };
    if servers.remove(name).is_none() {
        return false;
    }
    if servers.is_empty() {
        obj.remove("mcpServers");
    }
    true
}

/// Shared JSON register logic: read file → upsert → write back.
fn json_register_at(path: &Path, entry: Value) -> Result<Vec<FileAction>> {
    let file_existed = path.exists();
    let mut config = read_json_file(path);

    let changed = upsert_mcp_server(&mut config, "peek", entry);
    if !changed {
        return Ok(vec![FileAction::Unchanged(path.to_path_buf())]);
    }

    write_json_file(path, &config)?;
    let action = if file_existed {
        FileAction::Updated(path.to_path_buf())
    } else {
        FileAction::Created(path.to_path_buf())
    };
    Ok(vec![action])
}

/// Shared JSON unregister logic: read file → remove → write back.
fn json_unregister_at(path: &Path) -> Result<Vec<FileAction>> {
    let mut config = read_json_file(path);
    let removed = remove_mcp_server(&mut config, "peek");
    if !removed {
        return Ok(vec![FileAction::NotFound(path.to_path_buf())]);
    }
    write_json_file(path, &config)?;
    Ok(vec![FileAction::Updated(path.to_path_buf())])
}

// ---------------------------------------------------------------------------
// CLI entry points
// ---------------------------------------------------------------------------

pub fn run_register(args: &crate::cli::RegisterArgs) -> anyhow::Result<std::process::ExitCode> {
    if args.list_targets {
        for target in all_targets() {
            let local_tag = if target.supports_local() {
                "global, local"
            } else {
                "global only"
            };
            println!(
                "{} ({}) — {}",
                target.id(),
                local_tag,
                target.display_name()
            );
            let global_paths = target.config_paths(Location::Global);
            for p in &global_paths {
                println!("  global: {}", p.display());
            }
            if target.supports_local() {
                let local_paths = target.config_paths(Location::Local);
                for p in &local_paths {
                    println!("  local:  {}", p.display());
                }
            }
        }
        return Ok(std::process::ExitCode::SUCCESS);
    }

    let target_id = match &args.target {
        Some(t) => t.as_str(),
        None => anyhow::bail!(
            "error: --target is required (or use --list-targets to see available platforms)"
        ),
    };

    let target = find_target(target_id).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown target: '{}'. Use --list-targets to see available platforms.",
            target_id
        )
    })?;

    let location = resolve_location(args.local, target.as_ref())?;
    let actions = target.register(location)?;
    for action in &actions {
        println!("{}", action);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

pub fn run_unregister(args: &crate::cli::UnregisterArgs) -> anyhow::Result<std::process::ExitCode> {
    let target = find_target(&args.target).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown target: '{}'. Use 'peek register --list-targets' to see available platforms.",
            args.target
        )
    })?;

    let location = resolve_location(args.local, target.as_ref())?;
    let actions = target.unregister(location)?;
    for action in &actions {
        println!("{}", action);
    }
    Ok(std::process::ExitCode::SUCCESS)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_json_file_returns_empty_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let val = read_json_file(&path);
        assert!(val.is_object());
        assert!(val.as_object().unwrap().is_empty());
    }

    #[test]
    fn read_json_file_backups_on_parse_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not valid json{{{").unwrap();
        let val = read_json_file(&path);
        assert!(val.as_object().unwrap().is_empty());
        assert!(path.with_extension("json.backup").exists());
    }
}
