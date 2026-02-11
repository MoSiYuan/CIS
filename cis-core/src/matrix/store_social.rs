//! # Matrix Social Store
//!
//! 独立的 SQLite 存储，用于管理人类用户相关的社交数据。
//!
//! ## 设计目标
//!
//! - **分离关注点**: 将人类用户数据（用户、设备、令牌）与协议事件数据分离
//! - **可独立备份**: 用户社交数据可以独立备份/恢复而不影响事件日志
//! - **Skill 化基础**: 支持通过 Skill 扩展注册逻辑（邀请码、付费等）
//! - **可卸载**: 可以禁用 Matrix 人类功能而不影响联邦事件处理
//!
//! ## Schema
//!
//! - `matrix_users`: 本地用户账户
//! - `matrix_devices`: 设备注册
//! - `matrix_tokens`: 访问令牌
//! - `matrix_profiles`: 用户详细资料（扩展）

use rusqlite::{Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

use super::error::{MatrixError, MatrixResult};

/// 用户记录
#[derive(Debug, Clone)]
pub struct UserRecord {
    pub user_id: String,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub created_at: i64,
}

/// 设备记录
#[derive(Debug, Clone)]
pub struct DeviceRecord {
    pub device_id: String,
    pub user_id: String,
    pub display_name: Option<String>,
    pub last_seen: Option<i64>,
    pub ip_address: Option<String>,
    pub created_at: i64,
}

/// 令牌信息
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub token: String,
    pub user_id: String,
    pub device_id: Option<String>,
    pub created_at: i64,
    pub expires_at: Option<i64>,
}

/// 用户资料（扩展）
#[derive(Debug, Clone, Default)]
pub struct UserProfile {
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub status_msg: Option<String>,
}

/// Matrix 社交数据存储
///
/// 管理所有人类用户相关的本地数据，与协议事件存储分离。
/// 这是实现 Skill 化注册和灵活用户管理的基础。
pub struct MatrixSocialStore {
    db: Arc<Mutex<Connection>>,
}

impl MatrixSocialStore {
    /// 打开或创建社交存储
    pub fn open(path: &str) -> MatrixResult<Self> {
        let conn = Connection::open(path)
            .map_err(|e| MatrixError::Store(format!("Failed to open social database: {}", e)))?;

        // 启用 WAL 模式以提高并发性能
        let _journal_mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
            .map_err(|e| MatrixError::Store(format!("Failed to enable WAL: {}", e)))?;

        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };

        store.init_schema()?;
        Ok(store)
    }

    /// 创建内存中的存储（用于测试）
    pub fn open_in_memory() -> MatrixResult<Self> {
        let conn = Connection::open_in_memory()
            .map_err(|e| MatrixError::Store(format!("Failed to open in-memory database: {}", e)))?;

        let store = Self {
            db: Arc::new(Mutex::new(conn)),
        };

        store.init_schema()?;
        Ok(store)
    }

    /// 初始化数据库 Schema
    fn init_schema(&self) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // 用户表 - 本地用户账户
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_users (
                user_id TEXT PRIMARY KEY,
                display_name TEXT,
                avatar_url TEXT,
                created_at INTEGER DEFAULT (unixepoch())
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create users table: {}", e)))?;

        // 设备表 - 设备注册
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_devices (
                device_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                display_name TEXT,
                last_seen INTEGER,
                ip_address TEXT,
                created_at INTEGER DEFAULT (unixepoch()),
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create devices table: {}", e)))?;

        // 访问令牌表
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_tokens (
                token TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                device_id TEXT,
                created_at INTEGER DEFAULT (unixepoch()),
                expires_at INTEGER,
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id),
                FOREIGN KEY (device_id) REFERENCES matrix_devices(device_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create tokens table: {}", e)))?;

        // 用户详细资料表（扩展）
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_profiles (
                user_id TEXT PRIMARY KEY,
                display_name TEXT,
                avatar_url TEXT,
                status_msg TEXT,
                updated_at INTEGER DEFAULT (unixepoch()),
                FOREIGN KEY (user_id) REFERENCES matrix_users(user_id)
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create profiles table: {}", e)))?;

        // 创建索引
        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_devices_user ON matrix_devices(user_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_tokens_user ON matrix_tokens(user_id)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_tokens_expires ON matrix_tokens(expires_at)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        // 登录验证码表 - 用于首次登录验证
        db.execute(
            "CREATE TABLE IF NOT EXISTS matrix_login_codes (
                user_id TEXT PRIMARY KEY,
                code TEXT NOT NULL,
                attempts INTEGER DEFAULT 0,
                created_at INTEGER DEFAULT (unixepoch()),
                expires_at INTEGER NOT NULL,
                verified INTEGER DEFAULT 0
            )",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create login codes table: {}", e)))?;

        db.execute(
            "CREATE INDEX IF NOT EXISTS idx_login_codes_expires ON matrix_login_codes(expires_at)",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to create index: {}", e)))?;

        Ok(())
    }

    // ==================== User Management ====================

    /// 创建新用户
    pub fn create_user(&self, user_id: &str, profile: Option<UserProfile>) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let profile = profile.unwrap_or_default();

        // 插入用户
        db.execute(
            "INSERT INTO matrix_users (user_id, display_name, avatar_url) 
             VALUES (?1, ?2, ?3)
             ON CONFLICT(user_id) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, display_name),
             avatar_url = COALESCE(excluded.avatar_url, avatar_url)",
            rusqlite::params![
                user_id,
                profile.display_name.as_deref().or(Some(user_id)),
                profile.avatar_url
            ],
        ).map_err(|e| MatrixError::Store(format!("Failed to create user: {}", e)))?;

        // 同步到 profiles 表
        db.execute(
            "INSERT INTO matrix_profiles (user_id, display_name, avatar_url, status_msg) 
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(user_id) DO UPDATE SET
             display_name = excluded.display_name,
             avatar_url = excluded.avatar_url,
             status_msg = excluded.status_msg,
             updated_at = unixepoch()",
            rusqlite::params![
                user_id,
                profile.display_name,
                profile.avatar_url,
                profile.status_msg
            ],
        ).map_err(|e| MatrixError::Store(format!("Failed to create profile: {}", e)))?;

        Ok(())
    }

    /// 获取用户信息
    pub fn get_user(&self, user_id: &str) -> MatrixResult<Option<UserRecord>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<UserRecord>, rusqlite::Error> = db.query_row(
            "SELECT user_id, display_name, avatar_url, created_at 
             FROM matrix_users WHERE user_id = ?1",
            [user_id],
            |row| {
                Ok(UserRecord {
                    user_id: row.get(0)?,
                    display_name: row.get(1)?,
                    avatar_url: row.get(2)?,
                    created_at: row.get(3)?,
                })
            },
        ).optional();

        match result {
            Ok(user) => Ok(user),
            Err(e) => Err(MatrixError::Store(format!("Failed to get user: {}", e))),
        }
    }

    /// 检查用户是否存在
    pub fn user_exists(&self, user_id: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let count: i64 = db.query_row(
            "SELECT COUNT(*) FROM matrix_users WHERE user_id = ?1",
            [user_id],
            |row| row.get(0),
        ).map_err(|e| MatrixError::Store(format!("Failed to check user: {}", e)))?;

        Ok(count > 0)
    }

    /// 删除用户及其所有关联数据
    pub fn delete_user(&self, user_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // 删除关联数据（外键会自动处理，但显式删除更清晰）
        db.execute("DELETE FROM matrix_tokens WHERE user_id = ?1", [user_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete tokens: {}", e)))?;
        
        db.execute("DELETE FROM matrix_devices WHERE user_id = ?1", [user_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete devices: {}", e)))?;
        
        db.execute("DELETE FROM matrix_profiles WHERE user_id = ?1", [user_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete profile: {}", e)))?;
        
        db.execute("DELETE FROM matrix_users WHERE user_id = ?1", [user_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete user: {}", e)))?;

        Ok(())
    }

    /// 更新用户资料
    pub fn update_profile(&self, user_id: &str, profile: UserProfile) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // 更新 users 表
        db.execute(
            "UPDATE matrix_users 
             SET display_name = COALESCE(?2, display_name),
                 avatar_url = COALESCE(?3, avatar_url)
             WHERE user_id = ?1",
            rusqlite::params![user_id, profile.display_name, profile.avatar_url],
        ).map_err(|e| MatrixError::Store(format!("Failed to update user: {}", e)))?;

        // 更新 profiles 表
        db.execute(
            "INSERT INTO matrix_profiles (user_id, display_name, avatar_url, status_msg, updated_at)
             VALUES (?1, ?2, ?3, ?4, unixepoch())
             ON CONFLICT(user_id) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, display_name),
             avatar_url = COALESCE(excluded.avatar_url, avatar_url),
             status_msg = COALESCE(excluded.status_msg, status_msg),
             updated_at = unixepoch()",
            rusqlite::params![user_id, profile.display_name, profile.avatar_url, profile.status_msg],
        ).map_err(|e| MatrixError::Store(format!("Failed to update profile: {}", e)))?;

        Ok(())
    }

    /// 获取用户资料
    pub fn get_profile(&self, user_id: &str) -> MatrixResult<Option<UserProfile>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<UserProfile>, rusqlite::Error> = db.query_row(
            "SELECT display_name, avatar_url, status_msg 
             FROM matrix_profiles WHERE user_id = ?1",
            [user_id],
            |row| {
                Ok(UserProfile {
                    display_name: row.get(0)?,
                    avatar_url: row.get(1)?,
                    status_msg: row.get(2)?,
                })
            },
        ).optional();

        match result {
            Ok(profile) => Ok(profile),
            Err(e) => Err(MatrixError::Store(format!("Failed to get profile: {}", e))),
        }
    }

    // ==================== Device Management ====================

    /// 注册设备
    pub fn register_device(
        &self,
        device_id: &str,
        user_id: &str,
        display_name: Option<&str>,
        ip_address: Option<&str>,
    ) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT INTO matrix_devices (device_id, user_id, display_name, ip_address, last_seen) 
             VALUES (?1, ?2, ?3, ?4, unixepoch())
             ON CONFLICT(device_id) DO UPDATE SET
             display_name = COALESCE(excluded.display_name, display_name),
             ip_address = COALESCE(excluded.ip_address, ip_address),
             last_seen = unixepoch()",
            rusqlite::params![device_id, user_id, display_name, ip_address],
        ).map_err(|e| MatrixError::Store(format!("Failed to register device: {}", e)))?;

        Ok(())
    }

    /// 获取用户的所有设备
    pub fn get_user_devices(&self, user_id: &str) -> MatrixResult<Vec<DeviceRecord>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let mut stmt = db.prepare(
            "SELECT device_id, user_id, display_name, last_seen, ip_address, created_at 
             FROM matrix_devices WHERE user_id = ?1"
        ).map_err(|e| MatrixError::Store(format!("Failed to prepare query: {}", e)))?;

        let devices: Result<Vec<DeviceRecord>, rusqlite::Error> = stmt
            .query_map([user_id], |row| {
                Ok(DeviceRecord {
                    device_id: row.get(0)?,
                    user_id: row.get(1)?,
                    display_name: row.get(2)?,
                    last_seen: row.get(3)?,
                    ip_address: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| MatrixError::Store(format!("Failed to query devices: {}", e)))?
            .collect();

        devices.map_err(|e| MatrixError::Store(format!("Failed to collect devices: {}", e)))
    }

    /// 获取单个设备信息
    pub fn get_device(&self, device_id: &str) -> MatrixResult<Option<DeviceRecord>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<DeviceRecord>, rusqlite::Error> = db.query_row(
            "SELECT device_id, user_id, display_name, last_seen, ip_address, created_at 
             FROM matrix_devices WHERE device_id = ?1",
            [device_id],
            |row| {
                Ok(DeviceRecord {
                    device_id: row.get(0)?,
                    user_id: row.get(1)?,
                    display_name: row.get(2)?,
                    last_seen: row.get(3)?,
                    ip_address: row.get(4)?,
                    created_at: row.get(5)?,
                })
            },
        ).optional();

        match result {
            Ok(device) => Ok(device),
            Err(e) => Err(MatrixError::Store(format!("Failed to get device: {}", e))),
        }
    }

    /// 更新设备最后活跃时间
    pub fn touch_device(&self, device_id: &str, ip_address: Option<&str>) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "UPDATE matrix_devices 
             SET last_seen = unixepoch(),
                 ip_address = COALESCE(?2, ip_address)
             WHERE device_id = ?1",
            rusqlite::params![device_id, ip_address],
        ).map_err(|e| MatrixError::Store(format!("Failed to update device: {}", e)))?;

        Ok(())
    }

    /// 删除设备
    pub fn delete_device(&self, device_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute("DELETE FROM matrix_tokens WHERE device_id = ?1", [device_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete device tokens: {}", e)))?;

        db.execute("DELETE FROM matrix_devices WHERE device_id = ?1", [device_id])
            .map_err(|e| MatrixError::Store(format!("Failed to delete device: {}", e)))?;

        Ok(())
    }

    // ==================== Token Management ====================

    /// 创建访问令牌
    pub fn create_token(
        &self,
        user_id: &str,
        device_id: Option<&str>,
        expires_in_secs: Option<i64>,
    ) -> MatrixResult<String> {
        let token = generate_access_token();
        let expires_at = expires_in_secs.map(|secs| chrono::Utc::now().timestamp() + secs);

        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute(
            "INSERT INTO matrix_tokens (token, user_id, device_id, expires_at) 
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(token) DO UPDATE SET
             user_id = excluded.user_id,
             device_id = excluded.device_id,
             created_at = unixepoch(),
             expires_at = excluded.expires_at",
            rusqlite::params![&token, user_id, device_id, expires_at],
        ).map_err(|e| MatrixError::Store(format!("Failed to create token: {}", e)))?;

        Ok(token)
    }

    /// 验证令牌
    pub fn validate_token(&self, token: &str) -> MatrixResult<Option<TokenInfo>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<TokenInfo>, rusqlite::Error> = db.query_row(
            "SELECT token, user_id, device_id, created_at, expires_at 
             FROM matrix_tokens 
             WHERE token = ?1 AND (expires_at IS NULL OR expires_at > unixepoch())",
            [token],
            |row| {
                Ok(TokenInfo {
                    token: row.get(0)?,
                    user_id: row.get(1)?,
                    device_id: row.get(2)?,
                    created_at: row.get(3)?,
                    expires_at: row.get(4)?,
                })
            },
        ).optional();

        match result {
            Ok(info) => Ok(info),
            Err(e) => Err(MatrixError::Store(format!("Failed to validate token: {}", e))),
        }
    }

    /// 获取令牌对应的用户 ID（简化版）
    pub fn get_token_user_id(&self, token: &str) -> MatrixResult<Option<String>> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let result: Result<Option<String>, rusqlite::Error> = db.query_row(
            "SELECT user_id FROM matrix_tokens 
             WHERE token = ?1 AND (expires_at IS NULL OR expires_at > unixepoch())",
            [token],
            |row| row.get(0),
        ).optional();

        match result {
            Ok(user_id) => Ok(user_id),
            Err(e) => Err(MatrixError::Store(format!("Failed to get token user: {}", e))),
        }
    }

    /// 吊销令牌
    pub fn revoke_token(&self, token: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute("DELETE FROM matrix_tokens WHERE token = ?1", [token])
            .map_err(|e| MatrixError::Store(format!("Failed to revoke token: {}", e)))?;

        Ok(())
    }

    /// 吊销用户的所有令牌
    pub fn revoke_user_tokens(&self, user_id: &str) -> MatrixResult<()> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        db.execute("DELETE FROM matrix_tokens WHERE user_id = ?1", [user_id])
            .map_err(|e| MatrixError::Store(format!("Failed to revoke user tokens: {}", e)))?;

        Ok(())
    }

    /// 清理过期令牌
    pub fn cleanup_expired_tokens(&self) -> MatrixResult<usize> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let count = db.execute(
            "DELETE FROM matrix_tokens WHERE expires_at IS NOT NULL AND expires_at <= unixepoch()",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to cleanup tokens: {}", e)))?;

        Ok(count)
    }

    // ==================== Registration API ====================

    /// 完整的用户注册（创建用户 + 设备 + 令牌）
    /// 
    /// 这是 Matrix Client-Server API 注册流程的核心实现。
    /// 返回 (user_id, access_token, device_id)
    pub fn register_user_complete(
        &self,
        user_id: &str,
        device_id: Option<&str>,
        display_name: Option<&str>,
    ) -> MatrixResult<(String, String, String)> {
        // 生成设备 ID（如果没有提供）
        let device_id = device_id.map(|s| s.to_string()).unwrap_or_else(generate_device_id);

        // 创建用户
        let profile = display_name.map(|name| UserProfile {
            display_name: Some(name.to_string()),
            avatar_url: None,
            status_msg: None,
        });
        self.create_user(user_id, profile)?;

        // 注册设备
        self.register_device(&device_id, user_id, display_name, None)?;

        // 创建访问令牌（默认 30 天过期）
        let token = self.create_token(user_id, Some(&device_id), Some(30 * 24 * 60 * 60))?;

        Ok((user_id.to_string(), token, device_id))
    }

    /// 获取数据库连接（用于直接访问）
    pub fn conn(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.db.lock().expect("Failed to lock database")
    }
}

/// 生成随机访问令牌
fn generate_access_token() -> String {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    const TOKEN_LEN: usize = 64;

    let mut rng = rand::thread_rng();
    (0..TOKEN_LEN)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect()
}

/// 生成设备 ID
fn generate_device_id() -> String {
    format!("{}_{}", 
        &uuid::Uuid::new_v4().to_string()[..8],
        chrono::Utc::now().timestamp_millis()
    )
}

/// 生成6位数字验证码
fn generate_verification_code() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..6)
        .map(|_| rng.gen_range(0..10).to_string())
        .collect()
}

// ==================== Login Verification Code Methods ====================

impl MatrixSocialStore {
    /// 为用户生成登录验证码（首次登录时需要）
    /// 
    /// 返回 (code, is_new_user)
    pub fn generate_login_code(&self, user_id: &str) -> MatrixResult<(String, bool)> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let is_new_user = !self.user_exists(user_id)?;

        // 生成6位验证码
        let code = generate_verification_code();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let expires_at = now + 300; // 5分钟过期

        // 插入或更新验证码
        db.execute(
            "INSERT INTO matrix_login_codes (user_id, code, attempts, created_at, expires_at, verified)
             VALUES (?1, ?2, 0, ?3, ?4, 0)
             ON CONFLICT(user_id) DO UPDATE SET
             code = excluded.code,
             attempts = 0,
             created_at = excluded.created_at,
             expires_at = excluded.expires_at,
             verified = 0",
            rusqlite::params![user_id, code, now, expires_at],
        ).map_err(|e| MatrixError::Store(format!("Failed to create login code: {}", e)))?;

        Ok((code, is_new_user))
    }

    /// 验证登录验证码
    /// 
    /// 返回 Ok(true) 表示验证成功
    /// 返回 Ok(false) 表示验证失败但可重试
    /// 返回 Err 表示验证失败且不可重试（如过期、尝试次数过多）
    pub fn verify_login_code(&self, user_id: &str, input_code: &str) -> MatrixResult<bool> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        // 获取验证码信息
        let (stored_code, attempts, expires_at, verified): (String, i32, i64, i32) = db.query_row(
            "SELECT code, attempts, expires_at, verified FROM matrix_login_codes WHERE user_id = ?1",
            [user_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i32>(3)?,
                ))
            },
        ).map_err(|_| MatrixError::NotFound("No verification code found".to_string()))?;

        // 检查是否已验证
        if verified != 0 {
            return Err(MatrixError::Forbidden("Code already verified".to_string()));
        }

        // 检查是否过期
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        if now > expires_at {
            return Err(MatrixError::Forbidden("Verification code expired".to_string()));
        }

        // 检查尝试次数
        if attempts >= 5 {
            return Err(MatrixError::InvalidParameter("Too many attempts".to_string()));
        }

        // 增加尝试次数
        db.execute(
            "UPDATE matrix_login_codes SET attempts = attempts + 1 WHERE user_id = ?1",
            [user_id],
        ).map_err(|e| MatrixError::Store(format!("Failed to update attempts: {}", e)))?;

        // 验证验证码
        if stored_code == input_code {
            // 标记为已验证
            db.execute(
                "UPDATE matrix_login_codes SET verified = 1 WHERE user_id = ?1",
                [user_id],
            ).map_err(|e| MatrixError::Store(format!("Failed to mark code as verified: {}", e)))?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// 检查用户是否需要验证码登录
    pub fn needs_verification_code(&self, user_id: &str) -> MatrixResult<bool> {
        // 如果是新用户，需要验证码
        if !self.user_exists(user_id)? {
            return Ok(true);
        }

        // 如果用户存在但没有已验证的记录，需要验证码
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let verified: Option<i32> = db.query_row(
            "SELECT verified FROM matrix_login_codes WHERE user_id = ?1",
            [user_id],
            |row| row.get(0),
        ).optional().map_err(|e| MatrixError::Store(format!("Failed to check verification: {}", e)))?;

        match verified {
            Some(1) => Ok(false), // 已验证过
            _ => Ok(true),        // 未验证或记录不存在
        }
    }

    /// 清理过期验证码
    pub fn cleanup_expired_codes(&self) -> MatrixResult<usize> {
        let db = self.db.lock()
            .map_err(|_| MatrixError::Internal("Failed to lock database".to_string()))?;

        let count = db.execute(
            "DELETE FROM matrix_login_codes WHERE expires_at <= unixepoch()",
            [],
        ).map_err(|e| MatrixError::Store(format!("Failed to cleanup codes: {}", e)))?;

        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_lifecycle() {
        let store = MatrixSocialStore::open_in_memory().unwrap();

        // 创建用户
        let user_id = "@test:example.com";
        let profile = UserProfile {
            display_name: Some("Test User".to_string()),
            avatar_url: Some("mxc://example/avatar".to_string()),
            status_msg: None,
        };
        store.create_user(user_id, Some(profile)).unwrap();

        // 验证用户存在
        assert!(store.user_exists(user_id).unwrap());

        // 获取用户信息
        let user = store.get_user(user_id).unwrap().unwrap();
        assert_eq!(user.user_id, user_id);
        assert_eq!(user.display_name, Some("Test User".to_string()));

        // 更新资料
        let new_profile = UserProfile {
            display_name: Some("Updated Name".to_string()),
            avatar_url: None,
            status_msg: Some("Hello!".to_string()),
        };
        store.update_profile(user_id, new_profile).unwrap();

        let profile = store.get_profile(user_id).unwrap().unwrap();
        assert_eq!(profile.display_name, Some("Updated Name".to_string()));
        assert_eq!(profile.status_msg, Some("Hello!".to_string()));

        // 删除用户
        store.delete_user(user_id).unwrap();
        assert!(!store.user_exists(user_id).unwrap());
    }

    #[test]
    fn test_device_management() {
        let store = MatrixSocialStore::open_in_memory().unwrap();

        // 创建用户
        let user_id = "@device_test:example.com";
        store.create_user(user_id, None).unwrap();

        // 注册设备
        let device_id = "DEVICE123";
        store.register_device(device_id, user_id, Some("Element Web"), Some("192.168.1.1")).unwrap();

        // 获取设备
        let device = store.get_device(device_id).unwrap().unwrap();
        assert_eq!(device.device_id, device_id);
        assert_eq!(device.display_name, Some("Element Web".to_string()));

        // 获取用户设备列表
        let devices = store.get_user_devices(user_id).unwrap();
        assert_eq!(devices.len(), 1);

        // 删除设备
        store.delete_device(device_id).unwrap();
        assert!(store.get_device(device_id).unwrap().is_none());
    }

    #[test]
    fn test_token_management() {
        let store = MatrixSocialStore::open_in_memory().unwrap();

        // 创建用户
        let user_id = "@token_test:example.com";
        store.create_user(user_id, None).unwrap();

        // 创建令牌
        let token = store.create_token(user_id, None, None).unwrap();
        assert_eq!(token.len(), 64);

        // 验证令牌
        let info = store.validate_token(&token).unwrap().unwrap();
        assert_eq!(info.user_id, user_id);

        // 获取用户 ID
        let uid = store.get_token_user_id(&token).unwrap().unwrap();
        assert_eq!(uid, user_id);

        // 吊销令牌
        store.revoke_token(&token).unwrap();
        assert!(store.validate_token(&token).unwrap().is_none());
    }

    #[test]
    fn test_complete_registration() {
        let store = MatrixSocialStore::open_in_memory().unwrap();

        let user_id = "@reg_test:example.com";
        let (uid, token, device_id) = store.register_user_complete(
            user_id,
            Some("MYDEVICE"),
            Some("My Device"),
        ).unwrap();

        assert_eq!(uid, user_id);
        assert_eq!(device_id, "MYDEVICE");
        assert!(!token.is_empty());

        // 验证用户创建成功
        assert!(store.user_exists(user_id).unwrap());

        // 验证令牌有效
        assert!(store.validate_token(&token).unwrap().is_some());
    }
}
