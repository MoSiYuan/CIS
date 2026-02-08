//! # Countdown Timer for Recommended Level
//!
//! 实现 Recommended 级别的倒计时功能

use crate::types::Action;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tracing::{debug, info};

/// 倒计时定时器
#[derive(Debug, Clone)]
pub struct CountdownTimer {
    /// 总秒数
    total_seconds: u16,
    /// 默认动作
    default_action: Action,
    /// 是否已取消
    cancelled: Arc<AtomicBool>,
}

impl CountdownTimer {
    /// 创建新的倒计时定时器
    pub fn new(total_seconds: u16, default_action: Action) -> Self {
        Self {
            total_seconds,
            default_action,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 运行倒计时（带显示）
    pub async fn run_with_display(&self, task_id: &str) {
        let remaining = Arc::new(std::sync::atomic::AtomicU16::new(self.total_seconds));
        let cancelled = self.cancelled.clone();
        let action = self.default_action;
        let total = self.total_seconds;

        // 打印初始信息
        let action_str = match action {
            Action::Execute => "execute",
            Action::Skip => "skip",
            Action::Abort => "abort",
        };

        println!(
            "⏱️  Task '{}' will {} in {} seconds (Ctrl+C to interrupt)",
            task_id, action_str, total
        );

        // 启动倒计时显示任务
        let display_handle = tokio::spawn({
            let remaining = remaining.clone();
            let cancelled = cancelled.clone();
            let task_id = task_id.to_string();
            
            async move {
                let mut last_displayed = total + 1;
                
                loop {
                    if cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    
                    let current = remaining.load(Ordering::Relaxed);
                    
                    // 只在秒数变化时更新显示
                    if current != last_displayed && current > 0 {
                        Self::print_progress(&task_id, current, total, action);
                        last_displayed = current;
                    }
                    
                    if current == 0 {
                        break;
                    }
                    
                    sleep(Duration::from_millis(100)).await;
                }
            }
        });

        // 倒计时逻辑
        for i in (1..=self.total_seconds).rev() {
            remaining.store(i, Ordering::Relaxed);
            
            // 每秒检查一次是否被取消
            for _ in 0..10 {
                if self.cancelled.load(Ordering::Relaxed) {
                    break;
                }
                sleep(Duration::from_millis(100)).await;
            }
            
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }
        }

        remaining.store(0, Ordering::Relaxed);
        
        // 等待显示任务完成
        let _ = display_handle.await;
        
        // 打印完成信息
        if !self.cancelled.load(Ordering::Relaxed) {
            println!(); // 换行
            info!("Countdown complete for task '{}', proceeding with {:?}", task_id, action);
        }
    }

    /// 运行倒计时（静默模式，无显示）
    pub async fn run_silent(&self) {
        for _ in 0..self.total_seconds {
            if self.cancelled.load(Ordering::Relaxed) {
                break;
            }
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// 取消倒计时
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
        debug!("Countdown cancelled");
    }

    /// 是否已取消
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// 获取剩余时间
    pub fn remaining_seconds(&self) -> u16 {
        self.total_seconds
    }

    /// 获取默认动作
    pub fn default_action(&self) -> Action {
        self.default_action
    }

    /// 打印进度条
    fn print_progress(task_id: &str, remaining: u16, total: u16, action: Action) {
        let action_str = match action {
            Action::Execute => "execute",
            Action::Skip => "skip",
            Action::Abort => "abort",
        };

        let progress = (total - remaining) as f32 / total as f32;
        let filled = (progress * 20.0) as usize;
        let empty = 20 - filled;

        let bar: String = std::iter::repeat('█').take(filled)
            .chain(std::iter::repeat('░').take(empty))
            .collect();

        print!("\r⏱️  [{}] {}s | {} {}... ", bar, remaining, task_id, action_str);
        let _ = std::io::stdout().flush();
    }
}

/// 可交互倒计时（允许用户提前确认或取消）
pub struct InteractiveCountdown {
    timer: CountdownTimer,
}

impl InteractiveCountdown {
    /// 创建新的交互式倒计时
    pub fn new(total_seconds: u16, default_action: Action) -> Self {
        Self {
            timer: CountdownTimer::new(total_seconds, default_action),
        }
    }

    /// 运行交互式倒计时
    /// 返回用户的选择，如果在倒计时结束前没有输入则返回默认动作
    pub async fn run(&self, _task_id: &str) -> Action {
        // 简化的实现，实际应监听键盘输入
        self.timer.run_silent().await;
        self.timer.default_action()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_countdown_timer() {
        let timer = CountdownTimer::new(1, Action::Execute);
        assert_eq!(timer.remaining_seconds(), 1);
        assert!(!timer.is_cancelled());
        
        timer.run_silent().await;
        // 倒计时完成后不自动标记为取消
        assert!(!timer.is_cancelled());
    }

    #[tokio::test]
    async fn test_countdown_cancel() {
        let timer = CountdownTimer::new(10, Action::Execute);
        
        // 取消倒计时
        timer.cancel();
        assert!(timer.is_cancelled());
        
        // 快速完成
        timer.run_silent().await;
    }

    #[test]
    fn test_action_display() {
        assert_eq!(format!("{:?}", Action::Execute), "Execute");
        assert_eq!(format!("{:?}", Action::Skip), "Skip");
        assert_eq!(format!("{:?}", Action::Abort), "Abort");
    }
}
