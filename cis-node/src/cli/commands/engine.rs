//! # Engine Command
//!
//! Engine code scanning and injection commands for CIS.

use crate::cli::{CommandContext, CommandOutput, CommandError};
use colored::Colorize;
use std::path::Path;
use std::fs;
use std::collections::HashMap;

/// Engine command for code scanning and injection
pub struct EngineCommand;

/// Engine type detection result
#[derive(Debug, Clone)]
pub enum EngineType {
    Unreal5_7,
    Unreal5_6,
    Unity2022,
    Unity2021,
    Godot4,
    Godot3,
    Unknown,
}

/// Code injection location
#[derive(Debug, Clone)]
pub struct InjectionLocation {
    pub file_path: String,
    pub line_number: usize,
    pub injection_type: String,
    pub description: String,
    pub confidence: f64,
}

/// Scan result
#[derive(Debug, Clone)]
pub struct ScanResult {
    pub engine_type: EngineType,
    pub engine_version: Option<String>,
    pub injection_locations: Vec<InjectionLocation>,
    pub total_files_scanned: usize,
    pub total_candidates: usize,
}

impl EngineCommand {
    /// Scan directory for engine code
    pub async fn execute_scan(
        &self,
        ctx: &CommandContext,
        directory: String,
        engine_type: Option<String>,
        output_file: Option<String>,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose(&format!("Scanning directory: {}", directory));

        let path = Path::new(&directory);
        if !path.exists() {
            return Err(CommandError::not_found("Directory", &directory));
        }

        // Detect engine type if not specified
        let detected_engine = if let Some(eng_type) = engine_type {
            Self::parse_engine_type(&eng_type)
        } else {
            Self::detect_engine_type(path)
        };

        let engine_name = match &detected_engine {
            EngineType::Unreal5_7 => "Unreal Engine 5.7",
            EngineType::Unreal5_6 => "Unreal Engine 5.6",
            EngineType::Unity2022 => "Unity 2022",
            EngineType::Unity2021 => "Unity 2021",
            EngineType::Godot4 => "Godot 4.x",
            EngineType::Godot3 => "Godot 3.x",
            EngineType::Unknown => "Unknown Engine",
        };

        ctx.verbose(&format!("Detected engine: {}", engine_name));

        // Scan for injection locations
        let result = self.scan_directory(path, &detected_engine).await
            .map_err(|e| CommandError::custom(format!("Scan failed: {}", e)))?;

        // Format output
        let output = if ctx.is_json() {
            serde_json::to_string_pretty(&result).unwrap_or_default()
        } else {
            self.format_scan_result(&result, engine_name)
        };

        // Write to file if requested
        if let Some(output_path) = output_file {
            fs::write(&output_path, &output)
                .map_err(|e| CommandError::custom(format!("Failed to write output: {}", e)))?;
            return Ok(CommandOutput::Message(
                format!("{} Scan complete: {} locations written to {}", "✓".green(), result.injection_locations.len(), output_path)
            ));
        }

        Ok(CommandOutput::Message(output))
    }

    /// Generate injection report
    pub async fn execute_report(
        &self,
        ctx: &CommandContext,
        scan_result: String,
        output_format: String,
    ) -> Result<CommandOutput, CommandError> {
        ctx.verbose("Generating injection report...");

        // Read scan result
        let result_json = fs::read_to_string(&scan_result)
            .map_err(|e| CommandError::custom(format!("Failed to read scan result: {}", e)))?;

        let result: ScanResult = serde_json::from_str(&result_json)
            .map_err(|e| CommandError::custom(format!("Invalid scan result: {}", e)))?;

        let report = match output_format.to_lowercase().as_str() {
            "markdown" | "md" => self.generate_markdown_report(&result),
            "json" => serde_json::to_string_pretty(&result)
                .map_err(|e| CommandError::custom(format!("Failed to serialize: {}", e)))?,
            "csv" => self.generate_csv_report(&result),
            _ => return Err(CommandError::invalid_argument("format", &output_format)),
        };

        Ok(CommandOutput::Message(report))
    }

    /// Show supported engine types
    pub async fn execute_list_engines(&self) -> Result<CommandOutput, CommandError> {
        let engines = vec![
            ("Unreal5.7", "Unreal Engine 5.7", "*.uproject, Engine/Source/*.cpp"),
            ("Unreal5.6", "Unreal Engine 5.6", "*.uproject, Engine/Source/*.cpp"),
            ("Unity2022", "Unity 2022", "Assets/, ProjectSettings/"),
            ("Unity2021", "Unity 2021", "Assets/, ProjectSettings/"),
            ("Godot4", "Godot Engine 4.x", "project.godot, *.gd"),
            ("Godot3", "Godot Engine 3.x", "engine.gd, *.gd"),
        ];

        let mut output = String::from("Supported Engine Types\n\n");

        for (id, name, patterns) in engines {
            output.push_str(&format!(
                "• {} - {}\n  Detection: {}\n",
                id.cyan().bold(),
                name,
                patterns.dim()
            ));
        }

        Ok(CommandOutput::Message(output))
    }

    /// Scan directory for injection locations
    async fn scan_directory(&self, path: &Path, engine_type: &EngineType) -> Result<ScanResult, std::io::Error> {
        let mut locations = Vec::new();
        let mut files_scanned = 0;

        // Recursively scan files
        let entries = walkdir::WalkDir::new(path)
            .max_depth(10)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file());

        for entry in entries {
            let file_path = entry.path();
            let file_name = file_path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            // Skip certain directories
            if file_path.to_string_lossy().contains("DerivedDataCache")
                || file_path.to_string_lossy().contains("Binaries")
                || file_path.to_string_lossy().contains(".git") {
                continue;
            }

            // Check file based on engine type
            let file_locations = self.scan_file(file_path, engine_type).await?;
            locations.extend(file_locations);
            files_scanned += 1;
        }

        Ok(ScanResult {
            engine_type: engine_type.clone(),
            engine_version: None,
            injection_locations: locations,
            total_files_scanned: files_scanned,
            total_candidates: locations.len(),
        })
    }

    /// Scan single file for injection locations
    async fn scan_file(&self, file_path: &Path, engine_type: &EngineType) -> Result<Vec<InjectionLocation>, std::io::Error> {
        let content = fs::read_to_string(file_path)?;
        let mut locations = Vec::new();

        let file_name = file_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        match engine_type {
            EngineType::Unreal5_7 | EngineType::Unreal5_6 => {
                locations.extend(self.scan_unreal_file(file_path, &content));
            }
            EngineType::Unity2022 | EngineType::Unity2021 => {
                locations.extend(self.scan_unity_file(file_path, &content));
            }
            EngineType::Godot4 | EngineType::Godot3 => {
                locations.extend(self.scan_godot_file(file_path, &content));
            }
            EngineType::Unknown => {
                // No specific patterns
            }
        }

        Ok(locations)
    }

    /// Scan Unreal Engine file
    fn scan_unreal_file(&self, file_path: &Path, content: &str) -> Vec<InjectionLocation> {
        let mut locations = Vec::new();
        let file_name = file_path.to_string_lossy();

        // Look for AActor/APawn classes
        for (idx, line) in content.lines().enumerate() {
            if line.contains("class ") && line.contains(" : public AActor") {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "ActorClass".to_string(),
                    description: "AActor subclass - can inject BeginPlay() logic".to_string(),
                    confidence: 0.9,
                });
            }

            if line.contains("UFUNCTION") && line.contains("BlueprintCallable") {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "UFunction".to_string(),
                    description: "Blueprint callable function - can inject AI logic".to_string(),
                    confidence: 0.85,
                });
            }
        }

        locations
    }

    /// Scan Unity file
    fn scan_unity_file(&self, file_path: &Path, content: &str) -> Vec<InjectionLocation> {
        let mut locations = Vec::new();
        let file_name = file_path.to_string_lossy();

        if !file_name.ends_with(".cs") {
            return locations;
        }

        for (idx, line) in content.lines().enumerate() {
            if line.contains(" : MonoBehaviour") {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "MonoBehaviour".to_string(),
                    description: "Unity component - can inject Start/Update logic".to_string(),
                    confidence: 0.95,
                });
            }

            if line.contains("[Command]") || line.contains("[Rpc]") {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "NetworkCommand".to_string(),
                    description: "Network command - can inject AI decision logic".to_string(),
                    confidence: 0.88,
                });
            }
        }

        locations
    }

    /// Scan Godot file
    fn scan_godot_file(&self, file_path: &Path, content: &str) -> Vec<InjectionLocation> {
        let mut locations = Vec::new();
        let file_name = file_path.to_string_lossy();

        if !file_name.ends_with(".gd") {
            return locations;
        }

        for (idx, line) in content.lines().enumerate() {
            if line.contains("extends ") && (line.contains("Node") || line.contains("Resource")) {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "GodotNode".to_string(),
                    description: "Godot node - can inject _ready/_process logic".to_string(),
                    confidence: 0.92,
                });
            }

            if line.contains("func _") {
                locations.push(InjectionLocation {
                    file_path: file_name.clone(),
                    line_number: idx + 1,
                    injection_type: "GodotCallback".to_string(),
                    description: "Godot virtual function - can inject AI logic".to_string(),
                    confidence: 0.80,
                });
            }
        }

        locations
    }

    /// Detect engine type from directory
    fn detect_engine_type(path: &Path) -> EngineType {
        // Check for Unreal Engine
        if path.join("*.uproject").exists()
            || path.join("Engine/Source").exists() {
            // Try to determine version
            return EngineType::Unreal5_7; // Default to latest
        }

        // Check for Unity
        if path.join("Assets").exists()
            || path.join("ProjectSettings").exists()
            || path.join("Library").exists() {
            // Could read ProjectVersion.txt to determine exact version
            return EngineType::Unity2022;
        }

        // Check for Godot
        if path.join("project.godot").exists() {
            return EngineType::Godot4;
        }

        if path.join("engine.gd").exists() {
            return EngineType::Godot3;
        }

        EngineType::Unknown
    }

    /// Parse engine type from string
    fn parse_engine_type(s: &str) -> EngineType {
        match s.to_lowercase().as_str() {
            "unreal5.7" | "unreal5_7" | "unreal" => EngineType::Unreal5_7,
            "unreal5.6" | "unreal5_6" => EngineType::Unreal5_6,
            "unity2022" | "unity" => EngineType::Unity2022,
            "unity2021" => EngineType::Unity2021,
            "godot4" | "godot" => EngineType::Godot4,
            "godot3" => EngineType::Godot3,
            _ => EngineType::Unknown,
        }
    }

    /// Format scan result for display
    fn format_scan_result(&self, result: &ScanResult, engine_name: &str) -> String {
        let mut output = format!(
            "{} Engine Scan Results\n\
             {}\n\
             \n\
             Engine: {}\n\
             Files Scanned: {}\n\
             Injection Locations Found: {}\n\
             \n",
            "═".repeat(40),
            "═".repeat(40),
            engine_name.bold(),
            result.total_files_scanned,
            result.total_candidates
        );

        if result.injection_locations.is_empty() {
            output.push_str("No injection locations found.\n");
            return output;
        }

        output.push_str(&format!("\n{}\n", "Injection Locations".bold()));
        for (idx, loc) in result.injection_locations.iter().enumerate() {
            output.push_str(&format!(
                "\n{}. {} ({}% confidence)\n\
                 File: {}\n\
                 Line: {}\n\
                 Type: {}\n\
                 Description: {}\n",
                idx + 1,
                "▸".cyan(),
                format!("{:.0}", loc.confidence * 100.0).green(),
                loc.file_path.dim(),
                loc.line_number,
                loc.injection_type.yellow(),
                loc.description,
            ));
        }

        output
    }

    /// Generate markdown report
    fn generate_markdown_report(&self, result: &ScanResult) -> String {
        let mut output = String::from("# Engine Injection Report\n\n");

        output.push_str(&format!(
            "- **Engine**: {:?}\n\
             - **Files Scanned**: {}\n\
             - **Injection Locations**: {}\n\n",
            result.engine_type,
            result.total_files_scanned,
            result.total_candidates
        ));

        output.push_str("## Injection Locations\n\n");

        for (idx, loc) in result.injection_locations.iter().enumerate() {
            output.push_str(&format!(
                "### {}. {}\n\n\
                 - **File**: `{}`\n\
                 - **Line**: {}\n\
                 - **Type**: {}\n\
                 - **Confidence**: {:.0}%\n\
                 - **Description**: {}\n\n",
                idx + 1,
                loc.injection_type,
                loc.file_path,
                loc.line_number,
                loc.injection_type,
                loc.confidence * 100.0,
                loc.description
            ));
        }

        output
    }

    /// Generate CSV report
    fn generate_csv_report(&self, result: &ScanResult) -> String {
        let mut output = String::from("File,Line,Type,Confidence,Description\n");

        for loc in &result.injection_locations {
            output.push_str(&format!(
                "{},{},{},{:.2},{}\n",
                loc.file_path,
                loc.line_number,
                loc.injection_type,
                loc.confidence,
                loc.description.replace(",", ";")
            ));
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_engine_type() {
        assert!(matches!(EngineCommand::parse_engine_type("unreal"), EngineType::Unreal5_7));
        assert!(matches!(EngineCommand::parse_engine_type("unity"), EngineType::Unity2022));
        assert!(matches!(EngineCommand::parse_engine_type("godot4"), EngineType::Godot4));
        assert!(matches!(EngineCommand::parse_engine_type("godot3"), EngineType::Godot3));
        assert!(matches!(EngineCommand::parse_engine_type("invalid"), EngineType::Unknown));
    }

    #[test]
    fn test_scan_unreal_file() {
        let cmd = EngineCommand;
        let content = r#"
            UCLASS()
            class AMyActor : public AActor {
                GENERATED_BODY()

                UFUNCTION(BlueprintCallable)
                void MyFunction();
            };
        "#;

        let path = Path::new("/fake/MyActor.cpp");
        let locations = cmd.scan_unreal_file(path, content);

        assert!(!locations.is_empty());
        assert!(locations.iter().any(|l| l.injection_type == "ActorClass"));
        assert!(locations.iter().any(|l| l.injection_type == "UFunction"));
    }

    #[test]
    fn test_scan_unity_file() {
        let cmd = EngineCommand;
        let content = r#"
            using UnityEngine;

            public class MyScript : MonoBehaviour {
                void Start() { }

                [Command]
                void MyRpc() { }
            }
        "#;

        let path = Path::new("/fake/MyScript.cs");
        let locations = cmd.scan_unity_file(path, content);

        assert!(!locations.is_empty());
        assert!(locations.iter().any(|l| l.injection_type == "MonoBehaviour"));
        assert!(locations.iter().any(|l| l.injection_type == "NetworkCommand"));
    }

    #[test]
    fn test_scan_godot_file() {
        let cmd = EngineCommand;
        let content = r#"
            extends Node

            func _ready():
                pass

            func _process(delta):
                pass
        "#;

        let path = Path::new("/fake/MyScript.gd");
        let locations = cmd.scan_godot_file(path, content);

        assert!(!locations.is_empty());
        assert!(locations.iter().any(|l| l.injection_type == "GodotNode"));
        assert!(locations.iter().any(|l| l.injection_type == "GodotCallback"));
    }
}
