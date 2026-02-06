//! # Room Store Manager
//!
//! Manages multiple RoomStore instances - one per room.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::error::{CisError, Result};
use crate::matrix::nucleus::RoomId;

use super::room_store::RoomStore;
use super::room_types::{RoomInfo, RoomMetadata, RoomStats, RoomStoreConfig};

/// Manages room stores
pub struct RoomStoreManager {
    /// Room ID -> RoomStore mapping
    rooms: RwLock<HashMap<RoomId, Arc<RoomStore>>>,
    /// Base storage path
    base_path: PathBuf,
    /// Configuration
    config: RoomStoreConfig,
}

impl std::fmt::Debug for RoomStoreManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RoomStoreManager")
            .field("base_path", &self.base_path)
            .field("config", &self.config)
            .finish()
    }
}

impl RoomStoreManager {
    /// Create new room store manager with default config
    pub fn new(base_path: impl AsRef<Path>) -> Self {
        Self::with_config(base_path, RoomStoreConfig::default())
    }

    /// Create with custom config
    pub fn with_config(base_path: impl AsRef<Path>, config: RoomStoreConfig) -> Self {
        Self {
            rooms: RwLock::new(HashMap::new()),
            base_path: base_path.as_ref().to_path_buf(),
            config,
        }
    }

    /// Create with default path in CIS data directory
    pub fn default_manager() -> Self {
        let base_path = crate::storage::Paths::data_dir().join("rooms");
        Self::new(base_path)
    }

    /// Get or create room store
    pub async fn get_or_create(&self, room_id: &RoomId) -> Result<Arc<RoomStore>> {
        // Try read first
        {
            let rooms = self.rooms.read().await;
            if let Some(store) = rooms.get(room_id) {
                return Ok(store.clone());
            }
        }

        // Need to create
        let mut rooms = self.rooms.write().await;

        // Double-check
        if let Some(store) = rooms.get(room_id) {
            return Ok(store.clone());
        }

        // Create room directory
        let room_path = self.room_path(room_id);
        tokio::fs::create_dir_all(&room_path).await.map_err(|e| {
            CisError::storage(format!("Failed to create room dir for {}: {}", room_id, e))
        })?;

        // Create store
        let db_path = room_path.join("events.db");
        let store = RoomStore::open(room_id.clone(), db_path).await?;
        let store = Arc::new(store);

        rooms.insert(room_id.clone(), store.clone());
        info!("Created RoomStore for {}", room_id);

        Ok(store)
    }

    /// Get existing room store (returns None if not loaded)
    pub async fn get(&self, room_id: &RoomId) -> Option<Arc<RoomStore>> {
        let rooms = self.rooms.read().await;
        rooms.get(room_id).cloned()
    }

    /// Check if room exists (even if not loaded)
    pub async fn room_exists(&self, room_id: &RoomId) -> bool {
        // Check memory
        if self.get(room_id).await.is_some() {
            return true;
        }

        // Check disk
        let room_path = self.room_path(room_id);
        let db_path = room_path.join("events.db");
        tokio::fs::try_exists(db_path).await.unwrap_or(false)
    }

    /// Close room store and remove from memory
    pub async fn close_room(&self, room_id: &RoomId) -> Result<()> {
        let mut rooms = self.rooms.write().await;

        if let Some(store) = rooms.remove(room_id) {
            store.close().await?;
            info!("Closed RoomStore for {}", room_id);
        }

        Ok(())
    }

    /// List all active (loaded) rooms
    pub async fn list_active_rooms(&self) -> Vec<RoomId> {
        let rooms = self.rooms.read().await;
        rooms.keys().cloned().collect()
    }

    /// List all rooms on disk
    pub async fn list_all_rooms(&self) -> Result<Vec<RoomInfo>> {
        let mut rooms = Vec::new();

        let mut entries = tokio::fs::read_dir(&self.base_path).await.map_err(|e| {
            CisError::storage(format!("Failed to read rooms directory: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            CisError::storage(format!("Failed to read directory entry: {}", e))
        })? {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            // Check if it's a room directory (has events.db)
            let db_path = path.join("events.db");
            if !tokio::fs::try_exists(&db_path).await.unwrap_or(false) {
                continue;
            }

            // Get room ID from directory name
            let room_id_str = path.file_name().and_then(|n| n.to_str());
            if let Some(room_id_str) = room_id_str {
                // Decode room ID (replace : with special sequence if needed)
                let room_id = decode_room_id(room_id_str);

                // Get stats
                let stats = if let Ok(store) = self.get_or_create(&room_id).await {
                    store.get_stats().await.ok()
                } else {
                    None
                };

                // Load metadata
                let metadata = self.load_room_metadata(&room_id).await.ok();

                rooms.push(RoomInfo {
                    room_id: room_id.to_string(),
                    metadata: metadata.unwrap_or_else(|| RoomMetadata {
                        room_id: room_id.to_string(),
                        name: None,
                        topic: None,
                        creator: "unknown".to_string(),
                        created_at: 0,
                        last_activity: stats.as_ref().and_then(|s| s.latest_event).unwrap_or(0),
                        is_archived: false,
                        federate: false,
                    }),
                    stats: stats.unwrap_or_else(|| RoomStats {
                        total_events: 0,
                        earliest_event: None,
                        latest_event: None,
                        event_type_counts: HashMap::new(),
                        db_size_bytes: 0,
                        created_at: None,
                    }),
                });
            }
        }

        Ok(rooms)
    }

    /// Archive room (close and move to archive directory)
    pub async fn archive_room(&self, room_id: &RoomId) -> Result<()> {
        // Close if open
        self.close_room(room_id).await?;

        // Move to archive
        let room_path = self.room_path(room_id);
        let archive_dir = self.base_path.join("archived");
        let archive_path = archive_dir.join(encode_room_id(room_id));

        tokio::fs::create_dir_all(&archive_dir).await.map_err(|e| {
            CisError::storage(format!("Failed to create archive dir: {}", e))
        })?;

        tokio::fs::rename(&room_path, &archive_path).await.map_err(|e| {
            CisError::storage(format!("Failed to archive room {}: {}", room_id, e))
        })?;

        info!("Archived room {} to {:?}", room_id, archive_path);
        Ok(())
    }

    /// Delete room permanently
    pub async fn delete_room(&self, room_id: &RoomId) -> Result<()> {
        // Close if open
        self.close_room(room_id).await?;

        // Delete directory
        let room_path = self.room_path(room_id);
        tokio::fs::remove_dir_all(&room_path).await.map_err(|e| {
            CisError::storage(format!("Failed to delete room {}: {}", room_id, e))
        })?;

        info!("Deleted room {}", room_id);
        Ok(())
    }

    /// Get room path
    fn room_path(&self, room_id: &RoomId) -> PathBuf {
        let encoded = encode_room_id(room_id);
        self.base_path.join(encoded)
    }

    /// Load room metadata
    async fn load_room_metadata(&self, room_id: &RoomId) -> Result<RoomMetadata> {
        let room_path = self.room_path(room_id);
        let meta_path = room_path.join("meta.json");

        let content = tokio::fs::read_to_string(&meta_path).await.map_err(|e| {
            CisError::storage(format!("Failed to read room metadata: {}", e))
        })?;

        let metadata: RoomMetadata = serde_json::from_str(&content).map_err(|e| {
            CisError::serialization(format!("Failed to parse room metadata: {}", e))
        })?;

        Ok(metadata)
    }

    /// Save room metadata
    pub async fn save_room_metadata(&self, metadata: &RoomMetadata) -> Result<()> {
        let room_id = RoomId::new(&metadata.room_id);
        let room_path = self.room_path(&room_id);

        // Ensure directory exists
        tokio::fs::create_dir_all(&room_path).await.map_err(|e| {
            CisError::storage(format!("Failed to create room dir: {}", e))
        })?;

        let meta_path = room_path.join("meta.json");
        let content = serde_json::to_string_pretty(metadata).map_err(|e| {
            CisError::serialization(format!("Failed to serialize metadata: {}", e))
        })?;

        tokio::fs::write(&meta_path, content).await.map_err(|e| {
            CisError::storage(format!("Failed to write room metadata: {}", e))
        })?;

        Ok(())
    }

    /// Close all rooms
    pub async fn close_all(&self) -> Result<()> {
        let mut rooms = self.rooms.write().await;

        for (room_id, store) in rooms.drain() {
            if let Err(e) = store.close().await {
                warn!("Failed to close room {}: {}", room_id, e);
            }
        }

        info!("Closed all room stores");
        Ok(())
    }

    /// Get total room count (loaded)
    pub async fn loaded_room_count(&self) -> usize {
        let rooms = self.rooms.read().await;
        rooms.len()
    }

    /// Get total memory usage estimate
    pub async fn memory_usage_estimate(&self) -> usize {
        // Rough estimate: each loaded room ~1MB
        self.loaded_room_count().await * 1024 * 1024
    }
}

/// Encode room ID for filesystem (replace special chars)
fn encode_room_id(room_id: &RoomId) -> String {
    room_id
        .as_str()
        .replace('!', "_")
        .replace(':', "_")
        .replace('/', "_")
        .replace('\\', "_")
}

/// Decode room ID from filesystem
fn decode_room_id(encoded: &str) -> RoomId {
    // This is a simplified version - might need adjustment based on actual encoding
    let decoded = encoded.replace('_', ":");
    RoomId::new(&decoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_manager_create_and_get() {
        let temp = TempDir::new().unwrap();
        let manager = RoomStoreManager::new(temp.path());

        let room_id = RoomId::new("!test:cis.local");

        // Get or create
        let store1 = manager.get_or_create(&room_id).await.unwrap();
        let store2 = manager.get_or_create(&room_id).await.unwrap();

        // Should be same instance
        assert!(Arc::ptr_eq(&store1, &store2));

        // Should be listed
        let active = manager.list_active_rooms().await;
        assert_eq!(active.len(), 1);
    }

    #[tokio::test]
    async fn test_manager_multiple_rooms() {
        let temp = TempDir::new().unwrap();
        let manager = RoomStoreManager::new(temp.path());

        let room1 = RoomId::new("!room1:cis.local");
        let room2 = RoomId::new("!room2:cis.local");

        manager.get_or_create(&room1).await.unwrap();
        manager.get_or_create(&room2).await.unwrap();

        let active = manager.list_active_rooms().await;
        assert_eq!(active.len(), 2);
    }

    #[tokio::test]
    async fn test_close_and_reopen() {
        let temp = TempDir::new().unwrap();
        let manager = RoomStoreManager::new(temp.path());

        let room_id = RoomId::new("!test:cis.local");

        // Create
        let store1 = manager.get_or_create(&room_id).await.unwrap();
        let ptr1 = Arc::as_ptr(&store1);

        // Close
        manager.close_room(&room_id).await.unwrap();

        // Reopen - should be new instance
        let store2 = manager.get_or_create(&room_id).await.unwrap();
        let ptr2 = Arc::as_ptr(&store2);

        assert_ne!(ptr1, ptr2);
    }

    #[tokio::test]
    async fn test_archive_room() {
        let temp = TempDir::new().unwrap();
        let manager = RoomStoreManager::new(temp.path());

        let room_id = RoomId::new("!test:cis.local");

        // Create
        manager.get_or_create(&room_id).await.unwrap();

        // Archive
        manager.archive_room(&room_id).await.unwrap();

        // Should not exist anymore
        assert!(!manager.room_exists(&room_id).await);

        // Check archive directory
        let archive_path = temp.path().join("archived").join(encode_room_id(&room_id));
        assert!(tokio::fs::try_exists(archive_path).await.unwrap());
    }

    #[test]
    fn test_encode_decode_room_id() {
        let room_id = RoomId::new("!test:cis.local");
        let encoded = encode_room_id(&room_id);
        assert!(!encoded.contains('!'));
        assert!(!encoded.contains(':'));
    }
}
