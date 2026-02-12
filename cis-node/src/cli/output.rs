//! # Output Formatting
//!
//! Utilities for formatting command output in different formats.

use serde_json::Value;
use std::io::{self, Write};

use crate::cli::command::CommandOutput;

/// Output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Plain,
    Json,
    Table,
}

/// Format command output for display
pub fn format_output(output: &CommandOutput, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => format_json(output),
        OutputFormat::Table => format_table(output),
        OutputFormat::Plain => format_plain(output),
    }
}

/// Format output as JSON
pub fn format_json(output: &CommandOutput) -> String {
    let value = match output {
        CommandOutput::Success => {
            json!({"status": "success"})
        }
        CommandOutput::Message(msg) => {
            json!({"status": "success", "message": msg})
        }
        CommandOutput::Data(data) => data.clone(),
        CommandOutput::Table(rows) => {
            json!({"status": "success", "rows": rows})
        }
        CommandOutput::Multi(outputs) => {
            json!({"status": "success", "outputs": outputs})
        }
    };

    serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string())
}

/// Format output as a table
pub fn format_table(output: &CommandOutput) -> String {
    match output {
        CommandOutput::Table(rows) => {
            if rows.is_empty() {
                return String::new();
            }

            // Calculate column widths
            let num_cols = rows[0].len();
            let mut widths = vec![0usize; num_cols];

            for row in rows {
                for (i, cell) in row.iter().enumerate() {
                    widths[i] = widths[i].max(cell.len());
                }
            }

            // Format rows
            let mut result = String::new();
            for (row_idx, row) in rows.iter().enumerate() {
                for (i, cell) in row.iter().enumerate() {
                    if i > 0 {
                        result.push_str("  ");
                    }
                    result.push_str(&format!("{:<width$}", cell, width = widths[i]));
                }
                if row_idx < rows.len() - 1 {
                    result.push('\n');
                }
            }

            result
        }
        _ => format_plain(output),
    }
}

/// Format output as plain text
pub fn format_plain(output: &CommandOutput) -> String {
    match output {
        CommandOutput::Success => "Success".to_string(),
        CommandOutput::Message(msg) => msg.clone(),
        CommandOutput::Data(data) => {
            serde_json::to_string_pretty(data).unwrap_or_else(|_| "{}".to_string())
        }
        CommandOutput::Table(rows) => {
            rows.iter()
                .map(|row| row.join("\t"))
                .collect::<Vec<_>>()
                .join("\n")
        }
        CommandOutput::Multi(outputs) => {
            outputs.iter()
                .map(|o| format_plain(o))
                .collect::<Vec<_>>()
                .join("\n\n")
        }
    }
}

/// Print output to stdout
pub fn print_output(output: &CommandOutput, format: OutputFormat) {
    let formatted = format_output(output, format);
    println!("{}", formatted);
}

/// Print output to a writer
pub fn write_output<W: Write>(
    writer: &mut W,
    output: &CommandOutput,
    format: OutputFormat,
) -> io::Result<()> {
    let formatted = format_output(output, format);
    writeln!(writer, "{}", formatted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_json_success() {
        let output = CommandOutput::Success;
        let json = format_json(&output);
        assert!(json.contains("\"status\""));
        assert!(json.contains("\"success\""));
    }

    #[test]
    fn test_format_json_message() {
        let output = CommandOutput::Message("Test message".to_string());
        let json = format_json(&output);
        assert!(json.contains("Test message"));
    }

    #[test]
    fn test_format_table() {
        let output = CommandOutput::Table(vec![
            vec!["Name".to_string(), "Value".to_string()],
            vec!["A".to_string(), "1".to_string()],
            vec!["B".to_string(), "2".to_string()],
        ]);
        let table = format_table(&output);
        assert!(table.contains("Name  Value"));
        assert!(table.contains("A     1"));
    }

    #[test]
    fn test_format_plain() {
        let output = CommandOutput::Message("Test".to_string());
        let plain = format_plain(&output);
        assert_eq!(plain, "Test");
    }
}
