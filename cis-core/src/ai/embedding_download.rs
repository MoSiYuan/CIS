//! # Embedding Model Download with Progress
//!
//! 提供带进度显示的模型下载功能。

use std::path::Path;
use std::time::Duration;

use tracing::{error, info};

use crate::error::{CisError, Result};
use crate::storage::unified_paths::UnifiedPaths;

/// 模型文件信息
#[derive(Debug, Clone)]
pub struct ModelFile {
    pub name: &'static str,
    pub url: &'static str,
    pub size_bytes: u64,
    pub path: std::path::PathBuf,
}

/// Nomic Embed Text v1.5 模型
#[allow(clippy::incompatible_msrv)]
pub const NOMIC_EMBED_MODEL: ModelFile = ModelFile {
    name: "nomic-embed-text-v1.5",
    url: "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/onnx/model.onnx",
    size_bytes: 130_000_000, // ~130MB
    path: std::path::PathBuf::new(), // 在运行时设置
};

/// Tokenizer 文件
#[allow(clippy::incompatible_msrv)]
pub const NOMIC_TOKENIZER: ModelFile = ModelFile {
    name: "tokenizer",
    url: "https://huggingface.co/nomic-ai/nomic-embed-text-v1.5/resolve/main/tokenizer.json",
    size_bytes: 500_000, // ~500KB
    path: std::path::PathBuf::new(),
};

/// 获取模型文件路径
pub fn get_model_paths() -> (ModelFile, ModelFile) {
    let model_dir = UnifiedPaths::models_dir().join("nomic-embed-text-v1.5");
    
    let mut model = NOMIC_EMBED_MODEL.clone();
    model.path = model_dir.join("model.onnx");
    
    let mut tokenizer = NOMIC_TOKENIZER.clone();
    tokenizer.path = model_dir.join("tokenizer.json");
    
    (model, tokenizer)
}

/// 检查模型是否已下载
pub fn is_model_downloaded() -> bool {
    let (model, tokenizer) = get_model_paths();
    model.path.exists() && tokenizer.path.exists()
}

/// 同步下载文件（带简单进度）
pub fn download_file_sync(url: &str, dest: &Path, description: &str) -> Result<()> {
    info!("Downloading {} from {}", description, url);
    
    // 创建父目录
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| CisError::io(format!("Failed to create directory: {}", e)))?;
    }
    
    // 创建临时文件
    let temp_path = dest.with_extension("tmp");
    
    // 发送请求
    let response = reqwest::blocking::get(url)
        .map_err(|e| CisError::network(format!("Failed to download: {}", e)))?;
    
    if !response.status().is_success() {
        return Err(CisError::network(format!(
            "Download failed with status: {}",
            response.status()
        )));
    }
    
    // 获取内容
    let content = response.bytes()
        .map_err(|e| CisError::network(format!("Failed to read response: {}", e)))?;
    
    // 写入临时文件
    std::fs::write(&temp_path, &content)
        .map_err(|e| CisError::io(format!("Failed to write file: {}", e)))?;
    
    // 重命名为最终文件
    std::fs::rename(&temp_path, dest)
        .map_err(|e| CisError::io(format!("Failed to rename file: {}", e)))?;
    
    info!("Successfully downloaded {} to {}", description, dest.display());
    
    Ok(())
}

/// 下载模型（带重试）
pub async fn download_model_with_retry(max_retries: u32) -> Result<()> {
    let (model, tokenizer) = get_model_paths();
    
    // 检查是否已存在
    if is_model_downloaded() {
        println!("✓ 向量模型已存在，跳过下载");
        return Ok(());
    }
    
    println!("📥 准备下载向量模型 (Nomic Embed Text v1.5)");
    println!("   模型大小: ~130 MB");
    println!("   Tokenizer: ~500 KB");
    println!("   保存位置: {}", model.path.parent().unwrap().display());
    println!();
    
    // 下载模型文件
    let mut last_error = None;
    for attempt in 1..=max_retries {
        if attempt > 1 {
            println!("\n⏳ 重试下载 (尝试 {}/{})...", attempt, max_retries);
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        
        println!("📥 正在下载模型文件...");
        
        // 使用 blocking 在 spawn_blocking 中执行
        let url = model.url.to_string();
        let path = model.path.clone();
        
        match tokio::task::spawn_blocking(move || {
            download_file_sync(&url, &path, "model.onnx")
        }).await {
            Ok(Ok(_)) => {
                println!("✓ 模型文件下载完成");
                break;
            }
            Ok(Err(e)) => {
                eprintln!("✗ 下载失败: {}", e);
                last_error = Some(e);
                
                // 清理临时文件
                let temp = model.path.with_extension("tmp");
                let _ = std::fs::remove_file(&temp);
            }
            Err(e) => {
                eprintln!("✗ 任务失败: {}", e);
                last_error = Some(CisError::other(format!("Task failed: {}", e)));
            }
        }
    }
    
    if let Some(e) = last_error {
        return Err(e);
    }
    
    // 下载 tokenizer
    println!();
    println!("📥 正在下载 tokenizer...");
    
    let url = tokenizer.url.to_string();
    let path = tokenizer.path.clone();
    
    match tokio::task::spawn_blocking(move || {
        download_file_sync(&url, &path, "tokenizer.json")
    }).await {
        Ok(Ok(_)) => {
            println!("✓ Tokenizer 下载完成");
        }
        Ok(Err(e)) => {
            eprintln!("[WARNING] Tokenizer 下载失败: {}", e);
            eprintln!("   模型可能仍可用，但建议重新下载。");
        }
        Err(e) => {
            eprintln!("[WARNING] Tokenizer 下载失败: {}", e);
        }
    }
    
    println!();
    println!("[OK] 向量模型下载完成！");
    println!("   模型路径: {}", model.path.display());
    
    Ok(())
}

/// 自动下载模型（非交互式）
pub async fn auto_download_model() -> Result<bool> {
    if is_model_downloaded() {
        return Ok(true);
    }
    
    match download_model_with_retry(3).await {
        Ok(_) => Ok(true),
        Err(e) => {
            error!("Failed to auto-download model: {}", e);
            Ok(false)
        }
    }
}

/// 验证模型文件完整性
pub fn verify_model() -> Result<bool> {
    let (model, tokenizer) = get_model_paths();
    
    if !model.path.exists() || !tokenizer.path.exists() {
        return Ok(false);
    }
    
    // 检查文件大小（模型应该 >100MB）
    let model_meta = std::fs::metadata(&model.path)
        .map_err(|e| CisError::io(format!("Failed to read model metadata: {}", e)))?;
    
    if model_meta.len() < 100_000_000 {
        return Ok(false); // 文件太小，可能下载不完整
    }
    
    Ok(true)
}

/// 删除并重新下载模型
pub async fn redownload_model() -> Result<()> {
    let (model, tokenizer) = get_model_paths();
    
    // 删除旧文件
    if model.path.exists() {
        tokio::fs::remove_file(&model.path).await
            .map_err(|e| CisError::io(format!("Failed to remove old model: {}", e)))?;
    }
    
    if tokenizer.path.exists() {
        tokio::fs::remove_file(&tokenizer.path).await
            .map_err(|e| CisError::io(format!("Failed to remove old tokenizer: {}", e)))?;
    }
    
    // 重新下载
    download_model_with_retry(3).await
}

/// 获取模型下载状态
pub fn get_download_status() -> DownloadStatus {
    let (model, tokenizer) = get_model_paths();
    
    DownloadStatus {
        model_exists: model.path.exists(),
        tokenizer_exists: tokenizer.path.exists(),
        model_path: model.path,
        tokenizer_path: tokenizer.path,
        is_complete: is_model_downloaded(),
    }
}

/// 下载状态
#[derive(Debug, Clone)]
pub struct DownloadStatus {
    pub model_exists: bool,
    pub tokenizer_exists: bool,
    pub model_path: std::path::PathBuf,
    pub tokenizer_path: std::path::PathBuf,
    pub is_complete: bool,
}

impl DownloadStatus {
    pub fn print(&self) {
        println!("向量模型状态:");
        println!("  模型文件: {}", if self.model_exists { "✓ 已下载" } else { "✗ 未下载" });
        println!("  Tokenizer: {}", if self.tokenizer_exists { "✓ 已下载" } else { "✗ 未下载" });
        if self.model_exists {
            println!("  路径: {}", self.model_path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_paths() {
        let (model, tokenizer) = get_model_paths();
        assert!(model.path.to_string_lossy().contains("nomic-embed"));
        assert!(tokenizer.path.to_string_lossy().contains("tokenizer"));
    }

    #[test]
    fn test_download_status() {
        let status = get_download_status();
        // 只是测试不 panic
        println!("{:?}", status);
    }
}
