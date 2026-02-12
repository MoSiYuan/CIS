// CIS v1.1.6 - WASM 燃料限制器
//
// 本模块实现了 WASM 执行的燃料（fuel）限制机制。
// 燃料是 WebAssembly 中用于限制执行时间的机制。
//
// 每个指令消耗一定量的燃料，当燃料耗尽时，执行会被终止。
// 这可以防止无限循环和拒绝服务攻击。

use std::sync::Arc;
use std::time::{Duration, Instant};
use anyhow::{anyhow, Result};
use wasmtime::{FuelConfig, Store};

use crate::error::{CisError, Result as CisResult};

/// 燃料限制配置
#[derive(Debug, Clone)]
pub struct FuelConfig {
    /// 初始燃料量
    pub initial_fuel: u64,
    /// 燃料消耗周期（毫秒）
    pub fuel_interval_ms: u64,
    /// 每个周期补充的燃料量
    pub fuel_refill_amount: u64,
    /// 最大燃料积累量
    pub max_fuel_accumulation: u64,
    /// 是否启用自动补充
    pub enable_auto_refill: bool,
}

impl Default for FuelConfig {
    fn default() -> Self {
        Self {
            initial_fuel: 10_000_000_000,    // 100 亿燃料单位
            fuel_interval_ms: 1000,          // 每秒补充一次
            fuel_refill_amount: 1_000_000_000, // 每次补充 10 亿
            max_fuel_accumulation: 50_000_000_000, // 最多积累 500 亿
            enable_auto_refill: true,
        }
    }
}

impl FuelConfig {
    /// 创建新的燃料配置
    pub fn new(initial_fuel: u64) -> Self {
        Self {
            initial_fuel,
            ..Default::default()
        }
    }

    /// 设置燃料补充参数
    pub fn with_refill(mut self, interval_ms: u64, amount: u64) -> Self {
        self.fuel_interval_ms = interval_ms;
        self.fuel_refill_amount = amount;
        self
    }

    /// 设置最大积累量
    pub fn with_max_accumulation(mut self, max: u64) -> Self {
        self.max_fuel_accumulation = max;
        self
    }

    /// 禁用自动补充
    pub fn without_auto_refill(mut self) -> Self {
        self.enable_auto_refill = false;
        self
    }

    /// 验证配置
    pub fn validate(&self) -> Result<()> {
        if self.initial_fuel == 0 {
            return Err(anyhow!("Initial fuel cannot be zero"));
        }

        if self.max_fuel_accumulation < self.initial_fuel {
            return Err(anyhow!(
                "Max fuel accumulation must be >= initial fuel"
            ));
        }

        if self.enable_auto_refill && self.fuel_refill_amount == 0 {
            return Err(anyhow!("Fuel refill amount cannot be zero when auto-refill is enabled"));
        }

        Ok(())
    }
}

/// 燃料使用统计
#[derive(Debug, Clone)]
pub struct FuelStats {
    /// 当前剩余燃料
    pub current_fuel: u64,
    /// 已消耗燃料
    pub consumed_fuel: u64,
    /// 累计补充燃料
    pub refilled_fuel: u64,
    /// 燃料补充次数
    pub refill_count: u64,
    /// 执行时间
    pub elapsed_time: Duration,
    /// 平均燃料消耗率（燃料/秒）
    pub consumption_rate: f64,
}

/// 燃料限制器
///
/// 管理和监控 WASM 模块的燃料消耗
#[derive(Debug)]
pub struct FuelLimiter {
    /// 配置
    config: FuelConfig,
    /// 初始燃料
    initial_fuel: u64,
    /// 燃料补充计数
    refill_count: Arc<std::sync::atomic::AtomicU64>,
    /// 累计补充量
    refilled_amount: Arc<std::sync::atomic::AtomicU64>,
    /// 开始时间
    start_time: Instant,
    /// 最后一次补充时间
    last_refill_time: Arc<std::sync::Mutex<Instant>>,
    /// 是否已耗尽
    exhausted: Arc<std::sync::atomic::AtomicBool>,
}

impl FuelLimiter {
    /// 创建新的燃料限制器
    pub fn new(config: FuelConfig) -> Result<Self> {
        config.validate()?;

        let start_time = Instant::now();
        let last_refill_time = Arc::new(std::sync::Mutex::new(start_time));

        Ok(Self {
            initial_fuel: config.initial_fuel,
            config,
            refill_count: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            refilled_amount: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            start_time,
            last_refill_time,
            exhausted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
        })
    }

    /// 使用默认配置创建
    pub fn with_default_config() -> Result<Self> {
        Self::new(FuelConfig::default())
    }

    /// 设置燃料到 Store
    pub fn set_fuel_to_store<T>(&self, store: &mut Store<T>) -> Result<()> {
        store.set_fuel(self.config.initial_fuel)
            .map_err(|e| anyhow!("Failed to set fuel: {}", e))
    }

    /// 检查并补充燃料
    pub fn check_and_refill<T>(&self, store: &mut Store<T>) -> Result<bool> {
        if !self.config.enable_auto_refill {
            return Ok(false);
        }

        let mut last_refill = self.last_refill_time.lock().unwrap();
        let elapsed = last_refill.elapsed();

        if elapsed >= Duration::from_millis(self.config.fuel_interval_ms) {
            // 检查是否超过最大积累量
            let current_fuel = store.fuel_consumed()
                .map_err(|e| anyhow!("Failed to get fuel consumed: {}", e))?;

            let remaining = self.config.initial_fuel.saturating_sub(current_fuel);

            if remaining >= self.config.max_fuel_accumulation {
                // 已达到最大积累量，不补充
                return Ok(false);
            }

            // 补充燃料
            let refill_amount = std::cmp::min(
                self.config.fuel_refill_amount,
                self.config.max_fuel_accumulation - remaining,
            );

            store.add_fuel(refill_amount)
                .map_err(|e| anyhow!("Failed to add fuel: {}", e))?;

            // 更新统计
            self.refill_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.refilled_amount.fetch_add(refill_amount, std::sync::atomic::Ordering::Relaxed);

            // 更新最后补充时间
            *last_refill = Instant::now();

            return Ok(true);
        }

        Ok(false)
    }

    /// 检查燃料是否耗尽
    pub fn is_exhausted<T>(&self, store: &Store<T>) -> bool {
        match store.fuel_consumed() {
            Ok(consumed) => {
                if consumed >= self.config.initial_fuel {
                    self.exhausted.store(true, std::sync::atomic::Ordering::Relaxed);
                    return true;
                }
                false
            }
            Err(_) => {
                // 如果无法获取燃料状态，假设已耗尽
                true
            }
        }
    }

    /// 获取燃料统计
    pub fn get_stats<T>(&self, store: &Store<T>) -> Result<FuelStats> {
        let consumed = store.fuel_consumed()
            .unwrap_or(0);

        let current_fuel = self.config.initial_fuel.saturating_sub(consumed);
        let refilled = self.refilled_amount.load(std::sync::atomic::Ordering::Relaxed);
        let refill_count = self.refill_count.load(std::sync::atomic::Ordering::Relaxed);
        let elapsed = self.start_time.elapsed();

        // 计算消耗率（燃料/秒）
        let consumption_rate = if elapsed.as_secs_f64() > 0.0 {
            consumed as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        };

        Ok(FuelStats {
            current_fuel,
            consumed_fuel: consumed,
            refilled_fuel: refilled,
            refill_count,
            elapsed_time: elapsed,
            consumption_rate,
        })
    }

    /// 获取配置
    pub fn config(&self) -> &FuelConfig {
        &self.config
    }

    /// 获取初始燃料量
    pub fn initial_fuel(&self) -> u64 {
        self.initial_fuel
    }
}

/// 燃料使用分析器
///
/// 分析燃料使用模式，检测异常
#[derive(Debug)]
pub struct FuelAnalyzer {
    /// 燃料消耗历史
    consumption_history: Vec<FuelStats>,
    /// 最大历史记录数
    max_history_size: usize,
}

impl FuelAnalyzer {
    /// 创建新的分析器
    pub fn new(max_history_size: usize) -> Self {
        Self {
            consumption_history: Vec::with_capacity(max_history_size),
            max_history_size,
        }
    }

    /// 记录燃料统计
    pub fn record(&mut self, stats: FuelStats) {
        if self.consumption_history.len() >= self.max_history_size {
            self.consumption_history.remove(0);
        }
        self.consumption_history.push(stats);
    }

    /// 分析燃料使用趋势
    pub fn analyze_trend(&self) -> FuelTrend {
        if self.consumption_history.len() < 2 {
            return FuelTrend::InsufficientData;
        }

        let len = self.consumption_history.len();
        let recent = &self.consumption_history[len - 1];
        let previous = &self.consumption_history[len - 2];

        // 比较最近的消耗率
        let rate_change = recent.consumption_rate - previous.consumption_rate;

        // 比较总消耗量
        let total_change = recent.consumed_fuel as i64 - previous.consumed_fuel as i64;

        match (rate_change, total_change) {
            (r, _) if r.abs() < 1000.0 => FuelTrend::Stable,
            (r, _) if r > 1000.0 && total_change > 0 => FuelTrend::Increasing,
            (r, _) if r < -1000.0 => FuelTrend::Decreasing,
            _ => FuelTrend::Stable,
        }
    }

    /// 检测异常消耗
    pub fn detect_anomalies(&self) -> Vec<FuelAnomaly> {
        let mut anomalies = Vec::new();

        if self.consumption_history.len() < 3 {
            return anomalies;
        }

        let len = self.consumption_history.len();
        let latest = &self.consumption_history[len - 1];
        let avg_consumption: f64 = self.consumption_history
            .iter()
            .map(|s| s.consumed_fuel as f64)
            .sum::<f64>() / len as f64;

        // 检测异常高消耗
        if latest.consumed_fuel as f64 > avg_consumption * 2.0 {
            anomalies.push(FuelAnomaly::AbnormallyHighConsumption {
                current: latest.consumed_fuel,
                average: avg_consumption as u64,
            });
        }

        // 检测快速耗尽
        if latest.elapsed_time < Duration::from_secs(1) &&
           latest.current_fuel == 0 {
            anomalies.push(FuelAnomaly::RapidDepletion {
                time: latest.elapsed_time,
                consumed: latest.consumed_fuel,
            });
        }

        anomalies
    }

    /// 获取平均消耗率
    pub fn average_consumption_rate(&self) -> f64 {
        if self.consumption_history.is_empty() {
            return 0.0;
        }

        let sum: f64 = self.consumption_history
            .iter()
            .map(|s| s.consumption_rate)
            .sum();

        sum / self.consumption_history.len() as f64
    }

    /// 清空历史
    pub fn clear(&mut self) {
        self.consumption_history.clear();
    }
}

/// 燃料趋势
#[derive(Debug, Clone, PartialEq)]
pub enum FuelTrend {
    /// 稳定
    Stable,
    /// 上升
    Increasing,
    /// 下降
    Decreasing,
    /// 数据不足
    InsufficientData,
}

/// 燃料异常
#[derive(Debug, Clone)]
pub enum FuelAnomaly {
    /// 异常高消耗
    AbnormallyHighConsumption {
        current: u64,
        average: u64,
    },
    /// 快速耗尽
    RapidDepletion {
        time: Duration,
        consumed: u64,
    },
    /// 异常消耗率
    UnusualRate {
        rate: f64,
        expected_range: (f64, f64),
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuel_config_validation() {
        let config = FuelConfig::default();
        assert!(config.validate().is_ok());

        // 无效的初始燃料
        let invalid_config = FuelConfig {
            initial_fuel: 0,
            ..Default::default()
        };
        assert!(invalid_config.validate().is_err());
    }

    #[test]
    fn test_fuel_limiter_creation() {
        let limiter = FuelLimiter::with_default_config();
        assert!(limiter.is_ok());

        let limiter = limiter.unwrap();
        assert_eq!(limiter.initial_fuel(), FuelConfig::default().initial_fuel);
    }

    #[test]
    fn test_fuel_stats() {
        let config = FuelConfig::default();
        let limiter = FuelLimiter::new(config).unwrap();

        // 模拟燃料统计
        let stats = FuelStats {
            current_fuel: 5_000_000_000,
            consumed_fuel: 5_000_000_000,
            refilled_fuel: 1_000_000_000,
            refill_count: 1,
            elapsed_time: Duration::from_secs(5),
            consumption_rate: 1_000_000_000.0,
        };

        assert_eq!(stats.current_fuel, 5_000_000_000);
        assert_eq!(stats.consumed_fuel, 5_000_000_000);
        assert_eq!(stats.refilled_fuel, 1_000_000_000);
        assert_eq!(stats.refill_count, 1);
    }

    #[test]
    fn test_fuel_analyzer() {
        let mut analyzer = FuelAnalyzer::new(10);

        // 记录一些统计数据
        let stats1 = FuelStats {
            current_fuel: 9_000_000_000,
            consumed_fuel: 1_000_000_000,
            refilled_fuel: 0,
            refill_count: 0,
            elapsed_time: Duration::from_secs(1),
            consumption_rate: 1_000_000_000.0,
        };

        let stats2 = FuelStats {
            current_fuel: 8_000_000_000,
            consumed_fuel: 2_000_000_000,
            refilled_fuel: 0,
            refill_count: 0,
            elapsed_time: Duration::from_secs(2),
            consumption_rate: 1_000_000_000.0,
        };

        analyzer.record(stats1);
        analyzer.record(stats2);

        assert_eq!(analyzer.consumption_history.len(), 2);

        // 检测趋势
        let trend = analyzer.analyze_trend();
        assert_eq!(trend, FuelTrend::Stable);
    }

    #[test]
    fn test_fuel_anomaly_detection() {
        let mut analyzer = FuelAnalyzer::new(10);

        // 正常消耗
        for i in 1..=5 {
            analyzer.record(FuelStats {
                current_fuel: 10_000_000_000 - (i * 1_000_000_000),
                consumed_fuel: i * 1_000_000_000,
                refilled_fuel: 0,
                refill_count: 0,
                elapsed_time: Duration::from_secs(i as u64),
                consumption_rate: 1_000_000_000.0,
            });
        }

        // 异常高消耗
        analyzer.record(FuelStats {
            current_fuel: 0,
            consumed_fuel: 10_000_000_000,
            refilled_fuel: 0,
            refill_count: 0,
            elapsed_time: Duration::from_millis(100),
            consumption_rate: 100_000_000_000.0,
        });

        let anomalies = analyzer.detect_anomalies();
        assert!(!anomalies.is_empty());
    }
}
