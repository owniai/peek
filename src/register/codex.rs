use super::{FileAction, Location, Target};
use anyhow::Result;
use std::path::{Path, PathBuf};

pub(crate) struct CodexTarget;

impl Target for CodexTarget {
    fn id(&self) -> &str {
        "codex"
    }
    fn display_name(&self) -> &str {
        "Codex CLI"
    }
    fn supports_local(&self) -> bool {
        false
    }

    fn register(&self, location: Location) -> Result<Vec<FileAction>> {
        let path = config_path(location);
        register_at(&path)
    }

    fn unregister(&self, location: Location) -> Result<Vec<FileAction>> {
        let path = config_path(location);
        unregister_at(&path)
    }

    fn config_paths(&self, location: Location) -> Vec<PathBuf> {
        vec![config_path(location)]
    }
}

fn config_path(location: Location) -> PathBuf {
    match location {
        Location::Global => super::home_dir().join(".codex").join("config.toml"),
        Location::Local => unreachable!("Codex does not support local registration"),
    }
}

fn build_peek_entry() -> toml::Value {
    let mut entry = toml::map::Map::new();
    entry.insert("command".into(), toml::Value::String("peek".into()));
    entry.insert(
        "args".into(),
        toml::Value::Array(vec![toml::Value::String("mcp".into())]),
    );
    toml::Value::Table(entry)
}

fn register_at(path: &Path) -> Result<Vec<FileAction>> {
    let file_existed = path.exists();
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut doc = content
        .parse::<toml::Value>()
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));

    let peek_entry = build_peek_entry();

    // Get or create mcp_servers table
    let table = doc.as_table_mut().unwrap();
    let servers = table
        .entry("mcp_servers")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();

    // Idempotency check
    if servers.get("peek") == Some(&peek_entry) {
        return Ok(vec![FileAction::Unchanged(path.to_path_buf())]);
    }

    servers.insert("peek".into(), peek_entry);

    let output = toml::to_string_pretty(&doc)? + "\n";
    super::atomic_write(path, &output)?;

    let action = if file_existed {
        FileAction::Updated(path.to_path_buf())
    } else {
        FileAction::Created(path.to_path_buf())
    };
    Ok(vec![action])
}

fn unregister_at(path: &Path) -> Result<Vec<FileAction>> {
    if !path.exists() {
        return Ok(vec![FileAction::NotFound(path.to_path_buf())]);
    }

    let content = std::fs::read_to_string(path)?;
    let mut doc = content
        .parse::<toml::Value>()
        .unwrap_or_else(|_| toml::Value::Table(toml::map::Map::new()));

    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return Ok(vec![FileAction::NotFound(path.to_path_buf())]),
    };

    let servers = match table.get_mut("mcp_servers") {
        Some(toml::Value::Table(s)) => s,
        _ => return Ok(vec![FileAction::NotFound(path.to_path_buf())]),
    };

    if servers.remove("peek").is_none() {
        return Ok(vec![FileAction::NotFound(path.to_path_buf())]);
    }

    // Clean up empty mcp_servers table
    if servers.is_empty() {
        table.remove("mcp_servers");
    }

    let output = toml::to_string_pretty(&doc)? + "\n";
    super::atomic_write(path, &output)?;

    Ok(vec![FileAction::Updated(path.to_path_buf())])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- register_at ---

    #[test]
    fn register_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let actions = register_at(&path).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], FileAction::Created(path.clone()));

        let content = std::fs::read_to_string(&path).unwrap();
        let config: toml::Value = content.parse().unwrap();
        assert_eq!(
            config["mcp_servers"]["peek"]["command"],
            toml::Value::String("peek".into())
        );
        assert_eq!(
            config["mcp_servers"]["peek"]["args"],
            toml::Value::Array(vec![toml::Value::String("mcp".into())])
        );
    }

    #[test]
    fn register_preserves_other_sections() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[mcp_servers.other]\ncommand = \"other\"\n").unwrap();

        register_at(&path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let config: toml::Value = content.parse().unwrap();
        assert_eq!(
            config["mcp_servers"]["other"]["command"],
            toml::Value::String("other".into())
        );
        assert_eq!(
            config["mcp_servers"]["peek"]["command"],
            toml::Value::String("peek".into())
        );
    }

    #[test]
    fn register_idempotent_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // First register — creates
        let actions = register_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::Created(path.clone()));

        // Second register — unchanged
        let actions = register_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::Unchanged(path.clone()));
    }

    #[test]
    fn register_overwrites_different_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[mcp_servers.peek]\ncommand = \"old\"\nargs = [\"old\"]\n",
        )
        .unwrap();

        let actions = register_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::Updated(path.clone()));

        let content = std::fs::read_to_string(&path).unwrap();
        let config: toml::Value = content.parse().unwrap();
        assert_eq!(
            config["mcp_servers"]["peek"]["command"],
            toml::Value::String("peek".into())
        );
        assert_eq!(
            config["mcp_servers"]["peek"]["args"],
            toml::Value::Array(vec![toml::Value::String("mcp".into())])
        );
    }

    // --- unregister_at ---

    #[test]
    fn unregister_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            "[mcp_servers.peek]\ncommand = \"peek\"\nargs = [\"mcp\"]\n\n[mcp_servers.other]\ncommand = \"other\"\n",
        )
        .unwrap();

        let actions = unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::Updated(path.clone()));

        let content = std::fs::read_to_string(&path).unwrap();
        let config: toml::Value = content.parse().unwrap();
        assert!(config["mcp_servers"].get("peek").is_none());
        assert_eq!(
            config["mcp_servers"]["other"]["command"],
            toml::Value::String("other".into())
        );
    }

    #[test]
    fn unregister_not_found_when_no_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "[mcp_servers.other]\ncommand = \"other\"\n").unwrap();

        let actions = unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::NotFound(path.clone()));
    }

    #[test]
    fn unregister_not_found_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let actions = unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::NotFound(path.clone()));
        assert!(!path.exists());
    }

    #[test]
    fn register_then_unregister_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");

        // Register
        let actions = register_at(&path).unwrap();
        assert!(matches!(actions[0], FileAction::Created(_)));

        // Unregister
        let actions = unregister_at(&path).unwrap();
        assert!(matches!(actions[0], FileAction::Updated(_)));

        // File still exists but mcp_servers is gone
        let content = std::fs::read_to_string(&path).unwrap();
        let config: toml::Value = content.parse().unwrap();
        assert!(config.get("mcp_servers").is_none());
    }
}
