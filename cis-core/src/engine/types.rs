//! # Engine Scanner Types
//!
//! Core data structures for engine scanning and injection detection.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Supported game engine types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum EngineType {
    /// Unreal Engine 5.7+
    #[serde(rename = "unreal")]
    Unreal5_7,

    /// Unity 2022 LTS
    #[serde(rename = "unity2022")]
    Unity2022,

    /// Godot 4.x
    #[serde(rename = "godot4")]
    Godot4,

    /// Custom/unknown engine
    #[serde(rename = "custom")]
    Custom(String),
}

impl EngineType {
    /// Get the display name for this engine type
    pub fn display_name(&self) -> &str {
        match self {
            EngineType::Unreal5_7 => "Unreal Engine 5.7+",
            EngineType::Unity2022 => "Unity 2022 LTS",
            EngineType::Godot4 => "Godot 4.x",
            EngineType::Custom(name) => name,
        }
    }

    /// Get common file extensions for this engine
    pub fn source_extensions(&self) -> &'static [&'static str] {
        match self {
            EngineType::Unreal5_7 => &["h", "cpp", "uc", "uproject", "uplugin"],
            EngineType::Unity2022 => &["cs", "unity", "asset", "meta"],
            EngineType::Godot4 => &["gd", "tscn", "tres", "cs"],
            EngineType::Custom(_) => &["*"],
        }
    }

    /// Get config file patterns for this engine
    pub fn config_patterns(&self) -> &'static [&'static str] {
        match self {
            EngineType::Unreal5_7 => &[
                "Config/Unreal.ini",
                "Config/DefaultEngine.ini",
                "*.uproject",
            ],
            EngineType::Unity2022 => &[
                "ProjectSettings/ProjectSettings.asset",
                "Packages/manifest.json",
                "*.unity",
            ],
            EngineType::Godot4 => &[
                "project.godot",
                "export_presets.cfg",
            ],
            EngineType::Custom(_) => &["*"],
        }
    }
}

impl std::fmt::Display for EngineType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Engine detection information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineInfo {
    /// Detected engine type
    pub engine_type: EngineType,

    /// Optional version string
    pub version: Option<String>,

    /// Root directory of the engine/project
    pub root_path: PathBuf,

    /// Discovered configuration files
    pub config_files: Vec<PathBuf>,

    /// Additional metadata
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

impl EngineInfo {
    /// Create a new engine info
    pub fn new(engine_type: EngineType, root_path: PathBuf) -> Self {
        Self {
            engine_type,
            version: None,
            root_path,
            config_files: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    /// Add a configuration file
    pub fn add_config_file(&mut self, path: PathBuf) {
        if !self.config_files.contains(&path) {
            self.config_files.push(path);
        }
    }

    /// Set version
    pub fn with_version(mut self, version: String) -> Self {
        self.version = Some(version);
        self
    }

    /// Add metadata
    pub fn add_metadata(&mut self, key: String, value: String) {
        self.metadata.insert(key, value);
    }
}

/// Types of code injection points
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum InjectionType {
    /// Function/Method call injection
    FunctionCall,

    /// Variable/Property assignment
    VariableAssignment,

    /// Resource/Asset loading
    ResourceLoad,

    /// Event/Callback hook
    EventHook,

    /// Custom/Pattern-based injection
    CustomHook,

    /// Constructor/Destructor injection
    Constructor,
}

impl InjectionType {
    /// Get display description
    pub fn description(&self) -> &str {
        match self {
            InjectionType::FunctionCall => "Function/Method Call",
            InjectionType::VariableAssignment => "Variable/Property Assignment",
            InjectionType::ResourceLoad => "Resource/Asset Loading",
            InjectionType::EventHook => "Event/Callback Hook",
            InjectionType::CustomHook => "Custom Pattern Hook",
            InjectionType::Constructor => "Constructor/Destructor",
        }
    }
}

impl std::fmt::Display for InjectionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Injection pattern definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionPattern {
    /// Pattern name/identifier
    pub name: String,

    /// Regex pattern for matching
    pub pattern: String,

    /// Type of injection this pattern detects
    pub injection_type: InjectionType,

    /// Confidence threshold (0.0 - 1.0)
    #[serde(default = "default_confidence")]
    pub confidence: f32,

    /// Language/Engine this pattern applies to
    #[serde(default)]
    pub languages: Vec<String>,

    /// Description of what this pattern detects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn default_confidence() -> f32 {
    0.8
}

impl InjectionPattern {
    /// Create a new injection pattern
    pub fn new(
        name: String,
        pattern: String,
        injection_type: InjectionType,
    ) -> Self {
        Self {
            name,
            pattern,
            injection_type,
            confidence: default_confidence(),
            languages: Vec::new(),
            description: None,
        }
    }

    /// Set confidence threshold
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Add applicable language
    pub fn with_language(mut self, language: String) -> Self {
        self.languages.push(language);
        self
    }

    /// Set description
    pub fn with_description(mut self, description: String) -> Self {
        self.description = Some(description);
        self
    }
}

/// A detected injection location
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectibleLocation {
    /// File containing the injection point
    pub file_path: PathBuf,

    /// Line number (1-indexed)
    pub line_number: usize,

    /// Column number (1-indexed, optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column_number: Option<usize>,

    /// Type of injection detected
    pub injection_type: InjectionType,

    /// Matched code snippet
    pub code_snippet: String,

    /// Pattern that matched
    pub pattern_name: String,

    /// Confidence score (0.0 - 1.0)
    pub confidence: f32,

    /// Additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl InjectibleLocation {
    /// Create a new injection location
    pub fn new(
        file_path: PathBuf,
        line_number: usize,
        injection_type: InjectionType,
        code_snippet: String,
        pattern_name: String,
    ) -> Self {
        Self {
            file_path,
            line_number,
            column_number: None,
            injection_type,
            code_snippet,
            pattern_name,
            confidence: 0.8,
            context: None,
        }
    }

    /// Set confidence score
    pub fn with_confidence(mut self, confidence: f32) -> Self {
        self.confidence = confidence.clamp(0.0, 1.0);
        self
    }

    /// Set column number
    pub fn with_column(mut self, column: usize) -> Self {
        self.column_number = Some(column);
        self
    }

    /// Set context
    pub fn with_context(mut self, context: String) -> Self {
        self.context = Some(context);
        self
    }
}

/// Scan result summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Detected engine information
    pub engine: Option<EngineInfo>,

    /// All discovered injection points
    pub locations: Vec<InjectibleLocation>,

    /// Files scanned
    pub files_scanned: usize,

    /// Total lines scanned
    pub lines_scanned: usize,

    /// Scan duration in milliseconds
    pub duration_ms: u64,

    /// Errors encountered during scan
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub errors: Vec<String>,
}

impl ScanResult {
    /// Create empty scan result
    pub fn new() -> Self {
        Self {
            engine: None,
            locations: Vec::new(),
            files_scanned: 0,
            lines_scanned: 0,
            duration_ms: 0,
            errors: Vec::new(),
        }
    }

    /// Get count of injection points by type
    pub fn count_by_type(&self, injection_type: &InjectionType) -> usize {
        self.locations
            .iter()
            .filter(|l| &l.injection_type == injection_type)
            .count()
    }

    /// Get high confidence locations (> 0.8)
    pub fn high_confidence_locations(&self) -> Vec<&InjectibleLocation> {
        self.locations
            .iter()
            .filter(|l| l.confidence > 0.8)
            .collect()
    }
}

impl Default for ScanResult {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_type_display() {
        assert_eq!(EngineType::Unreal5_7.display_name(), "Unreal Engine 5.7+");
        assert_eq!(EngineType::Unity2022.display_name(), "Unity 2022 LTS");
        assert_eq!(EngineType::Godot4.display_name(), "Godot 4.x");
    }

    #[test]
    fn test_engine_type_extensions() {
        let unreal_exts = EngineType::Unreal5_7.source_extensions();
        assert!(unreal_exts.contains(&"h"));
        assert!(unreal_exts.contains(&"cpp"));

        let unity_exts = EngineType::Unity2022.source_extensions();
        assert!(unity_exts.contains(&"cs"));
    }

    #[test]
    fn test_injection_pattern_builder() {
        let pattern = InjectionPattern::new(
            "Test Pattern".to_string(),
            r"test\(".to_string(),
            InjectionType::FunctionCall,
        )
        .with_confidence(0.9)
        .with_language("C++".to_string())
        .with_description("Test pattern".to_string());

        assert_eq!(pattern.name, "Test Pattern");
        assert_eq!(pattern.confidence, 0.9);
        assert_eq!(pattern.languages.len(), 1);
        assert!(pattern.description.is_some());
    }

    #[test]
    fn test_injectible_location_builder() {
        let location = InjectibleLocation::new(
            PathBuf::from("/test/file.cpp"),
            42,
            InjectionType::FunctionCall,
            "test()".to_string(),
            "Test Pattern".to_string(),
        )
        .with_confidence(0.95)
        .with_column(10)
        .with_context("Function call".to_string());

        assert_eq!(location.line_number, 42);
        assert_eq!(location.confidence, 0.95);
        assert_eq!(location.column_number, Some(10));
    }

    #[test]
    fn test_scan_result_counts() {
        let mut result = ScanResult::new();

        result.locations.push(
            InjectibleLocation::new(
                PathBuf::from("/test/file.cpp"),
                1,
                InjectionType::FunctionCall,
                "func()".to_string(),
                "pattern".to_string(),
            )
            .with_confidence(0.9),
        );

        result.locations.push(
            InjectibleLocation::new(
                PathBuf::from("/test/file.cpp"),
                2,
                InjectionType::VariableAssignment,
                "x = 1".to_string(),
                "pattern".to_string(),
            )
            .with_confidence(0.7),
        );

        assert_eq!(result.count_by_type(&InjectionType::FunctionCall), 1);
        assert_eq!(result.high_confidence_locations().len(), 1);
    }
}
