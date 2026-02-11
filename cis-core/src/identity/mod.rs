//! # Identity Module
//!
//! Hardware-bound DID management and cryptographic identity operations.
//!
//! ## Features
//!
//! - DID generation and parsing (format: `did:cis:{node_id}:{pub_key_short}`)
//! - Ed25519 key pair management
//! - Message signing and verification
//! - Deterministic key derivation from seed
//! - Secure key storage

pub mod did;
pub mod ssh_key;

pub use did::DIDManager;
pub use ssh_key::SshKeyEncryption;
