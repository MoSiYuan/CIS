//! # Configuration Decryption Tool
//!
//! Command-line utility to decrypt CIS configuration files.
//!
//! ## Usage
//!
//! ```bash
//! # Decrypt and display
//! decrypt-config config.toml.enc
//!
//! # Decrypt and save
//! decrypt-config config.toml.enc -o config.toml
//!
//! # View without decrypting
//! decrypt-config config.toml.enc --view
//! ```
//!
//! ## Environment
//!
//! Set `CIS_CONFIG_ENCRYPTION_KEY` or `CIS_CONFIG_ENCRYPTION_KEY_FILE`
//! to specify the encryption key.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use cis_core::config::ConfigEncryption;
use cis_core::error::CisError;

/// Command-line arguments
struct Args {
    /// Input file path
    input: PathBuf,
    /// Output file path (optional, prints to stdout if not specified)
    output: Option<PathBuf>,
    /// View mode (print to stdout even if output specified)
    view: bool,
}

/// Parse command-line arguments
fn parse_args() -> Result<Args, String> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        return Err(format!(
            "Usage: {} <input-file> [-o output-file] [--view]\n\
             \nEnvironment Variables:\n\
             CIS_CONFIG_ENCRYPTION_KEY  - Encryption key (hex or base64)\n\
             CIS_CONFIG_ENCRYPTION_KEY_FILE - Path to key file\n\
             \nExamples:\n\
             {} config.toml.enc                    # Print to stdout\n\
             {} config.toml.enc -o config.toml     # Save to file\n\
             {} config.toml.enc --view             # View decrypted content",
            args[0], args[0], args[0], args[0]
        ));
    }

    let mut input = None;
    let mut output = None;
    let mut view = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    return Err(format!("{} requires an argument", args[i]));
                }
                output = Some(PathBuf::from(&args[i + 1]));
                i += 2;
            }
            "--view" => {
                view = true;
                i += 1;
            }
            "-h" | "--help" => {
                return Err(format!(
                    "Usage: {} <input-file> [-o output-file] [--view]",
                    args[0]
                ));
            }
            arg => {
                if input.is_some() {
                    return Err(format!("Unexpected argument: {}", arg));
                }
                input = Some(PathBuf::from(arg));
                i += 1;
            }
        }
    }

    let input = input.ok_or_else(|| "No input file specified".to_string())?;

    Ok(Args { input, output, view })
}

/// Main entry point
fn real_main() -> Result<(), CisError> {
    // Parse arguments
    let args = parse_args().map_err(|e| CisError::invalid_input(e))?;

    // Check input file exists
    if !args.input.exists() {
        return Err(CisError::not_found(format!(
            "Input file not found: {}",
            args.input.display()
        )));
    }

    // Read input file
    let content = fs::read_to_string(&args.input).map_err(|e| {
        CisError::configuration(format!(
            "Failed to read input file '{}': {}",
            args.input.display(),
            e
        ))
    })?;

    // Check if file is encrypted
    if !ConfigEncryption::is_encrypted(&content) {
        println!("Warning: Input file does not appear to be encrypted.");
        println!("Content will be displayed as-is.");
    }

    // Initialize encryption
    let encryption = ConfigEncryption::new()?;

    // Decrypt content
    let decrypted = encryption.decrypt_config(&content)?;

    // Output result
    if args.view || args.output.is_none() {
        // Print to stdout
        println!("{}", decrypted);
    }

    // Save to file if specified
    if let Some(output_path) = args.output {
        fs::write(&output_path, decrypted).map_err(|e| {
            CisError::configuration(format!(
                "Failed to write output file '{}': {}",
                output_path.display(),
                e
            ))
        })?;
        eprintln!("Decrypted configuration saved to: {}", output_path.display());
    }

    Ok(())
}

fn main() -> ExitCode {
    // Initialize logger
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .init();

    // Run main function
    match real_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_valid() {
        let args = parse_args(&vec![
            "decrypt-config".to_string(),
            "input.enc".to_string(),
        ])
        .unwrap();

        assert_eq!(args.input, PathBuf::from("input.enc"));
        assert!(args.output.is_none());
        assert!(!args.view);
    }

    #[test]
    fn test_parse_args_with_output() {
        let args = parse_args(&vec![
            "decrypt-config".to_string(),
            "input.enc".to_string(),
            "-o".to_string(),
            "output.toml".to_string(),
        ])
        .unwrap();

        assert_eq!(args.input, PathBuf::from("input.enc"));
        assert_eq!(args.output, Some(PathBuf::from("output.toml")));
        assert!(!args.view);
    }

    #[test]
    fn test_parse_args_with_view() {
        let args = parse_args(&vec![
            "decrypt-config".to_string(),
            "input.enc".to_string(),
            "--view".to_string(),
        ])
        .unwrap();

        assert_eq!(args.input, PathBuf::from("input.enc"));
        assert!(args.output.is_none());
        assert!(args.view);
    }

    #[test]
    fn test_parse_args_no_input() {
        let result = parse_args(&vec!["decrypt-config".to_string()]);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_args_help() {
        let result = parse_args(&vec!["decrypt-config".to_string(), "--help".to_string()]);
        assert!(result.is_err());
    }
}
