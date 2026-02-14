# Configuration File Encryption

> **Version**: 1.0
> **Component**: CIS Core Configuration
> **Status**: Implemented (v1.1.6)

---

## Overview

CIS configuration encryption provides secure storage of sensitive configuration data (such as API keys, passwords, TLS certificates) using AES-256-GCM encryption with Argon2id key derivation.

## Features

- **AES-256-GCM Encryption**: Industry-standard authenticated encryption
- **Argon2id Key Derivation**: Memory-hard key derivation for resistance to brute-force attacks
- **Automatic Detection**: Transparently detects and decrypts encrypted files
- **Multiple Key Sources**: Support for environment variables, key files, and default locations
- **Secure Defaults**: High-security Argon2 parameters by default

## Architecture

### Encryption Format

Encrypted configuration files use a structured format:

```
[Header JSON (base64)]
[Encrypted Data (base64)]
```

### Header Structure

```json
{
  "magic": "CISENC",
  "version": 1,
  "algorithm": "aes-256-gcm",
  "salt": "<base64-encoded salt>",
  "nonce": "<base64-encoded nonce>"
}
```

### Key Derivation

```
Master Key (32 bytes)
       |
       v
  Argon2id KDF
  (m=65536, t=3, p=4)
       |
       v
  Derived Key (32 bytes)
       |
       v
  AES-256-GCM Cipher
```

## Usage

### Setting Up Encryption

#### Option 1: Environment Variable

```bash
export CIS_CONFIG_ENCRYPTION_KEY="<64-char-hex-key>"
```

#### Option 2: Key File

```bash
# Generate a key
cis-cli config generate-key > ~/.config/cis/encryption.key

# Set key file location
export CIS_CONFIG_ENCRYPTION_KEY_FILE="~/.config/cis/encryption.key"
```

#### Option 3: Default Location

```bash
# Place key at default location (auto-detected)
mkdir -p ~/.config/cis
cp encryption.key ~/.config/cis/encryption.key
```

### Encrypting Configuration

#### Using CIS CLI

```bash
# Encrypt existing config
cis-cli config encrypt config.toml -o config.toml.enc

# Encrypt and replace
cis-cli config encrypt config.toml --in-place
```

#### Programmatic

```rust
use cis_core::config::ConfigEncryption;

let encryption = ConfigEncryption::new()?;
let encrypted = encryption.encrypt_config(toml_content)?;
fs::write("config.toml.enc", encrypted)?;
```

### Decrypting Configuration

Decryption is automatic when loading configuration through `ConfigLoader`. For manual decryption:

#### Using decrypt-config tool

```bash
# Decrypt and print to stdout
decrypt-config config.toml.enc

# Decrypt and save to file
decrypt-config config.toml.enc -o config.toml

# View without saving
decrypt-config config.toml.enc --view
```

#### Programmatic

```rust
use cis_core::config::ConfigEncryption;

let encryption = ConfigEncryption::new()?;
let encrypted_content = fs::read_to_string("config.toml.enc")?;
let decrypted = encryption.decrypt_config(&encrypted_content)?;
```

### Loading Configuration

```rust
use cis_core::config::Config;

// Automatically detects and decrypts if needed
let config = Config::load()?;

// Or from specific path
let config = Config::load_from("config.toml.enc")?;
```

## Key Management

### Generating a New Key

```bash
# Using CIS CLI
cis-cli config generate-key

# Or using openssl
openssl rand -hex 32 > encryption.key
```

### Key Formats

The encryption key can be specified in two formats:

#### Hex Format (64 characters)

```text
0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef
```

#### Base64 Format (44 characters)

```text
ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef0123456789ABCDEFGHIJ=
```

### Key Security Best Practices

1. **Never commit keys to version control**
2. **Use file permissions 0600**: `chmod 600 encryption.key`
3. **Rotate keys regularly**: Every 90 days recommended
4. **Use separate keys for dev/prod**
5. **Backup keys securely**: Encrypted backup with strong password

## Configuration Examples

### Example 1: TLS Certificate Configuration

```toml
# Encrypted config.toml.enc
[network.tls]
enabled = true
cert_path = "/etc/cis/certs/server.pem"
key_path = "/etc/cis/certs/key.pem"

[security]
# Sensitive values are now encrypted
api_keys = ["sk-...", "sk-..."]
```

### Example 2: Database Passwords

```toml
# Encrypted config.toml.enc
[storage.database]
host = "localhost"
port = 5432
username = "cis_user"
password = "encrypted-password-here"
```

## Security Considerations

### Threat Model

**Protected Against**:
- Unauthorized reading of configuration files
- Disk theft
- Backup exposure
- Cloud storage access

**NOT Protected Against**:
- Runtime memory access (key is in memory during use)
- Compromised host system
- Keylogged input

### Security Parameters

| Parameter | Value | Description |
|-----------|-------|-------------|
| Algorithm | AES-256-GCM | Authenticated encryption |
| Key Size | 256 bits | AES-256 |
| Nonce Size | 96 bits | Recommended for GCM |
| KDF | Argon2id | Memory-hard |
| KDF Memory | 64 MB | Argon2 m parameter |
| KDF Iterations | 3 | Argon2 t parameter |
| KDF Parallelism | 4 | Argon2 p parameter |

### Recommendations

1. **Key Storage**
   - Use environment variables in production
   - Use key files for development
   - Never hardcode keys in source

2. **Access Control**
   - File permissions: 0600 (owner read/write only)
   - Directory permissions: 0700 (owner access only)
   - Audit key access regularly

3. **Key Rotation**
   - Generate new key every 90 days
   - Re-encrypt all configs with new key
   - Securely delete old key (shred)

4. **Backup Strategy**
   - Encrypt backups with separate key
   - Store encryption key offline
   - Test restoration regularly

## Troubleshooting

### Common Issues

#### Issue: "No encryption key found"

```
Warning: No encryption key found. Using insecure default key.
```

**Solution**: Set `CIS_CONFIG_ENCRYPTION_KEY` or `CIS_CONFIG_ENCRYPTION_KEY_FILE`.

#### Issue: "Decryption failed"

```
Error: Decryption failed: ...
```

**Possible causes**:
- Wrong key
- Corrupted file
- File tampered with

**Solution**:
1. Verify correct key is set
2. Check file integrity (backup exists?)
3. Ensure file hasn't been modified

#### Issue: "Invalid encryption magic"

```
Error: Invalid encryption magic
```

**Cause**: File is not actually encrypted or corrupted.

**Solution**:
- Check if file is actually plaintext
- Verify backup exists

## API Reference

### ConfigEncryption

```rust
pub struct ConfigEncryption {
    key: [u8; 32],
}

impl ConfigEncryption {
    pub fn new() -> Result<Self>;
    pub fn with_key(key: [u8; 32]) -> Self;
    pub fn encrypt_config(&self, plaintext: &str) -> Result<String>;
    pub fn decrypt_config(&self, content: &str) -> Result<String>;
    pub fn is_encrypted(content: &str) -> bool;
    pub fn generate_key() -> [u8; 32];
    pub fn key_to_hex(key: &[u8; 32]) -> String;
}
```

## Migration Guide

### From Plain Text to Encrypted

1. **Generate encryption key**
   ```bash
   cis-cli config generate-key > encryption.key
   export CIS_CONFIG_ENCRYPTION_KEY_FILE="$(pwd)/encryption.key"
   ```

2. **Encrypt existing configuration**
   ```bash
   cp config.toml config.toml.backup
   cis-cli config encrypt config.toml -o config.toml.enc
   ```

3. **Test decryption**
   ```bash
   decrypt-config config.toml.enc --view | diff - config.toml
   ```

4. **Switch to encrypted file**
   ```bash
   mv config.toml.enc config.toml
   ```

5. **Remove plaintext backup** (after verification)
   ```bash
   shred config.toml.backup
   ```

## Performance

| Operation | Time (approx) | Notes |
|-----------|---------------|-------|
| Key derivation | 50-100ms | One-time per process start |
| Encryption (1MB) | 5-10ms | Scales linearly with size |
| Decryption (1MB) | 5-10ms | Same as encryption |
| File detection | <1ms | Base64 decode only |

## Implementation Details

### File: `cis-core/src/config/encryption.rs`

**Key Components**:
- `ConfigEncryption`: Main encryption/decryption interface
- `EncryptionHeader`: Metadata for encrypted files
- Key derivation using Argon2id
- AES-256-GCM encryption via `aes_gcm` crate

### Dependencies

```toml
[dependencies]
aes-gcm = "0.10"
argon2 = "0.5"
base64 = "0.21"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
dirs = "5.0"
```

## Future Enhancements

- [ ] Support for multiple encryption keys (key rotation)
- [ ] Hardware security module (HSM) integration
- [ ] Cloud KMS integration (AWS KMS, GCP KMS)
- [ ] Per-field encryption (encrypt only sensitive fields)
- [ ] Public key encryption (for sharing encrypted configs)

## References

- [AES-GCM RFC](https://datatracker.ietf.org/doc/html/rfc5116)
- [Argon2 RFC](https://datatracker.ietf.org/doc/html/rfc9106)
- [OWASP Key Management](https://cheatsheetseries.owasp.org/cheatsheets/Key_Management_Cheat_Sheet.html)
