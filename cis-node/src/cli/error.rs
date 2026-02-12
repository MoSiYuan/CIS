//! # Error Handling
//!
//! Utilities for formatting and handling command errors.

use crate::cli::command::CommandError;

/// Format an error for display
pub fn format_error(error: &CommandError) -> String {
    use colored::*;

    let mut output = format!("{} {}\n", "Error:".red().bold(), error.message);

    if !error.suggestions.is_empty() {
        output.push_str(&format!("\n{}", "Suggestions:".cyan().bold()));
        for (i, suggestion) in error.suggestions.iter().enumerate() {
            output.push_str(&format!("\n  {}. {}", i + 1, suggestion));
        }
        output.push('\n');
    }

    if let Some(source) = &error.source {
        output.push_str(&format!("\n{}\n  {}\n", "Details:".yellow(), source));
    }

    output
}

/// Print an error to stderr
pub fn print_error(error: &CommandError) {
    eprintln!("{}", format_error(error));
}

/// Format a warning message
pub fn format_warning(message: &str) -> String {
    use colored::*;
    format!("{} {}", "Warning:".yellow().bold(), message)
}

/// Print a warning to stderr
pub fn print_warning(message: &str) {
    eprintln!("{}", format_warning(message));
}

/// Format a success message
pub fn format_success(message: &str) -> String {
    use colored::*;
    format!("{} {}", "✓".green(), message)
}

/// Print a success message to stdout
pub fn print_success(message: &str) {
    println!("{}", format_success(message));
}

/// Format an info message
pub fn format_info(message: &str) -> String {
    use colored::*;
    format!("{} {}", "Info:".blue().bold(), message)
}

/// Print an info message to stdout
pub fn print_info(message: &str) {
    println!("{}", format_info(message));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_error() {
        let error = CommandError::new("Something went wrong")
            .with_suggestion("Try again")
            .with_suggestion("Check logs");

        let formatted = format_error(&error);
        assert!(formatted.contains("Something went wrong"));
        assert!(formatted.contains("Try again"));
        assert!(formatted.contains("Check logs"));
    }

    #[test]
    fn test_format_warning() {
        let warning = format_warning("This is a warning");
        assert!(warning.contains("Warning"));
        assert!(warning.contains("This is a warning"));
    }

    #[test]
    fn test_format_success() {
        let success = format_success("Operation completed");
        assert!(success.contains("✓"));
        assert!(success.contains("Operation completed"));
    }
}
