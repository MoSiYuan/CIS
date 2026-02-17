//! # Output Formatting Module
//!
//! 处理多种输出格式：JSON、人类可读、流式 SSE、NDJSON

use super::{CliResult, StreamEvent};
use serde::Serialize;

/// 输出格式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[derive(Default)]
pub enum OutputFormat {
    /// JSON 格式
    Json,
    /// 人类可读格式
    #[default]
    Human,
    /// 流式 NDJSON（每行一个 JSON）
    NdJson,
    /// 静默模式（仅输出关键信息）
    Quiet,
}

impl OutputFormat {
    /// 从字符串解析
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "json" => Some(OutputFormat::Json),
            "human" | "text" => Some(OutputFormat::Human),
            "ndjson" | "jsonl" => Some(OutputFormat::NdJson),
            "quiet" | "silent" => Some(OutputFormat::Quiet),
            _ => None,
        }
    }

    /// 获取格式名称
    pub fn as_str(&self) -> &'static str {
        match self {
            OutputFormat::Json => "json",
            OutputFormat::Human => "human",
            OutputFormat::NdJson => "ndjson",
            OutputFormat::Quiet => "quiet",
        }
    }
}


/// 输出管理器
pub struct OutputManager {
    format: OutputFormat,
    buffer: Vec<u8>,
    is_tty: bool,
    use_colors: bool,
    stream_mode: bool,
}

impl OutputManager {
    /// 创建新的输出管理器
    ///
    /// # P1-14 安全修复
    ///
    /// 替换 unmaintained 的 atty crate 为 std::io::IsTerminal (Rust 1.70+)
    pub fn new(format: OutputFormat) -> Self {
        let is_tty = std::io::stdout().is_terminal();
        let use_colors = is_tty && format == OutputFormat::Human;

        Self {
            format,
            buffer: Vec::new(),
            is_tty,
            use_colors,
            stream_mode: false,
        }
    }

    /// 启用流式模式
    pub fn with_stream_mode(mut self, enabled: bool) -> Self {
        self.stream_mode = enabled;
        self
    }

    /// 禁用颜色
    pub fn no_color(mut self) -> Self {
        self.use_colors = false;
        self
    }

    /// 输出成功结果
    pub fn output<T: Serialize>(&mut self, result: &CliResult<T>) {
        match self.format {
            OutputFormat::Json => {
                let json = result.to_json();
                self.println(&json);
            }
            OutputFormat::Human => {
                self.output_human(result);
            }
            OutputFormat::NdJson => {
                // NDJSON 模式下仍然输出完整 JSON
                let json = result.to_json();
                self.println(&json);
            }
            OutputFormat::Quiet => {
                // 静默模式仅在有错误时输出
                if result.code != 0 {
                    if let Some(ref error) = result.error {
                        self.println(&format!("Error: {}", error.message));
                    }
                }
            }
        }
    }

    /// 输出流式事件
    pub fn stream_event(&mut self, event: &StreamEvent) {
        if self.stream_mode {
            if self.format == OutputFormat::NdJson {
                // NDJSON 格式
                let json = serde_json::to_string(event).unwrap_or_default();
                self.println(&json);
            } else {
                // SSE 格式
                let sse = event.to_sse();
                self.print(&sse);
            }
            self.flush();
        } else {
            // 非流式模式下，只输出关键状态
            match event {
                StreamEvent::Status { state: _, progress } => {
                    if self.format == OutputFormat::Human {
                        self.print_progress(*progress);
                    }
                }
                StreamEvent::Error { message, code } => {
                    self.eprintln(&format!("Error ({}): {}", code, message));
                }
                _ => {}
            }
        }
    }

    /// 人类可读格式输出
    fn output_human<T: Serialize>(&mut self, result: &CliResult<T>) {
        use colored::Colorize;

        if result.code == 0 {
            let check = if self.use_colors {
                "✓".green().to_string()
            } else {
                "✓".to_string()
            };
            self.println(&format!("{} Success", check));

            if let Some(ref data) = result.data {
                if let Ok(pretty) = serde_json::to_string_pretty(data) {
                    self.println(&pretty);
                }
            }
        } else {
            let cross = if self.use_colors {
                "✗".red().to_string()
            } else {
                "✗".to_string()
            };
            self.println(&format!("{} Error (code: {})", cross, result.code));

            if let Some(ref error) = result.error {
                let error_type = if self.use_colors {
                    error.error_type.clone().red().to_string()
                } else {
                    error.error_type.clone()
                };
                self.println(&format!("  Type: {}", error_type));
                self.println(&format!("  Message: {}", error.message));

                if let Some(ref suggestion) = error.suggestion {
                    let sug = if self.use_colors {
                        format!("  Suggestion: {}", suggestion).yellow().to_string()
                    } else {
                        format!("  Suggestion: {}", suggestion)
                    };
                    self.println(&sug);
                }

                if error.auto_fixable {
                    if let Some(ref cmd) = error.fix_command {
                        let fix = if self.use_colors {
                            format!("  Fix: {}", cmd).cyan().to_string()
                        } else {
                            format!("  Fix: {}", cmd)
                        };
                        self.println(&fix);
                    }
                }
            }
        }

        // 元数据
        if self.is_tty {
            self.println("");
            let meta = if self.use_colors {
                format!(
                    "  {}: {} | v{}",
                    result.meta.command.dimmed(),
                    result.meta.timestamp.dimmed(),
                    result.meta.version.dimmed()
                )
            } else {
                format!(
                    "  {}: {} | v{}",
                    result.meta.command, result.meta.timestamp, result.meta.version
                )
            };
            self.println(&meta);
        }
    }

    /// 打印进度条
    fn print_progress(&mut self, progress: f32) {
        let width = 40;
        let filled = (progress * width as f32) as usize;
        let bar: String = "=".repeat(filled);
        let empty: String = "-".repeat(width - filled);

        if self.use_colors {
            use colored::Colorize;
            self.print(&format!("\r[{}{}] {:.1}%", bar.green(), empty.dimmed(), progress * 100.0));
        } else {
            self.print(&format!("\r[{}{}] {:.1}%", bar, empty, progress * 100.0));
        }

        if progress >= 1.0 {
            self.println("");
        }
        self.flush();
    }

    /// 打印字符串
    fn print(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
    }

    /// 打印字符串并换行
    fn println(&mut self, s: &str) {
        self.buffer.extend_from_slice(s.as_bytes());
        self.buffer.push(b'\n');
        self.flush();
    }

    /// 打印到 stderr
    fn eprintln(&mut self, s: &str) {
        eprintln!("{}", s);
    }

    /// 刷新缓冲区
    fn flush(&mut self) {
        if !self.buffer.is_empty() {
            print!("{}", String::from_utf8_lossy(&self.buffer));
            self.buffer.clear();
        }
    }
}

impl Drop for OutputManager {
    fn drop(&mut self) {
        self.flush();
    }
}

/// 表格输出（用于列表显示）
pub struct TableOutput {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    max_widths: Vec<usize>,
}

impl TableOutput {
    /// 创建新表格
    pub fn new(headers: Vec<String>) -> Self {
        let max_widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
        Self {
            headers,
            rows: Vec::new(),
            max_widths,
        }
    }

    /// 添加行
    pub fn add_row(&mut self, row: Vec<String>) {
        for (i, cell) in row.iter().enumerate() {
            if i < self.max_widths.len() {
                self.max_widths[i] = self.max_widths[i].max(cell.len());
            }
        }
        self.rows.push(row);
    }

    /// 渲染为字符串
    pub fn render(&self) -> String {
        let mut output = String::new();

        // 表头
        output.push_str(&self.render_row(&self.headers, true));
        output.push_str(&self.render_separator());

        // 数据行
        for row in &self.rows {
            output.push_str(&self.render_row(row, false));
        }

        output
    }

    fn render_row(&self, row: &[String], is_header: bool) -> String {
        use colored::Colorize;

        let cells: Vec<String> = row
            .iter()
            .enumerate()
            .map(|(i, cell)| {
                let width = self.max_widths.get(i).copied().unwrap_or(0);
                let padded = format!("{:width$}", cell, width = width);
                if is_header {
                    padded.bold().to_string()
                } else {
                    padded
                }
            })
            .collect();

        format!("| {} |\n", cells.join(" | "))
    }

    fn render_separator(&self) -> String {
        let sep: Vec<String> = self
            .max_widths
            .iter()
            .map(|w| "-".repeat(*w + 2))
            .collect();
        format!("+{}+\n", sep.join("+"))
    }
}

/// 树形输出（用于层级结构）
pub struct TreeOutput {
    lines: Vec<String>,
}

impl TreeOutput {
    /// 创建新树
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// 添加节点
    pub fn add_node(&mut self, label: &str, depth: usize, is_last: bool) {
        use colored::Colorize;

        let indent = "  ".repeat(depth);
        let branch = if depth == 0 {
            ""
        } else if is_last {
            "└─"
        } else {
            "├─"
        };

        let line = format!("{}{} {}", indent, branch.dimmed(), label);
        self.lines.push(line);
    }

    /// 渲染为字符串
    pub fn render(&self) -> String {
        self.lines.join("\n")
    }
}

impl Default for TreeOutput {
    fn default() -> Self {
        Self::new()
    }
}

/// 交互式确认提示
pub fn confirm(message: &str, default: bool) -> bool {
    use colored::Colorize;
    use std::io::{stdin, stdout, Write};

    let prompt = if default {
        "[Y/n]"
    } else {
        "[y/N]"
    };

    print!("{} {} ", message.cyan(), prompt.dimmed());
    stdout().flush().unwrap();

    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return default;
    }

    let input = input.trim().to_lowercase();
    match input.as_str() {
        "y" | "yes" => true,
        "n" | "no" => false,
        "" => default,
        _ => default,
    }
}

/// 交互式选择
pub fn select<T: AsRef<str>>(message: &str, options: &[T], default: usize) -> Option<usize> {
    use colored::Colorize;
    use std::io::{stdin, stdout, Write};

    println!("{}", message.cyan());

    for (i, option) in options.iter().enumerate() {
        let marker = if i == default { "●" } else { "○" };
        let num = format!("{}.", i + 1).dimmed();
        println!("  {} {} {}", marker.dimmed(), num, option.as_ref());
    }

    print!("Select [{}]: ", default + 1);
    stdout().flush().unwrap();

    let mut input = String::new();
    if stdin().read_line(&mut input).is_err() {
        return Some(default);
    }

    let input = input.trim();
    if input.is_empty() {
        return Some(default);
    }

    input.parse::<usize>().ok().map(|n| n.saturating_sub(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_output_format_from_str() {
        assert_eq!(OutputFormat::from_str("json"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("JSON"), Some(OutputFormat::Json));
        assert_eq!(OutputFormat::from_str("human"), Some(OutputFormat::Human));
        assert_eq!(OutputFormat::from_str("ndjson"), Some(OutputFormat::NdJson));
        assert_eq!(OutputFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_table_output() {
        let mut table = TableOutput::new(vec!["Name".to_string(), "Status".to_string()]);
        table.add_row(vec!["test1".to_string(), "running".to_string()]);
        table.add_row(vec!["test2-longer".to_string(), "stopped".to_string()]);

        let rendered = table.render();
        assert!(rendered.contains("Name"));
        assert!(rendered.contains("test1"));
        assert!(rendered.contains("running"));
    }

    #[test]
    fn test_tree_output() {
        let mut tree = TreeOutput::new();
        tree.add_node("root", 0, false);
        tree.add_node("child1", 1, false);
        tree.add_node("child2", 1, true);

        let rendered = tree.render();
        assert!(rendered.contains("root"));
        assert!(rendered.contains("child1"));
        assert!(rendered.contains("child2"));
    }
}
