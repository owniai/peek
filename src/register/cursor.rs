use super::{FileAction, Location, Target, json_register_at, json_unregister_at};
use anyhow::Result;
use std::path::PathBuf;

pub(crate) struct CursorTarget;

impl Target for CursorTarget {
    fn id(&self) -> &str {
        "cursor"
    }
    fn display_name(&self) -> &str {
        "Cursor"
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
        Location::Global => super::home_dir().join(".cursor").join("mcp.json"),
        Location::Local => PathBuf::from(".cursor").join("mcp.json"),
    }
}
