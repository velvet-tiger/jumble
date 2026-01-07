//! Memory storage for AI agents.
//!
//! This module provides persistent JSON-backed storage for AI agents to store
//! and retrieve learned information, preferences, and context over time.

use rustbreak::{deser::Ron, FileDatabase};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A single memory entry with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The stored value.
    pub value: String,
    /// ISO 8601 timestamp when this entry was created or last updated.
    pub timestamp: String,
    /// Optional source identifier (e.g., which agent or tool stored this).
    pub source: Option<String>,
}

/// Memory database type: a simple key-value store.
pub type MemoryDb = HashMap<String, MemoryEntry>;

/// Type alias for the FileDatabase used in memory storage.
pub type MemoryDatabase = FileDatabase<MemoryDb, Ron>;

/// Opens or creates a memory database for a project.
///
/// The database is stored at `<project_root>/.jumble/memory.json`.
/// If the file doesn't exist, it will be created with an empty HashMap.
///
/// # Arguments
/// * `project_root` - The root directory of the project (where `.jumble/` is located).
///
/// # Returns
/// * `Ok(MemoryDatabase)` - Successfully opened or created the database.
/// * `Err(String)` - Failed to open/create the database.
pub fn open_or_create_memory_db(project_root: &Path) -> Result<MemoryDatabase, String> {
    let memory_path = project_root.join(".jumble/memory.ron");

    // Ensure .jumble directory exists
    if let Some(parent) = memory_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create .jumble directory: {}", e))?;
    }

    // Open or create the database
    let db = FileDatabase::<MemoryDb, Ron>::load_from_path_or(memory_path, HashMap::new())
        .map_err(|e| format!("Failed to open memory database: {}", e))?;

    Ok(db)
}

/// Generates an ISO 8601 timestamp for the current time.
pub fn current_timestamp() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_open_or_create_memory_db() {
        let temp_dir = TempDir::new().unwrap();
        let project_root = temp_dir.path().to_path_buf();

        // Create .jumble directory
        fs::create_dir_all(project_root.join(".jumble")).unwrap();

        // Open database (should create it)
        let db = open_or_create_memory_db(&project_root).unwrap();

        // Verify the file was created
        assert!(project_root.join(".jumble/memory.ron").exists());

        // Write some data
        db.write(|db_data| {
            db_data.insert(
                "test_key".to_string(),
                MemoryEntry {
                    value: "test_value".to_string(),
                    timestamp: current_timestamp(),
                    source: Some("test".to_string()),
                },
            );
        })
        .unwrap();

        db.save().unwrap();

        // Open again and verify data persisted
        let db2 = open_or_create_memory_db(&project_root).unwrap();
        db2.read(|db_data| {
            assert_eq!(db_data.len(), 1);
            assert_eq!(db_data.get("test_key").unwrap().value, "test_value");
        })
        .unwrap();
    }

    #[test]
    fn test_timestamp_format() {
        let ts = current_timestamp();
        // Should be a valid RFC 3339 / ISO 8601 timestamp
        assert!(ts.contains('T'));
        assert!(ts.contains('Z') || ts.contains('+'));
    }
}
