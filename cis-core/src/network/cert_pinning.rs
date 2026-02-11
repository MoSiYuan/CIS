//! # Certificate Pinning Module
//!
//! 提供 TLS 证书固定功能，支持首次连接信任（TOFU）和严格模式。
//!
//! ## 功能
//!
//! - 证书指纹计算（SHA-256）
//! - 指纹存储和检索
//! - 首次连接信任（TOFU）
//! - 证书变更检测
//! - 过期检查
//!
//! ## 使用示例
//!
//! ```rust
//! use cis_core::network::cert_pinning::{CertificatePinning, SqlitePinStore, PinningPolicy};
//! use std::sync::Arc;
//!
//! # async fn example() -> anyhow::Result<()> {
//! // 创建 SQLite 存储
//! let store = Arc::new(SqlitePinStore::new("pins.db").await?);
//!
//! // 创建固定管理器，使用 TOFU 策略
//! let pinning = CertificatePinning::new(store)
//!     .with_policy(PinningPolicy::Tofu);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use sha2::{Digest, Sha256};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::error::{CisError, Result};

/// 证书固定管理器
///
/// 管理域名与证书指纹的映射关系，支持多种验证策略。
#[derive(Clone)]
pub struct CertificatePinning {
    store: Arc<dyn PinStore>,
    policy: PinningPolicy,
}

impl fmt::Debug for CertificatePinning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CertificatePinning")
            .field("policy", &self.policy)
            .field("store", &"<dyn PinStore>")
            .finish()
    }
}

/// 固定条目
///
/// 存储单个域名的证书固定信息。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinEntry {
    /// 域名
    pub domain: String,
    /// 证书指纹
    pub fingerprint: Vec<u8>,
    /// 哈希算法
    pub algorithm: HashAlgorithm,
    /// 固定时间
    pub pinned_at: SystemTime,
    /// 过期时间（可选）
    pub expires_at: Option<SystemTime>,
}

/// 哈希算法
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HashAlgorithm {
    /// SHA-256
    Sha256,
    /// SHA-384
    Sha384,
    /// SHA-512
    Sha512,
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HashAlgorithm::Sha256 => write!(f, "SHA-256"),
            HashAlgorithm::Sha384 => write!(f, "SHA-384"),
            HashAlgorithm::Sha512 => write!(f, "SHA-512"),
        }
    }
}

/// 固定策略
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinningPolicy {
    /// 首次连接信任（Trust On First Use）
    ///
    /// 首次连接到域名时自动固定证书，后续连接必须匹配。
    Tofu,
    /// 严格模式
    ///
    /// 必须预先配置固定，不接受新证书。
    Strict,
    /// 禁用固定
    ///
    /// 不进行证书固定验证（不推荐用于生产环境）。
    Disabled,
}

/// 固定验证结果
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinVerification {
    /// 指纹匹配，验证通过
    Valid,
    /// 首次连接（TOFU 模式），需要固定新证书
    NewPin,
    /// 指纹不匹配
    Mismatch {
        /// 期望的指纹
        expected: Vec<u8>,
        /// 实际的指纹
        actual: Vec<u8>,
    },
    /// 固定已过期
    Expired,
}

impl fmt::Display for PinVerification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PinVerification::Valid => write!(f, "Certificate pinning valid"),
            PinVerification::NewPin => write!(f, "New certificate to pin (TOFU)"),
            PinVerification::Mismatch { expected, actual } => {
                write!(
                    f,
                    "Certificate pinning mismatch: expected {}, got {}",
                    hex::encode(expected),
                    hex::encode(actual)
                )
            }
            PinVerification::Expired => write!(f, "Certificate pin has expired"),
        }
    }
}

/// 固定存储 trait
///
/// 定义了存储和检索固定条目的接口。
pub trait PinStore: Send + Sync {
    /// 获取域名的固定条目
    fn get_pin(&self, domain: &str) -> Result<Option<PinEntry>>;

    /// 存储固定条目
    fn store_pin(&self, entry: &PinEntry) -> Result<()>;

    /// 移除域名的固定
    fn remove_pin(&self, domain: &str) -> Result<()>;

    /// 列出所有固定条目
    fn list_pins(&self) -> Result<Vec<PinEntry>> {
        // 默认实现返回空列表
        Ok(vec![])
    }

    /// 清空所有固定
    fn clear_pins(&self) -> Result<()> {
        // 默认实现不做任何事
        Ok(())
    }
}

impl CertificatePinning {
    /// 创建新的证书固定管理器
    ///
    /// # Arguments
    /// * `store` - 固定存储后端
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::network::cert_pinning::{CertificatePinning, MemoryPinStore};
    /// use std::sync::Arc;
    ///
    /// let store = Arc::new(MemoryPinStore::new());
    /// let pinning = CertificatePinning::new(store);
    /// ```
    pub fn new(store: Arc<dyn PinStore>) -> Self {
        Self {
            store,
            policy: PinningPolicy::Tofu,
        }
    }

    /// 设置固定策略
    ///
    /// # Arguments
    /// * `policy` - 固定策略
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::network::cert_pinning::{CertificatePinning, MemoryPinStore, PinningPolicy};
    /// use std::sync::Arc;
    ///
    /// let store = Arc::new(MemoryPinStore::new());
    /// let pinning = CertificatePinning::new(store)
    ///     .with_policy(PinningPolicy::Strict);
    /// ```
    pub fn with_policy(mut self, policy: PinningPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// 获取当前策略
    pub fn policy(&self) -> PinningPolicy {
        self.policy
    }

    /// 验证证书
    ///
    /// 根据当前策略验证域名对应的证书。
    ///
    /// # Arguments
    /// * `domain` - 域名
    /// * `cert_der` - 证书的 DER 编码
    ///
    /// # Returns
    /// * `Ok(PinVerification)` - 验证结果
    /// * `Err(CisError)` - 验证过程中发生错误
    ///
    /// # Examples
    ///
    /// ```rust
    /// use cis_core::network::cert_pinning::{CertificatePinning, MemoryPinStore};
    /// use std::sync::Arc;
    ///
    /// # fn example() -> cis_core::Result<()> {
    /// let store = Arc::new(MemoryPinStore::new());
    /// let pinning = CertificatePinning::new(store);
    ///
    /// let cert_der = b"fake_cert_data";
    /// match pinning.verify("example.com", cert_der)? {
    ///     cis_core::network::cert_pinning::PinVerification::Valid => {
    ///         println!("Certificate is valid");
    ///     }
    ///     _ => {}
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn verify(&self, domain: &str, cert_der: &[u8]) -> Result<PinVerification> {
        if self.policy == PinningPolicy::Disabled {
            debug!("Certificate pinning is disabled");
            return Ok(PinVerification::Valid);
        }

        let fingerprint = compute_fingerprint(cert_der, HashAlgorithm::Sha256);
        let existing = self.store.get_pin(domain)?;

        match existing {
            None => {
                debug!("No existing pin for domain: {}", domain);
                Ok(PinVerification::NewPin)
            }
            Some(entry) => {
                // 检查是否过期
                if let Some(expires_at) = entry.expires_at {
                    let now = SystemTime::now();
                    if now > expires_at {
                        warn!("Pin for {} has expired", domain);
                        return Ok(PinVerification::Expired);
                    }
                }

                // 比较指纹
                if entry.fingerprint == fingerprint {
                    debug!("Pin verification successful for {}", domain);
                    Ok(PinVerification::Valid)
                } else {
                    warn!(
                        "Pin mismatch for {}: expected {}, got {}",
                        domain,
                        hex::encode(&entry.fingerprint),
                        hex::encode(&fingerprint)
                    );
                    Ok(PinVerification::Mismatch {
                        expected: entry.fingerprint,
                        actual: fingerprint,
                    })
                }
            }
        }
    }

    /// 添加固定
    ///
    /// 为域名添加证书固定。
    ///
    /// # Arguments
    /// * `domain` - 域名
    /// * `fingerprint` - 证书指纹
    /// * `algorithm` - 哈希算法
    pub fn pin(&self, domain: &str, fingerprint: &[u8], algorithm: HashAlgorithm) -> Result<()> {
        let entry = PinEntry {
            domain: domain.to_string(),
            fingerprint: fingerprint.to_vec(),
            algorithm,
            pinned_at: SystemTime::now(),
            expires_at: None,
        };

        self.store.store_pin(&entry)?;
        info!("Pinned certificate for {} with {}", domain, algorithm);
        Ok(())
    }

    /// 从证书添加固定
    ///
    /// 自动计算证书指纹并添加固定。
    ///
    /// # Arguments
    /// * `domain` - 域名
    /// * `cert_der` - 证书的 DER 编码
    pub fn pin_certificate(&self, domain: &str, cert_der: &[u8]) -> Result<()> {
        let fingerprint = compute_fingerprint(cert_der, HashAlgorithm::Sha256);
        self.pin(domain, &fingerprint, HashAlgorithm::Sha256)
    }

    /// 移除固定
    ///
    /// 移除域名的证书固定。
    ///
    /// # Arguments
    /// * `domain` - 域名
    pub fn unpin(&self, domain: &str) -> Result<()> {
        self.store.remove_pin(domain)?;
        info!("Removed pin for {}", domain);
        Ok(())
    }

    /// 获取固定条目
    ///
    /// # Arguments
    /// * `domain` - 域名
    pub fn get_pin(&self, domain: &str) -> Result<Option<PinEntry>> {
        self.store.get_pin(domain)
    }

    /// 检查域名是否有固定
    ///
    /// # Arguments
    /// * `domain` - 域名
    pub fn is_pinned(&self, domain: &str) -> Result<bool> {
        Ok(self.store.get_pin(domain)?.is_some())
    }

    /// 处理 TOFU 验证结果
    ///
    /// 如果验证结果是 NewPin，自动添加固定。
    ///
    /// # Arguments
    /// * `domain` - 域名
    /// * `cert_der` - 证书的 DER 编码
    /// * `verification` - 验证结果
    pub fn handle_tofu(
        &self,
        domain: &str,
        cert_der: &[u8],
        verification: PinVerification,
    ) -> Result<PinVerification> {
        match verification {
            PinVerification::NewPin if self.policy == PinningPolicy::Tofu => {
                self.pin_certificate(domain, cert_der)?;
                info!("Auto-pinned certificate for {} (TOFU)", domain);
                Ok(PinVerification::Valid)
            }
            PinVerification::NewPin => {
                // 非 TOFU 模式，返回错误
                Err(CisError::configuration(format!(
                    "No pin found for {} and policy is not TOFU",
                    domain
                )))
            }
            _ => Ok(verification),
        }
    }
}

/// 计算证书指纹
///
/// 使用指定的哈希算法计算证书指纹。
///
/// # Arguments
/// * `cert_der` - 证书的 DER 编码
/// * `algorithm` - 哈希算法
///
/// # Returns
/// * 指纹字节数组
///
/// # Examples
///
/// ```rust
/// use cis_core::network::cert_pinning::{compute_fingerprint, HashAlgorithm};
///
/// let cert_der = b"fake_cert_data";
/// let fingerprint = compute_fingerprint(cert_der, HashAlgorithm::Sha256);
/// assert_eq!(fingerprint.len(), 32); // SHA-256 产生 32 字节
/// ```
pub fn compute_fingerprint(cert_der: &[u8], algorithm: HashAlgorithm) -> Vec<u8> {
    match algorithm {
        HashAlgorithm::Sha256 => {
            let mut hasher = Sha256::new();
            hasher.update(cert_der);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha384 => {
            use sha2::Sha384;
            let mut hasher = Sha384::new();
            hasher.update(cert_der);
            hasher.finalize().to_vec()
        }
        HashAlgorithm::Sha512 => {
            use sha2::Sha512;
            let mut hasher = Sha512::new();
            hasher.update(cert_der);
            hasher.finalize().to_vec()
        }
    }
}

/// 内存固定存储
///
/// 基于内存的固定存储实现，适用于测试和临时使用。
#[derive(Debug)]
pub struct MemoryPinStore {
    pins: Arc<RwLock<HashMap<String, PinEntry>>>,
}

impl MemoryPinStore {
    /// 创建新的内存固定存储
    pub fn new() -> Self {
        Self {
            pins: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for MemoryPinStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PinStore for MemoryPinStore {
    fn get_pin(&self, domain: &str) -> Result<Option<PinEntry>> {
        // 注意：这是一个同步 trait，但内部使用异步锁
        // 在实际实现中，需要使用 block_on 或其他方式
        // 这里为了简化，我们使用 try_lock 的变体
        let pins = self.pins.try_read().map_err(|_| {
            CisError::invalid_state("Failed to acquire read lock on memory pin store")
        })?;
        Ok(pins.get(domain).cloned())
    }

    fn store_pin(&self, entry: &PinEntry) -> Result<()> {
        let mut pins = self.pins.try_write().map_err(|_| {
            CisError::invalid_state("Failed to acquire write lock on memory pin store")
        })?;
        pins.insert(entry.domain.clone(), entry.clone());
        Ok(())
    }

    fn remove_pin(&self, domain: &str) -> Result<()> {
        let mut pins = self.pins.try_write().map_err(|_| {
            CisError::invalid_state("Failed to acquire write lock on memory pin store")
        })?;
        pins.remove(domain);
        Ok(())
    }

    fn list_pins(&self) -> Result<Vec<PinEntry>> {
        let pins = self.pins.try_read().map_err(|_| {
            CisError::invalid_state("Failed to acquire read lock on memory pin store")
        })?;
        Ok(pins.values().cloned().collect())
    }

    fn clear_pins(&self) -> Result<()> {
        let mut pins = self.pins.try_write().map_err(|_| {
            CisError::invalid_state("Failed to acquire write lock on memory pin store")
        })?;
        pins.clear();
        Ok(())
    }
}

/// SQLite 固定存储
///
/// 基于 SQLite 的持久化固定存储实现。
#[derive(Debug)]
pub struct SqlitePinStore {
    conn: Arc<tokio::sync::Mutex<rusqlite::Connection>>,
}

impl SqlitePinStore {
    /// 创建新的 SQLite 固定存储
    ///
    /// # Arguments
    /// * `db_path` - 数据库文件路径
    ///
    /// # Examples
    ///
    /// ```rust,no_run
    /// use cis_core::network::cert_pinning::SqlitePinStore;
    ///
    /// # async fn example() -> anyhow::Result<()> {
    /// let store = SqlitePinStore::new("pins.db").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn new<P: AsRef<Path>>(db_path: P) -> Result<Self> {
        let path = db_path.as_ref().to_path_buf();

        // 在阻塞线程中创建连接
        let conn = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path).map_err(|e| CisError::Database(e))?;

            // 创建表
            conn.execute(
                "CREATE TABLE IF NOT EXISTS certificate_pins (
                    domain TEXT PRIMARY KEY,
                    fingerprint BLOB NOT NULL,
                    algorithm TEXT NOT NULL,
                    pinned_at INTEGER NOT NULL,
                    expires_at INTEGER
                )",
                [],
            )
            .map_err(|e| CisError::Database(e))?;

            Ok::<_, CisError>(conn)
        })
        .await
        .map_err(|e| CisError::internal(format!("Task join error: {}", e)))??;

        Ok(Self {
            conn: Arc::new(tokio::sync::Mutex::new(conn)),
        })
    }

    /// 从算法字符串解析 HashAlgorithm
    fn parse_algorithm(s: &str) -> Result<HashAlgorithm> {
        match s {
            "SHA-256" => Ok(HashAlgorithm::Sha256),
            "SHA-384" => Ok(HashAlgorithm::Sha384),
            "SHA-512" => Ok(HashAlgorithm::Sha512),
            _ => Err(CisError::invalid_input(format!(
                "Unknown hash algorithm: {}",
                s
            ))),
        }
    }

    /// 将 HashAlgorithm 转换为字符串
    fn algorithm_to_string(alg: HashAlgorithm) -> String {
        alg.to_string()
    }
}

impl PinStore for SqlitePinStore {
    fn get_pin(&self, domain: &str) -> Result<Option<PinEntry>> {
        // 由于 PinStore trait 是同步的，我们需要使用 block_on
        // 这在实际使用中可能需要重新设计
        let conn = Arc::clone(&self.conn);
        let domain = domain.to_string();

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let conn = conn.lock().await;
                let entry: Option<PinEntry> = conn
                    .query_row(
                        "SELECT domain, fingerprint, algorithm, pinned_at, expires_at 
                         FROM certificate_pins WHERE domain = ?1",
                        [&domain],
                        |row| {
                            let algorithm_str: String = row.get(2)?;
                            let algorithm = Self::parse_algorithm(&algorithm_str)
                                .unwrap_or(HashAlgorithm::Sha256);

                            let pinned_at_secs: i64 = row.get(3)?;
                            let pinned_at = std::time::UNIX_EPOCH
                                + std::time::Duration::from_secs(pinned_at_secs as u64);

                            let expires_at: Option<i64> = row.get(4)?;
                            let expires_at = expires_at.map(|secs| {
                                std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
                            });

                            Ok(PinEntry {
                                domain: row.get(0)?,
                                fingerprint: row.get(1)?,
                                algorithm,
                                pinned_at,
                                expires_at,
                            })
                        },
                    )
                    .ok();

                Ok(entry)
            })
        })
    }

    fn store_pin(&self, entry: &PinEntry) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        let entry = entry.clone();

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let conn = conn.lock().await;
                let pinned_at = entry
                    .pinned_at
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| CisError::invalid_state("Invalid pinned_at time"))?
                    .as_secs() as i64;

                let expires_at: Option<i64> = entry.expires_at.map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64
                });

                conn.execute(
                    "INSERT OR REPLACE INTO certificate_pins 
                     (domain, fingerprint, algorithm, pinned_at, expires_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    [
                        &entry.domain,
                        &entry.fingerprint as &dyn rusqlite::ToSql,
                        &Self::algorithm_to_string(entry.algorithm),
                        &pinned_at as &dyn rusqlite::ToSql,
                        &expires_at as &dyn rusqlite::ToSql,
                    ],
                )
                .map_err(|e| CisError::Database(e))?;

                Ok(())
            })
        })
    }

    fn remove_pin(&self, domain: &str) -> Result<()> {
        let conn = Arc::clone(&self.conn);
        let domain = domain.to_string();

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let conn = conn.lock().await;
                conn.execute("DELETE FROM certificate_pins WHERE domain = ?", [&domain])
                    .map_err(|e| CisError::Database(e))?;

                Ok(())
            })
        })
    }

    fn list_pins(&self) -> Result<Vec<PinEntry>> {
        let conn = Arc::clone(&self.conn);

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let conn = conn.lock().await;
                let mut stmt = conn
                    .prepare(
                        "SELECT domain, fingerprint, algorithm, pinned_at, expires_at 
                         FROM certificate_pins",
                    )
                    .map_err(|e| CisError::Database(e))?;

                let entries = stmt
                    .query_map([], |row| {
                        let algorithm_str: String = row.get(2)?;
                        let algorithm =
                            Self::parse_algorithm(&algorithm_str).unwrap_or(HashAlgorithm::Sha256);

                        let pinned_at_secs: i64 = row.get(3)?;
                        let pinned_at = std::time::UNIX_EPOCH
                            + std::time::Duration::from_secs(pinned_at_secs as u64);

                        let expires_at: Option<i64> = row.get(4)?;
                        let expires_at = expires_at.map(|secs| {
                            std::time::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
                        });

                        Ok(PinEntry {
                            domain: row.get(0)?,
                            fingerprint: row.get(1)?,
                            algorithm,
                            pinned_at,
                            expires_at,
                        })
                    })
                    .map_err(|e| CisError::Database(e))?
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .map_err(|e| CisError::Database(e))?;

                Ok(entries)
            })
        })
    }

    fn clear_pins(&self) -> Result<()> {
        let conn = Arc::clone(&self.conn);

        tokio::task::block_in_place(|| {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let conn = conn.lock().await;
                conn.execute("DELETE FROM certificate_pins", [])
                    .map_err(|e| CisError::Database(e))?;

                Ok(())
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 测试证书数据（假的）
    fn test_cert_der() -> Vec<u8> {
        b"test_certificate_data_for_pinning".to_vec()
    }

    fn test_cert_der2() -> Vec<u8> {
        b"different_certificate_data".to_vec()
    }

    #[test]
    fn test_compute_fingerprint() {
        let cert = test_cert_der();
        let fingerprint = compute_fingerprint(&cert, HashAlgorithm::Sha256);

        assert_eq!(fingerprint.len(), 32);

        // 相同输入应该产生相同输出
        let fingerprint2 = compute_fingerprint(&cert, HashAlgorithm::Sha256);
        assert_eq!(fingerprint, fingerprint2);

        // 不同输入应该产生不同输出
        let cert2 = test_cert_der2();
        let fingerprint3 = compute_fingerprint(&cert2, HashAlgorithm::Sha256);
        assert_ne!(fingerprint, fingerprint3);
    }

    #[test]
    fn test_memory_pin_store() {
        let store = MemoryPinStore::new();

        // 初始时应该为空
        assert!(store.get_pin("example.com").unwrap().is_none());

        // 添加固定
        let entry = PinEntry {
            domain: "example.com".to_string(),
            fingerprint: vec![1, 2, 3, 4],
            algorithm: HashAlgorithm::Sha256,
            pinned_at: SystemTime::now(),
            expires_at: None,
        };
        store.store_pin(&entry).unwrap();

        // 应该能获取到
        let retrieved = store.get_pin("example.com").unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().fingerprint, vec![1, 2, 3, 4]);

        // 列出所有
        let pins = store.list_pins().unwrap();
        assert_eq!(pins.len(), 1);

        // 移除固定
        store.remove_pin("example.com").unwrap();
        assert!(store.get_pin("example.com").unwrap().is_none());
    }

    #[test]
    fn test_certificate_pinning_verify_valid() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store.clone());

        let cert = test_cert_der();
        let fingerprint = compute_fingerprint(&cert, HashAlgorithm::Sha256);

        // 先添加固定
        pinning
            .pin("example.com", &fingerprint, HashAlgorithm::Sha256)
            .unwrap();

        // 验证应该通过
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::Valid);
    }

    #[test]
    fn test_certificate_pinning_verify_new_pin() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store);

        let cert = test_cert_der();

        // 首次验证应该返回 NewPin
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::NewPin);
    }

    #[test]
    fn test_certificate_pinning_verify_mismatch() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store.clone());

        let cert1 = test_cert_der();
        let cert2 = test_cert_der2();
        let fingerprint1 = compute_fingerprint(&cert1, HashAlgorithm::Sha256);

        // 先添加固定
        pinning
            .pin("example.com", &fingerprint1, HashAlgorithm::Sha256)
            .unwrap();

        // 用不同证书验证应该返回 Mismatch
        let result = pinning.verify("example.com", &cert2).unwrap();
        match result {
            PinVerification::Mismatch { expected, actual } => {
                assert_eq!(expected, fingerprint1);
                assert_eq!(actual, compute_fingerprint(&cert2, HashAlgorithm::Sha256));
            }
            _ => panic!("Expected Mismatch, got {:?}", result),
        }
    }

    #[test]
    fn test_certificate_pinning_tofu() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store.clone()).with_policy(PinningPolicy::Tofu);

        let cert = test_cert_der();

        // 首次验证
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::NewPin);

        // 处理 TOFU
        let result = pinning.handle_tofu("example.com", &cert, result).unwrap();
        assert_eq!(result, PinVerification::Valid);

        // 再次验证应该通过
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::Valid);
    }

    #[test]
    fn test_certificate_pinning_strict_mode() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store).with_policy(PinningPolicy::Strict);

        let cert = test_cert_der();

        // 严格模式下，没有预配置固定应该返回错误
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::NewPin);

        // 尝试处理 TOFU 应该失败（因为是严格模式）
        let result = pinning.handle_tofu("example.com", &cert, result);
        assert!(result.is_err());
    }

    #[test]
    fn test_certificate_pinning_disabled() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store).with_policy(PinningPolicy::Disabled);

        let cert = test_cert_der();

        // 禁用模式下，总是返回 Valid
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::Valid);
    }

    #[test]
    fn test_certificate_pinning_expired() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store.clone());

        let cert = test_cert_der();
        let fingerprint = compute_fingerprint(&cert, HashAlgorithm::Sha256);

        // 添加一个已过期的固定
        let entry = PinEntry {
            domain: "example.com".to_string(),
            fingerprint,
            algorithm: HashAlgorithm::Sha256,
            pinned_at: SystemTime::UNIX_EPOCH,
            expires_at: Some(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1)),
        };
        store.store_pin(&entry).unwrap();

        // 验证应该返回 Expired
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::Expired);
    }

    #[test]
    fn test_certificate_pinning_pin_certificate() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store);

        let cert = test_cert_der();

        // 从证书添加固定
        pinning.pin_certificate("example.com", &cert).unwrap();

        // 验证应该通过
        let result = pinning.verify("example.com", &cert).unwrap();
        assert_eq!(result, PinVerification::Valid);
    }

    #[test]
    fn test_certificate_pinning_unpin() {
        let store = Arc::new(MemoryPinStore::new());
        let pinning = CertificatePinning::new(store);

        let cert = test_cert_der();

        // 添加固定
        pinning.pin_certificate("example.com", &cert).unwrap();
        assert!(pinning.is_pinned("example.com").unwrap());

        // 移除固定
        pinning.unpin("example.com").unwrap();
        assert!(!pinning.is_pinned("example.com").unwrap());
    }

    #[test]
    fn test_pin_verification_display() {
        assert_eq!(
            PinVerification::Valid.to_string(),
            "Certificate pinning valid"
        );
        assert_eq!(
            PinVerification::NewPin.to_string(),
            "New certificate to pin (TOFU)"
        );
        assert_eq!(
            PinVerification::Expired.to_string(),
            "Certificate pin has expired"
        );

        let mismatch = PinVerification::Mismatch {
            expected: vec![0x01, 0x02],
            actual: vec![0x03, 0x04],
        };
        assert!(mismatch.to_string().contains("mismatch"));
    }

    #[test]
    fn test_hash_algorithm_display() {
        assert_eq!(HashAlgorithm::Sha256.to_string(), "SHA-256");
        assert_eq!(HashAlgorithm::Sha384.to_string(), "SHA-384");
        assert_eq!(HashAlgorithm::Sha512.to_string(), "SHA-512");
    }

    #[test]
    fn test_memory_pin_store_list_clear() {
        let store = MemoryPinStore::new();

        // 添加多个固定
        for i in 0..3 {
            let entry = PinEntry {
                domain: format!("example{}.com", i),
                fingerprint: vec![i as u8],
                algorithm: HashAlgorithm::Sha256,
                pinned_at: SystemTime::now(),
                expires_at: None,
            };
            store.store_pin(&entry).unwrap();
        }

        // 列出所有
        let pins = store.list_pins().unwrap();
        assert_eq!(pins.len(), 3);

        // 清空
        store.clear_pins().unwrap();
        let pins = store.list_pins().unwrap();
        assert!(pins.is_empty());
    }
}
