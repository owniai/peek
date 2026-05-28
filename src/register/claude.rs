use super::{FileAction, Location, Target, json_register_at, json_unregister_at};
use anyhow::Result;
use std::path::PathBuf;

pub(crate) struct ClaudeTarget;

impl Target for ClaudeTarget {
    fn id(&self) -> &str {
        "claude"
    }
    fn display_name(&self) -> &str {
        "Claude Code"
    }
    fn supports_local(&self) -> bool {
        true
    }

    fn register(&self, location: Location) -> Result<Vec<FileAction>> {
        let path = config_path(location);
        let entry = super::mcp_entry();
        json_register_at(&path, entry)
    }

    fn unregister(&self, location: Location) -> Result<Vec<FileAction>> {
        let path = config_path(location);
        json_unregister_at(&path)
    }

    fn config_paths(&self, location: Location) -> Vec<PathBuf> {
        vec![config_path(location)]
    }
}

fn config_path(location: Location) -> PathBuf {
    match location {
        Location::Global => super::home_dir().join(".claude.json"),
        Location::Local => PathBuf::from(".mcp.json"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- register_at ---

    #[test]
    fn register_creates_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        let actions = json_register_at(&path, super::super::mcp_entry()).unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0], FileAction::Created(path.clone()));

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["peek"]["command"], "peek");
        assert_eq!(config["mcpServers"]["peek"]["args"], json!(["mcp"]));
    }

    #[test]
    fn register_preserves_other_servers() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        std::fs::write(&path, r#"{"mcpServers": {"other": {"command": "other"}}}"#).unwrap();

        json_register_at(&path, super::super::mcp_entry()).unwrap();

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["other"]["command"], "other");
        assert_eq!(config["mcpServers"]["peek"]["command"], "peek");
    }

    #[test]
    fn register_idempotent_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        let entry = super::super::mcp_entry();

        let actions = json_register_at(&path, entry.clone()).unwrap();
        assert_eq!(actions[0], FileAction::Created(path.clone()));

        let actions = json_register_at(&path, entry).unwrap();
        assert_eq!(actions[0], FileAction::Unchanged(path.clone()));
    }

    #[test]
    fn register_overwrites_different_value() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"peek": {"command": "old", "args": ["old"]}}}"#,
        )
        .unwrap();

        let actions = json_register_at(&path, super::super::mcp_entry()).unwrap();
        assert_eq!(actions[0], FileAction::Updated(path.clone()));

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(config["mcpServers"]["peek"]["command"], "peek");
    }

    // --- unregister_at ---

    #[test]
    fn unregister_removes_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        std::fs::write(
            &path,
            r#"{"mcpServers": {"peek": {"command": "peek"}, "other": {"command": "other"}}}"#,
        )
        .unwrap();

        let actions = json_unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::Updated(path.clone()));

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config["mcpServers"].get("peek").is_none());
        assert_eq!(config["mcpServers"]["other"]["command"], "other");
    }

    #[test]
    fn unregister_not_found_when_no_entry() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");
        std::fs::write(&path, r#"{"mcpServers": {"other": {"command": "other"}}}"#).unwrap();

        let actions = json_unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::NotFound(path.clone()));
    }

    #[test]
    fn unregister_not_found_when_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");

        let actions = json_unregister_at(&path).unwrap();
        assert_eq!(actions[0], FileAction::NotFound(path.clone()));
        assert!(!path.exists());
    }

    #[test]
    fn register_then_unregister_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".claude.json");

        let actions = json_register_at(&path, super::super::mcp_entry()).unwrap();
        assert!(matches!(actions[0], FileAction::Created(_)));

        let actions = json_unregister_at(&path).unwrap();
        assert!(matches!(actions[0], FileAction::Updated(_)));

        let config: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config.get("mcpServers").is_none());
    }
}
