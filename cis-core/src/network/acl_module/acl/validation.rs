//! ACL 时间戳验证模块
//!
//! 实现时间戳验证、过期检查和时钟偏差处理。

use crate::error::{CisError, Result};
use std::time::{Duration, SystemTime};

/// 时钟偏差容忍度（秒）
const CLOCK_TOLERANCE_SECS: u64 = 60;

/// ACL 条目验证器
#[derive(Debug, Clone)]
pub struct AclValidator {
    /// 时钟容忍度
    clock_tolerance: Duration,
}

impl Default for AclValidator {
    fn default() -> Self {
        Self {
            clock_tolerance: Duration::from_secs(CLOCK_TOLERANCE_SECS),
        }
    }
}

impl AclValidator {
    /// 创建新的验证器
    pub fn new() -> Self {
        Self::default()
    }

    /// 使用自定义时钟容忍度创建验证器
    pub fn with_tolerance(tolerance: Duration) -> Self {
        Self {
            clock_tolerance: tolerance,
        }
    }

    /// 验证时间戳
    ///
    /// 检查时间戳是否在有效范围内（考虑时钟偏差）。
    ///
    /// # 参数
    /// - `timestamp`: ACL 条目的时间戳
    /// - `expiry`: 有效期
    ///
    /// # 返回
    /// - `Result<()>`: 时间戳有效返回 Ok，否则返回错误
    pub fn validate_timestamp(
        &self,
        timestamp: SystemTime,
        expiry: Duration,
    ) -> Result<AclValidationResult> {
        let now = SystemTime::now();

        // 1. 检查时间戳是否在未来（允许时钟偏差）
        let time_since_timestamp = now
            .duration_since(timestamp)
            .map_err(|_| CisError::acl("Clock went backwards - timestamp is in the future"))?;

        // 如果时间戳在当前时间 + 容忍度之后，则是无效的
        if time_since_timestamp < self.clock_tolerance {
            // 时间戳在未来（但可能在容忍度内）
            if timestamp > now + self.clock_tolerance {
                return Ok(AclValidationResult::InvalidTimestamp(
                    "Timestamp is too far in the future".to_string(),
                ));
            }
        }

        // 2. 检查是否过期
        if time_since_timestamp > expiry {
            return Ok(AclValidationResult::Expired(format!(
                "ACL entry expired {} ago",
                format_duration(time_since_timestamp - expiry)
            )));
        }

        // 3. 检查是否过期（绝对时间）
        let absolute_expiry = timestamp + expiry;
        if now > absolute_expiry + self.clock_tolerance {
            return Ok(AclValidationResult::Expired(format!(
                "ACL entry expired (absolute time exceeded)"
            )));
        }

        // 4. 检查时间戳是否太旧（防止重放攻击）
        let max_age = expiry + self.clock_tolerance;
        if time_since_timestamp > max_age {
            return Ok(AclValidationResult::Expired(format!(
                "ACL entry is too old (max age: {})",
                format_duration(max_age)
            )));
        }

        Ok(AclValidationResult::Valid)
    }

    /// 验证时间戳（简化的过期检查）
    ///
    /// 只检查是否过期，不考虑时钟偏差。
    pub fn is_expired(&self, timestamp: SystemTime, expiry: Duration) -> bool {
        let now = SystemTime::now();

        // 计算过期时间
        let expiry_time = timestamp + expiry;

        // 检查是否已过期
        match now.duration_since(expiry_time) {
            Ok(_) => true,  // 现在时间在过期时间之后 -> 已过期
            Err(_) => false, // 现在时间在过期时间之前 -> 未过期
        }
    }

    /// 获取剩余有效期
    ///
    /// 返回 ACL 条目还有多长时间过期。
    pub fn remaining_time(&self, timestamp: SystemTime, expiry: Duration) -> Option<Duration> {
        let now = SystemTime::now();
        let expiry_time = timestamp + expiry;

        expiry_time
            .duration_since(now)
            .ok()
    }

    /// 检查时间戳是否在可接受范围内
    ///
    /// 用于验证新创建的 ACL 条目的时间戳是否合理。
    pub fn is_acceptable_timestamp(&self, timestamp: SystemTime) -> bool {
        let now = SystemTime::now();

        // 时间戳不能太远在未来
        if timestamp > now + self.clock_tolerance {
            return false;
        }

        // 时间戳不能太远在过去（防止重放攻击）
        if let Ok(elapsed) = now.duration_since(timestamp) {
            if elapsed > self.clock_tolerance * 10 {
                return false;
            }
        }

        true
    }
}

/// ACL 验证结果
#[derive(Debug, Clone, PartialEq)]
pub enum AclValidationResult {
    /// 有效
    Valid,
    /// 无效的时间戳
    InvalidTimestamp(String),
    /// 已过期
    Expired(String),
    /// 签名无效
    InvalidSignature(String),
}

impl AclValidationResult {
    /// 检查是否有效
    pub fn is_valid(&self) -> bool {
        matches!(self, Self::Valid)
    }

    /// 检查是否允许（有效期内的）
    pub fn is_allowed(&self) -> bool {
        self.is_valid()
    }

    /// 获取错误消息
    pub fn error_message(&self) -> Option<String> {
        match self {
            Self::Valid => None,
            Self::InvalidTimestamp(msg) => Some(msg.clone()),
            Self::Expired(msg) => Some(msg.clone()),
            Self::InvalidSignature(msg) => Some(msg.clone()),
        }
    }
}

/// 格式化持续时间
fn format_duration(duration: Duration) -> String {
    let secs = duration.as_secs();
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;

    if days > 0 {
        format!("{} days", days)
    } else if hours > 0 {
        format!("{} hours", hours)
    } else if mins > 0 {
        format!("{} minutes", mins)
    } else {
        format!("{} seconds", secs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_timestamp_valid() {
        let validator = AclValidator::new();
        let now = SystemTime::now();
        let expiry = Duration::from_secs(3600); // 1 hour

        let result = validator.validate_timestamp(now, expiry).unwrap();
        assert_eq!(result, AclValidationResult::Valid);
    }

    #[test]
    fn test_validate_timestamp_expired() {
        let validator = AclValidator::new();
        let old_time = SystemTime::now() - Duration::from_secs(7200); // 2 hours ago
        let expiry = Duration::from_secs(3600); // 1 hour validity

        let result = validator.validate_timestamp(old_time, expiry).unwrap();
        assert!(matches!(result, AclValidationResult::Expired(_)));
    }

    #[test]
    fn test_is_expired() {
        let validator = AclValidator::new();
        let now = SystemTime::now();

        // 未过期
        assert!(!validator.is_expired(now, Duration::from_secs(3600)));

        // 已过期
        let old_time = now - Duration::from_secs(3700);
        assert!(validator.is_expired(old_time, Duration::from_secs(3600)));
    }

    #[test]
    fn test_remaining_time() {
        let validator = AclValidator::new();
        let now = SystemTime::now();
        let expiry = Duration::from_secs(3600);

        // 还有时间
        let remaining = validator.remaining_time(now, expiry);
        assert!(remaining.is_some());
        assert!(remaining.unwrap().as_secs() > 3500);

        // 已过期
        let old_time = now - Duration::from_secs(100);
        let remaining = validator.remaining_time(old_time, expiry);
        assert!(remaining.is_none());
    }

    #[test]
    fn test_clock_tolerance_future() {
        let validator = AclValidator::with_tolerance(Duration::from_secs(60));
        let future_time = SystemTime::now() + Duration::from_secs(30); // 30 seconds in future
        let expiry = Duration::from_secs(3600);

        let result = validator.validate_timestamp(future_time, expiry).unwrap();
        // 在容忍度范围内，应该是有效的
        assert_eq!(result, AclValidationResult::Valid);
    }

    #[test]
    fn test_clock_tolerance_past() {
        let validator = AclValidator::with_tolerance(Duration::from_secs(60));
        let past_time = SystemTime::now() - Duration::from_secs(30); // 30 seconds ago
        let expiry = Duration::from_secs(3600);

        let result = validator.validate_timestamp(past_time, expiry).unwrap();
        // 在容忍度范围内，应该是有效的
        assert_eq!(result, AclValidationResult::Valid);
    }

    #[test]
    fn test_unacceptable_future_timestamp() {
        let validator = AclValidator::with_tolerance(Duration::from_secs(60));
        let far_future = SystemTime::now() + Duration::from_secs(120); // 2 minutes in future

        assert!(!validator.is_acceptable_timestamp(far_future));
    }

    #[test]
    fn test_unacceptable_past_timestamp() {
        let validator = AclValidator::with_tolerance(Duration::from_secs(60));
        let far_past = SystemTime::now() - Duration::from_secs(600); // 10 minutes ago

        assert!(!validator.is_acceptable_timestamp(far_past));
    }

    #[test]
    fn test_acceptable_timestamp() {
        let validator = AclValidator::with_tolerance(Duration::from_secs(60));
        let now = SystemTime::now();

        // 当前时间应该可接受
        assert!(validator.is_acceptable_timestamp(now));

        // 1 秒前应该可接受
        assert!(validator.is_acceptable_timestamp(now - Duration::from_secs(1)));

        // 1 秒后应该可接受
        assert!(validator.is_acceptable_timestamp(now + Duration::from_secs(1)));
    }

    #[test]
    fn test_format_duration() {
        assert!(format_duration(Duration::from_secs(45)).contains("seconds"));
        assert!(format_duration(Duration::from_secs(90)).contains("minutes"));
        assert!(format_duration(Duration::from_secs(7200)).contains("hours"));
        assert!(format_duration(Duration::from_secs(172800)).contains("days"));
    }
}
