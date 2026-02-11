//! # Storage Configuration
//!
//! Storage-related configuration including data directories, database settings, and encryption.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::{validation_error, validate_non_empty_path, ValidateConfig};
use crate::error::Result;

/// Default max database connections
pub const DEFAULT_MAX_CONNECTIONS: u32 = 100;

/// Default connection timeout in seconds
pub const DEFAULT_DB_CONNECTION_TIMEOUT_SECS: u64 = 30;

/// Default database busy timeout in milliseconds
pub const DEFAULT_BUSY_TIMEOUT_MS: u64 = 5000;

/// Default WAL checkpoint interval in seconds
pub const DEFAULT_WAL_CHECKPOINT_INTERVAL_SECS: u64 = 300;

/// Default cache size in pages
pub const DEFAULT_CACHE_SIZE_PAGES: i32 = 10000;

/// Storage configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// Data directory for CIS storage
    #[serde(default = "default_data_dir")]
    pub data_dir: PathBuf,

    /// Backup directory
    #[serde(default = "default_backup_dir")]
    pub backup_dir: PathBuf,

    /// Temp directory for intermediate files
    #[serde(default = "default_temp_dir")]
    pub temp_dir: PathBuf,

    /// Maximum database connections in pool
    #[serde(default = "default_max_connections")]
    pub max_connections: u32,

    /// Database connection timeout
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: std::time::Duration,

    /// Database busy timeout (how long to wait when database is locked)
    #[serde(default = "default_busy_timeout")]
    pub busy_timeout: std::time::Duration,

    /// WAL checkpoint interval
    #[serde(default = "default_wal_checkpoint_interval")]
    pub wal_checkpoint_interval: std::time::Duration,

    /// SQLite cache size in pages
    #[serde(default = "default_cache_size")]
    pub cache_size_pages: i32,

    /// Enable WAL mode
    #[serde(default = "default_wal_enabled")]
    pub wal_enabled: bool,

    /// Enable foreign keys
    #[serde(default = "default_foreign_keys_enabled")]
    pub foreign_keys_enabled: bool,

    /// Enable synchronous mode (NORMAL, FULL, OFF)
    #[serde(default = "default_synchronous")]
    pub synchronous: String,

    /// Journal mode (WAL, DELETE, TRUNCATE, PERSIST, MEMORY, OFF)
    #[serde(default = "default_journal_mode")]
    pub journal_mode: String,

    /// Database-specific configurations
    #[serde(default)]
    pub databases: DatabaseConfig,

    /// Encryption configuration
    #[serde(default)]
    pub encryption: EncryptionConfig,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            backup_dir: default_backup_dir(),
            temp_dir: default_temp_dir(),
            max_connections: default_max_connections(),
            connection_timeout: default_connection_timeout(),
            busy_timeout: default_busy_timeout(),
            wal_checkpoint_interval: default_wal_checkpoint_interval(),
            cache_size_pages: default_cache_size(),
            wal_enabled: default_wal_enabled(),
            foreign_keys_enabled: default_foreign_keys_enabled(),
            synchronous: default_synchronous(),
            journal_mode: default_journal_mode(),
            databases: DatabaseConfig::default(),
            encryption: EncryptionConfig::default(),
        }
    }
}

impl ValidateConfig for StorageConfig {
    fn validate(&self) -> Result<()> {
        // Validate data directory is not empty
        validate_non_empty_path(&self.data_dir, "data_dir")?;

        // Validate backup directory is not empty
        validate_non_empty_path(&self.backup_dir, "backup_dir")?;

        // Validate temp directory is not empty
        validate_non_empty_path(&self.temp_dir, "temp_dir")?;

        // Validate max connections
        if self.max_connections == 0 {
            return Err(validation_error("max_connections cannot be zero"));
        }
        if self.max_connections > 1000 {
            return Err(validation_error(
                "max_connections cannot exceed 1000",
            ));
        }

        // Validate timeouts are not zero
        if self.connection_timeout.is_zero() {
            return Err(validation_error("connection_timeout cannot be zero"));
        }
        if self.busy_timeout.is_zero() {
            return Err(validation_error("busy_timeout cannot be zero"));
        }
        if self.wal_checkpoint_interval.is_zero() {
            return Err(validation_error(
                "wal_checkpoint_interval cannot be zero",
            ));
        }

        // Validate synchronous mode
        let valid_synchronous = ["OFF", "NORMAL", "FULL", "EXTRA"];
        if !valid_synchronous.contains(&self.synchronous.to_uppercase().as_str()) {
            return Err(validation_error(format!(
                "Invalid synchronous mode: {}. Must be one of: {:?}",
                self.synchronous, valid_synchronous
            )));
        }

        // Validate journal mode
        let valid_journal_modes = ["DELETE", "TRUNCATE", "PERSIST", "MEMORY", "WAL", "OFF"];
        if !valid_journal_modes.contains(&self.journal_mode.to_uppercase().as_str()) {
            return Err(validation_error(format!(
                "Invalid journal mode: {}. Must be one of: {:?}",
                self.journal_mode, valid_journal_modes
            )));
        }

        // Validate database configs
        self.databases.validate()?;
        self.encryption.validate()?;

        Ok(())
    }
}

/// Database-specific configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DatabaseConfig {
    /// Core database file name
    #[serde(default = "default_core_db_name")]
    pub core_db_name: String,

    /// Skill database file name
    #[serde(default = "default_skill_db_name")]
    pub skill_db_name: String,

    /// Federation database file name
    #[serde(default = "default_federation_db_name")]
    pub federation_db_name: String,

    /// Memory/Vector database file name
    #[serde(default = "default_memory_db_name")]
    pub memory_db_name: String,

    /// Conversation database file name
    #[serde(default = "default_conversation_db_name")]
    pub conversation_db_name: String,

    /// Task database file name
    #[serde(default = "default_task_db_name")]
    pub task_db_name: String,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            core_db_name: default_core_db_name(),
            skill_db_name: default_skill_db_name(),
            federation_db_name: default_federation_db_name(),
            memory_db_name: default_memory_db_name(),
            conversation_db_name: default_conversation_db_name(),
            task_db_name: default_task_db_name(),
        }
    }
}

impl ValidateConfig for DatabaseConfig {
    fn validate(&self) -> Result<()> {
        // Validate database names are not empty
        for (name, value) in [
            ("core_db_name", &self.core_db_name),
            ("skill_db_name", &self.skill_db_name),
            ("federation_db_name", &self.federation_db_name),
            ("memory_db_name", &self.memory_db_name),
            ("conversation_db_name", &self.conversation_db_name),
            ("task_db_name", &self.task_db_name),
        ] {
            if value.is_empty() {
                return Err(validation_error(format!("{} cannot be empty", name)));
            }
            if !value.ends_with(".db") && !value.ends_with(".sqlite") && !value.ends_with(".sqlite3") {
                // Warning: not a standard extension, but we'll allow it
            }
        }

        Ok(())
    }
}

/// Encryption configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EncryptionConfig {
    /// Enable database encryption
    #[serde(default)]
    pub enabled: bool,

    /// Key derivation algorithm (argon2id, pbkdf2)
    #[serde(default = "default_key_derivation")]
    pub key_derivation: String,

    /// Memory cost for Argon2 (in KB)
    #[serde(default = "default_argon2_memory")]
    pub argon2_memory: u32,

    /// Time cost for Argon2 (iterations)
    #[serde(default = "default_argon2_iterations")]
    pub argon2_iterations: u32,

    /// Parallelism for Argon2
    #[serde(default = "default_argon2_parallelism")]
    pub argon2_parallelism: u32,

    /// Salt length in bytes
    #[serde(default = "default_salt_length")]
    pub salt_length: usize,

    /// Key length in bytes
    #[serde(default = "default_key_length")]
    pub key_length: usize,
}

impl Default for EncryptionConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            key_derivation: default_key_derivation(),
            argon2_memory: default_argon2_memory(),
            argon2_iterations: default_argon2_iterations(),
            argon2_parallelism: default_argon2_parallelism(),
            salt_length: default_salt_length(),
            key_length: default_key_length(),
        }
    }
}

impl ValidateConfig for EncryptionConfig {
    fn validate(&self) -> Result<()> {
        if self.enabled {
            // Validate key derivation algorithm
            let valid_algorithms = ["argon2id", "argon2d", "argon2i", "pbkdf2"];
            if !valid_algorithms.contains(&self.key_derivation.to_lowercase().as_str()) {
                return Err(validation_error(format!(
                    "Invalid key derivation: {}. Must be one of: {:?}",
                    self.key_derivation, valid_algorithms
                )));
            }

            // Validate Argon2 parameters
            if self.key_derivation.to_lowercase().starts_with("argon2") {
                if self.argon2_memory < 1024 {
                    return Err(validation_error(
                        "argon2_memory must be at least 1024 KB",
                    ));
                }
                if self.argon2_iterations < 1 {
                    return Err(validation_error(
                        "argon2_iterations must be at least 1",
                    ));
                }
                if self.argon2_parallelism < 1 {
                    return Err(validation_error(
                        "argon2_parallelism must be at least 1",
                    ));
                }
            }

            // Validate salt length
            if self.salt_length < 16 {
                return Err(validation_error("salt_length must be at least 16 bytes"));
            }

            // Validate key length
            if self.key_length != 16 && self.key_length != 24 && self.key_length != 32 {
                return Err(validation_error(
                    "key_length must be 16, 24, or 32 bytes (for AES-128/192/256)",
                ));
            }
        }

        Ok(())
    }
}

// Default value functions
fn default_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/var/lib"))
        .join("cis")
}

fn default_backup_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("/var/lib"))
        .join("cis")
        .join("backups")
}

fn default_temp_dir() -> PathBuf {
    std::env::temp_dir().join("cis")
}

fn default_max_connections() -> u32 {
    DEFAULT_MAX_CONNECTIONS
}

fn default_connection_timeout() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_DB_CONNECTION_TIMEOUT_SECS)
}

fn default_busy_timeout() -> std::time::Duration {
    std::time::Duration::from_millis(DEFAULT_BUSY_TIMEOUT_MS)
}

fn default_wal_checkpoint_interval() -> std::time::Duration {
    std::time::Duration::from_secs(DEFAULT_WAL_CHECKPOINT_INTERVAL_SECS)
}

fn default_cache_size() -> i32 {
    DEFAULT_CACHE_SIZE_PAGES
}

fn default_wal_enabled() -> bool {
    true
}

fn default_foreign_keys_enabled() -> bool {
    true
}

fn default_synchronous() -> String {
    "NORMAL".to_string()
}

fn default_journal_mode() -> String {
    "WAL".to_string()
}

fn default_core_db_name() -> String {
    "core.db".to_string()
}

fn default_skill_db_name() -> String {
    "skill.db".to_string()
}

fn default_federation_db_name() -> String {
    "federation.db".to_string()
}

fn default_memory_db_name() -> String {
    "memory.db".to_string()
}

fn default_conversation_db_name() -> String {
    "conversation.db".to_string()
}

fn default_task_db_name() -> String {
    "task.db".to_string()
}

fn default_key_derivation() -> String {
    "argon2id".to_string()
}

fn default_argon2_memory() -> u32 {
    65536 // 64 MB
}

fn default_argon2_iterations() -> u32 {
    3
}

fn default_argon2_parallelism() -> u32 {
    4
}

fn default_salt_length() -> usize {
    32
}

fn default_key_length() -> usize {
    32 // 256 bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_storage_config_default() {
        let config = StorageConfig::default();
        assert!(config.data_dir.to_string_lossy().contains("cis"));
        assert!(config.backup_dir.to_string_lossy().contains("backups"));
        assert_eq!(config.max_connections, 100);
        assert!(config.wal_enabled);
        assert!(config.foreign_keys_enabled);
        assert_eq!(config.synchronous, "NORMAL");
        assert_eq!(config.journal_mode, "WAL");
    }

    #[test]
    fn test_storage_config_validate_success() {
        let config = StorageConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_storage_config_validate_zero_max_connections() {
        let mut config = StorageConfig::default();
        config.max_connections = 0;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_connections"));
    }

    #[test]
    fn test_storage_config_validate_max_connections_too_high() {
        let mut config = StorageConfig::default();
        config.max_connections = 1001;
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("max_connections"));
    }

    #[test]
    fn test_storage_config_validate_invalid_synchronous() {
        let mut config = StorageConfig::default();
        config.synchronous = "INVALID".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("synchronous"));
    }

    #[test]
    fn test_storage_config_validate_invalid_journal_mode() {
        let mut config = StorageConfig::default();
        config.journal_mode = "INVALID".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("journal mode"));
    }

    #[test]
    fn test_database_config_default() {
        let config = DatabaseConfig::default();
        assert_eq!(config.core_db_name, "core.db");
        assert_eq!(config.skill_db_name, "skill.db");
        assert_eq!(config.federation_db_name, "federation.db");
        assert_eq!(config.memory_db_name, "memory.db");
        assert_eq!(config.conversation_db_name, "conversation.db");
        assert_eq!(config.task_db_name, "task.db");
    }

    #[test]
    fn test_database_config_validate_empty_name() {
        let mut config = DatabaseConfig::default();
        config.core_db_name = String::new();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("core_db_name"));
    }

    #[test]
    fn test_encryption_config_default() {
        let config = EncryptionConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.key_derivation, "argon2id");
        assert_eq!(config.argon2_memory, 65536);
        assert_eq!(config.argon2_iterations, 3);
        assert_eq!(config.argon2_parallelism, 4);
        assert_eq!(config.salt_length, 32);
        assert_eq!(config.key_length, 32);
    }

    #[test]
    fn test_encryption_config_validate_disabled() {
        let config = EncryptionConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_encryption_config_validate_enabled_valid() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_encryption_config_validate_invalid_algorithm() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        config.key_derivation = "invalid".to_string();
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key derivation"));
    }

    #[test]
    fn test_encryption_config_validate_low_memory() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        config.argon2_memory = 512; // Too low
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("argon2_memory"));
    }

    #[test]
    fn test_encryption_config_validate_invalid_key_length() {
        let mut config = EncryptionConfig::default();
        config.enabled = true;
        config.key_length = 20; // Not 16, 24, or 32
        
        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("key_length"));
    }

    #[test]
    fn test_encryption_config_validate_valid_key_lengths() {
        for key_length in [16, 24, 32] {
            let mut config = EncryptionConfig::default();
            config.enabled = true;
            config.key_length = key_length;
            
            assert!(config.validate().is_ok(), "Key length {} should be valid", key_length);
        }
    }

    #[test]
    fn test_storage_config_serialize() {
        let config = StorageConfig::default();
        let toml = toml::to_string(&config).unwrap();
        assert!(toml.contains("data_dir"));
        assert!(toml.contains("max_connections"));
    }

    #[test]
    fn test_storage_config_deserialize() {
        let toml = r#"
            max_connections = 50
            synchronous = "FULL"
            journal_mode = "DELETE"
        "#;
        let config: StorageConfig = toml::from_str(toml).unwrap();
        assert_eq!(config.max_connections, 50);
        assert_eq!(config.synchronous, "FULL");
        assert_eq!(config.journal_mode, "DELETE");
    }
}
