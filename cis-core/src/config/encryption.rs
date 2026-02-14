//! # Configuration File Encryption
//!
//! Provides encryption and decryption for sensitive configuration data.
//!
//! ## Features
//!
//! - AES-256-GCM encryption for configuration files
//! - Key derivation from environment variables or key files
//! - Automatic detection of encrypted configuration files
//! - Secure key handling with Argon2id
//!
//! ## Usage
//!
//! ```rust
//! use cis_core::config::encryption::ConfigEncryption;
//!
//! // Create encryption instance
//! let encryption = ConfigEncryption::new()?;
//!
//! // Encrypt configuration
//! let encrypted = encryption.encrypt_config(toml_content)?;
//!
//! // Decrypt configuration
//! let decrypted = encryption.decrypt_config(&encrypted)?;
//! ```

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, SaltString},
    Argon2, Algorithm, Params, Version,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;

use crate::error::{CisError, Result};

/// Encryption magic bytes to identify encrypted files
const ENCRYPTION_MAGIC: &[u8] = b"CISENC";

/// Nonce size for AES-256-GCM (12 bytes recommended)
const NONCE_SIZE: usize = 12;

/// Default Argon2 parameters (high security)
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_MEMORY_COST: u32 = 65536; // 64 MB
const ARGON2_PARALLELISM: u32 = 4;

/// Environment variable for encryption key
const ENV_ENCRYPTION_KEY: &str = "CIS_CONFIG_ENCRYPTION_KEY";

/// Environment variable for encryption key file
const ENV_ENCRYPTION_KEY_FILE: &str = "CIS_CONFIG_ENCRYPTION_KEY_FILE";

/// Encrypted configuration header
#[derive(Debug, Clone, Serialize, Deserialize)]
struct EncryptionHeader {
    /// Magic bytes for identification
    magic: String,
    /// Version of encryption format
    version: u8,
    /// Encryption algorithm used
    algorithm: String,
    /// Salt for key derivation (base64 encoded)
    salt: String,
    /// Nonce used for encryption (base64 encoded)
    nonce: String,
}

impl Default for EncryptionHeader {
    fn default() -> Self {
        Self {
            magic: String::from_utf8_lossy(ENCRYPTION_MAGIC).to_string(),
            version: 1,
            algorithm: "aes-256-gcm".to_string(),
            salt: String::new(),
            nonce: String::new(),
        }
    }
}

/// Configuration file encryption/decryption
pub struct ConfigEncryption {
    /// Encryption key (32 bytes for AES-256)
    key: [u8; 32],
}

impl ConfigEncryption {
    /// Create a new ConfigEncryption instance
    ///
    /// Attempts to load the encryption key from:
    /// 1. Environment variable `CIS_CONFIG_ENCRYPTION_KEY`
    /// 2. Key file from `CIS_CONFIG_ENCRYPTION_KEY_FILE`
    /// 3. Default key file location
    pub fn new() -> Result<Self> {
        let key = Self::load_encryption_key()?;
        Ok(Self { key })
    }

    /// Create ConfigEncryption with a specific key
    pub fn with_key(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Encrypt configuration content
    ///
    /// ## Format
    ///
    /// The encrypted file has the following structure:
    /// ```text
    /// [Header JSON (base64)]\n
    /// [Encrypted Data (base64)]
    /// ```
    pub fn encrypt_config(&self, plaintext: &str) -> Result<String> {
        // Validate plaintext
        if plaintext.is_empty() {
            return Err(CisError::invalid_input(
                "Cannot encrypt empty configuration",
            ));
        }

        // Check maximum size (100MB)
        if plaintext.len() > 100 * 1024 * 1024 {
            return Err(CisError::invalid_input(
                "Configuration too large (max 100MB)",
            ));
        }

        // Generate random salt and nonce
        let salt = SaltString::generate(&mut OsRng);
        let nonce_bytes = Self::generate_random_nonce()?;

        // Derive encryption key from master key
        let derived_key = Self::derive_key(&self.key, salt.as_bytes())?;

        // Initialize cipher
        let cipher = Aes256Gcm::new(&derived_key.into());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt plaintext
        let plaintext_bytes = plaintext.as_bytes();
        let ciphertext = cipher
            .encrypt(nonce, plaintext_bytes)
            .map_err(|e| CisError::Encryption(format!("Encryption failed: {}", e)))?;

        // Create header
        let header = EncryptionHeader {
            salt: BASE64.encode(salt.as_bytes()),
            nonce: BASE64.encode(&nonce_bytes),
            ..Default::default()
        };

        // Serialize header
        let header_json =
            serde_json::to_string(&header).map_err(|e| {
                CisError::Encryption(format!("Failed to serialize header: {}", e))
            })?;

        // Encode as base64
        let header_b64 = BASE64.encode(header_json.as_bytes());
        let ciphertext_b64 = BASE64.encode(&ciphertext);

        // Combine header and ciphertext
        let encrypted = format!("{}\n{}", header_b64, ciphertext_b64);

        tracing::debug!("Configuration encrypted successfully ({} bytes)", encrypted.len());

        Ok(encrypted)
    }

    /// Decrypt configuration content
    ///
    /// Automatically detects whether the content is encrypted:
    /// - If first line is valid base64 JSON header -> decrypt
    /// - Otherwise, return as-is (plain text)
    pub fn decrypt_config(&self, content: &str) -> Result<String> {
        // Check if content is encrypted
        if !Self::is_encrypted(content) {
            tracing::debug!("Configuration is not encrypted, returning as-is");
            return Ok(content.to_string());
        }

        // Split header and ciphertext
        let mut lines = content.lines();
        let header_b64 = lines.next().ok_or_else(|| {
            CisError::Encryption("Missing encryption header".to_string())
        })?;

        let ciphertext_b64 = lines.next().ok_or_else(|| {
            CisError::Encryption("Missing encrypted data".to_string())
        })?;

        // Decode header
        let header_bytes = BASE64.decode(header_b64).map_err(|e| {
            CisError::Encryption(format!("Failed to decode header: {}", e))
        })?;

        let header_json = String::from_utf8(header_bytes).map_err(|e| {
            CisError::Encryption(format!("Invalid header UTF-8: {}", e))
        })?;

        let header: EncryptionHeader = serde_json::from_str(&header_json)
            .map_err(|e| {
                CisError::Encryption(format!("Failed to parse header: {}", e))
            })?;

        // Validate header
        if !header.magic.starts_with("CIS") {
            return Err(CisError::Encryption(
                "Invalid encryption magic".to_string(),
            ));
        }

        if header.version != 1 {
            return Err(CisError::Encryption(format!(
                "Unsupported encryption version: {}",
                header.version
            )));
        }

        if header.algorithm != "aes-256-gcm" {
            return Err(CisError::Encryption(format!(
                "Unsupported algorithm: {}",
                header.algorithm
            )));
        }

        // Decode salt and nonce
        let salt = BASE64.decode(&header.salt).map_err(|e| {
            CisError::Encryption(format!("Failed to decode salt: {}", e))
        })?;

        let nonce_bytes = BASE64.decode(&header.nonce).map_err(|e| {
            CisError::Encryption(format!("Failed to decode nonce: {}", e))
        })?;

        // Derive encryption key
        let derived_key = Self::derive_key(&self.key, &salt)?;

        // Initialize cipher
        let cipher = Aes256Gcm::new(&derived_key.into());
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Decode ciphertext
        let ciphertext = BASE64.decode(ciphertext_b64).map_err(|e| {
            CisError::Encryption(format!("Failed to decode ciphertext: {}", e))
        })?;

        // Decrypt
        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| CisError::Encryption(format!("Decryption failed: {}", e)))?;

        let plaintext_str = String::from_utf8(plaintext).map_err(|e| {
            CisError::Encryption(format!("Invalid plaintext UTF-8: {}", e))
        })?;

        tracing::debug!("Configuration decrypted successfully ({} bytes)", plaintext_str.len());

        Ok(plaintext_str)
    }

    /// Check if content is encrypted
    pub fn is_encrypted(content: &str) -> bool {
        // Check if first line looks like base64 (and is reasonably long)
        let first_line = match content.lines().next() {
            Some(line) => line,
            None => return false,
        };

        // Basic heuristics for base64
        if first_line.len() < 32 {
            return false;
        }

        // Try to decode as base64 and check if it's valid JSON
        match BASE64.decode(first_line) {
            Ok(bytes) => {
                // Try to parse as JSON header
                if let Ok(header_str) = String::from_utf8(bytes) {
                    if let Ok(_header) = serde_json::from_str::<EncryptionHeader>(&header_str) {
                        return true;
                    }
                }
                false
            }
            Err(_) => false,
        }
    }

    /// Generate a random nonce
    fn generate_random_nonce() -> Result<Vec<u8>> {
        let mut nonce = vec![0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce);
        Ok(nonce)
    }

    /// Derive encryption key using Argon2id
    fn derive_key(master_key: &[u8], salt: &[u8]) -> Result<[u8; 32]> {
        let params = Params::new(ARGON2_MEMORY_COST, ARGON2_TIME_COST, ARGON2_PARALLELISM, None)
            .map_err(|e| CisError::Encryption(format!("Invalid Argon2 params: {}", e)))?;

        let argon2 = Argon2::new(Algorithm::Argon2id, Version::Version13, params);

        let mut derived_key = [0u8; 32];
        argon2
            .hash_password_into(master_key, salt, &mut derived_key)
            .map_err(|e| CisError::Encryption(format!("Key derivation failed: {}", e)))?;

        Ok(derived_key)
    }

    /// Load encryption key from environment or key file
    fn load_encryption_key() -> Result<[u8; 32]> {
        // Try environment variable first
        if let Ok(key_str) = env::var(ENV_ENCRYPTION_KEY) {
            tracing::debug!("Loading encryption key from environment variable");
            return Self::parse_key(&key_str);
        }

        // Try key file from environment
        if let Ok(key_file) = env::var(ENV_ENCRYPTION_KEY_FILE) {
            tracing::debug!("Loading encryption key from file: {}", key_file);
            return Self::load_key_from_file(&key_file);
        }

        // Try default key file locations
        let default_locations = [
            dirs::config_dir()
                .map(|d| d.join("cis").join("encryption.key"))
                .unwrap_or_else(|| PathBuf::from("/etc/cis/encryption.key")),
            PathBuf::from(".cis/encryption.key"),
        ];

        for location in &default_locations {
            if location.exists() {
                tracing::debug!("Loading encryption key from default location: {:?}", location);
                return Self::load_key_from_file(location);
            }
        }

        // No key found - generate a warning but return a default key for testing
        tracing::warn!(
            "No encryption key found. Using insecure default key. Set {} or {}",
            ENV_ENCRYPTION_KEY,
            ENV_ENCRYPTION_KEY_FILE
        );

        // Return an insecure default key (only for development!)
        Ok([0x42u8; 32])
    }

    /// Parse key from string
    ///
    /// Accepts formats:
    /// - Hex string (64 hex characters)
    /// - Base64 string
    fn parse_key(key_str: &str) -> Result<[u8; 32]> {
        let key_str = key_str.trim();

        // Try hex format first
        if key_str.len() == 64 && key_str.chars().all(|c| c.is_ascii_hexdigit()) {
            let mut key = [0u8; 32];
            for (i, chunk) in key_str.as_bytes().chunks(2).enumerate() {
                let byte_str = std::str::from_utf8(chunk).unwrap();
                key[i] = u8::from_str_radix(byte_str, 16).map_err(|_| {
                    CisError::Encryption(format!("Invalid hex key: {}", key_str))
                })?;
            }
            return Ok(key);
        }

        // Try base64 format
        if let Ok(decoded) = BASE64.decode(key_str) {
            if decoded.len() == 32 {
                let mut key = [0u8; 32];
                key.copy_from_slice(&decoded);
                return Ok(key);
            }
        }

        Err(CisError::Encryption(
            "Invalid key format (expected 64 hex chars or base64)".to_string(),
        ))
    }

    /// Load key from file
    ///
    /// File format: plain hex or base64 key
    fn load_key_from_file(path: &PathBuf) -> Result<[u8; 32]> {
        let content = fs::read_to_string(path).map_err(|e| {
            CisError::Encryption(format!("Failed to read key file: {}", e))
        })?;

        Self::parse_key(&content)
    }

    /// Generate a new random encryption key
    pub fn generate_key() -> [u8; 32] {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Export key to hex string
    pub fn key_to_hex(key: &[u8; 32]) -> String {
        key.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0x42u8; 32];
        let encryption = ConfigEncryption::with_key(key);

        let plaintext = r#"
[network]
tcp_port = 6767
bind_address = "127.0.0.1"

[security]
max_request_size = 10485760
"#;

        let encrypted = encryption.encrypt_config(plaintext).unwrap();
        let decrypted = encryption.decrypt_config(&encrypted).unwrap();

        assert_eq!(plaintext.trim(), decrypted.trim());
    }

    #[test]
    fn test_is_encrypted_detection() {
        let key = [0x42u8; 32];
        let encryption = ConfigEncryption::with_key(key);

        let plaintext = "[network]\ntcp_port = 6767";
        let encrypted = encryption.encrypt_config(plaintext).unwrap();

        assert!(ConfigEncryption::is_encrypted(&encrypted));
        assert!(!ConfigEncryption::is_encrypted(plaintext));
    }

    #[test]
    fn test_key_generation() {
        let key1 = ConfigEncryption::generate_key();
        let key2 = ConfigEncryption::generate_key();

        assert_ne!(key1, key2, "Keys should be unique");

        let hex = ConfigEncryption::key_to_hex(&key1);
        assert_eq!(hex.len(), 64);
    }

    #[test]
    fn test_parse_key_hex() {
        let hex_key = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let key = ConfigEncryption::parse_key(hex_key).unwrap();

        assert_eq!(key[0], 0x01);
        assert_eq!(key[1], 0x23);
        assert_eq!(key[15], 0xef);
    }

    #[test]
    fn test_parse_key_base64() {
        // 32 bytes of zeros encoded in base64
        let b64_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
        let key = ConfigEncryption::parse_key(b64_key).unwrap();

        assert_eq!(key, [0u8; 32]);
    }

    #[test]
    fn test_parse_key_invalid() {
        let result = ConfigEncryption::parse_key("invalid-key");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_plaintext() {
        let key = [0x42u8; 32];
        let encryption = ConfigEncryption::with_key(key);

        let plaintext = "[network]\ntcp_port = 6767";
        let result = encryption.decrypt_config(plaintext).unwrap();

        assert_eq!(plaintext, result);
    }

    #[test]
    fn test_encrypt_empty_config() {
        let key = [0x42u8; 32];
        let encryption = ConfigEncryption::with_key(key);

        let result = encryption.encrypt_config("");
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_invalid_header() {
        let key = [0x42u8; 32];
        let encryption = ConfigEncryption::with_key(key);

        let invalid_content = "invalid-base64-content\nmore-invalid-content";
        let result = encryption.decrypt_config(invalid_content);
        assert!(result.is_err());
    }
}
