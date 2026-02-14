//! # Engine Scanner
//!
//! Main scanning implementation for detecting engines and injection points.

use super::patterns::PatternLibrary;
use super::types::*;
use crate::error::{CisError, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tokio::fs;
use walkdir::WalkDir;

/// Engine scanner for detecting game engines and injection points
pub struct EngineScanner {
    /// Pattern library for injection detection
    library: PatternLibrary,

    /// Maximum file size to scan (in bytes)
    max_file_size: usize,

    /// Whether to follow symlinks
    follow_symlinks: bool,

    /// Compiled regex cache
    regex_cache: std::collections::HashMap<String, Regex>,
}

impl EngineScanner {
    /// Create a new engine scanner with default settings
    pub fn new() -> Self {
        Self {
            library: PatternLibrary::new(),
            max_file_size: 10 * 1024 * 1024, // 10 MB
            follow_symlinks: false,
            regex_cache: std::collections::HashMap::new(),
        }
    }

    /// Create scanner with custom pattern library
    pub fn with_library(library: PatternLibrary) -> Self {
        Self {
            library,
            max_file_size: 10 * 1024 * 1024,
            follow_symlinks: false,
            regex_cache: std::collections::HashMap::new(),
        }
    }

    /// Set maximum file size to scan
    pub fn with_max_file_size(mut self, size: usize) -> Self {
        self.max_file_size = size;
        self
    }

    /// Set whether to follow symlinks
    pub fn with_follow_symlinks(mut self, follow: bool) -> Self {
        self.follow_symlinks = follow;
        self
    }

    /// Scan a directory for engines and injection points
    pub async fn scan_directory(&self, directory: PathBuf) -> Result<ScanResult> {
        let start = Instant::now();
        let mut result = ScanResult::new();

        // Validate directory exists
        if !directory.exists() {
            return Err(CisError::new(
                crate::error::ErrorCategory::NotFound,
                "001",
                format!("Directory not found: {:?}", directory),
            ));
        }

        // Detect engine type
        let engine_info = self.detect_engine(&directory).await?;
        result.engine = engine_info.clone();

        // Walk through directory
        let mut files_scanned = 0;
        let mut lines_scanned = 0;

        let walk_dir = WalkDir::new(&directory)
            .follow_links(self.follow_symlinks)
            .into_iter()
            .filter_entry(|e| {
                // Skip common build/cache directories
                !is_excluded_dir(e.path())
            })
            .filter_map(|e| e.ok());

        for entry in walk_dir {
            let path = entry.path();

            // Skip directories
            if path.is_dir() {
                continue;
            }

            // Check file extension based on detected engine
            let should_scan = if let Some(ref engine) = result.engine {
                is_source_file(path, &engine.engine_type)
            } else {
                // No engine detected, scan all common source files
                is_common_source_file(path)
            };

            if !should_scan {
                continue;
            }

            // Scan file
            match self.scan_file(path, &result.engine).await {
                Ok(mut locations) => {
                    files_scanned += 1;
                    lines_scanned += count_lines(path);
                    result.locations.append(&mut locations);
                }
                Err(e) => {
                    result.errors.push(format!("Error scanning {:?}: {}", path, e));
                }
            }
        }

        result.files_scanned = files_scanned;
        result.lines_scanned = lines_scanned;
        result.duration_ms = start.elapsed().as_millis() as u64;

        Ok(result)
    }

    /// Detect engine type from directory
    async fn detect_engine(&self, directory: &Path) -> Result<Option<EngineInfo>> {
        // Check for Unreal Engine
        if let Some(info) = self.detect_unreal(directory).await? {
            return Ok(Some(info));
        }

        // Check for Unity
        if let Some(info) = self.detect_unity(directory).await? {
            return Ok(Some(info));
        }

        // Check for Godot
        if let Some(info) = self.detect_godot(directory).await? {
            return Ok(Some(info));
        }

        Ok(None)
    }

    /// Detect Unreal Engine project
    async fn detect_unreal(&self, directory: &Path) -> Result<Option<EngineInfo>> {
        // Check for .uproject file
        let project_files = list_files(directory, ".uproject");
        if !project_files.is_empty() {
            let mut info = EngineInfo::new(EngineType::Unreal5_7, directory.to_path_buf());

            // Add config files
            for file in project_files {
                info.add_config_file(file.clone());
            }

            // Check for Config directory
            let config_dir = directory.join("Config");
            if config_dir.exists() {
                let mut entries = fs::read_dir(&config_dir).await.map_err(|e| {
                    CisError::new(
                        crate::error::ErrorCategory::Io,
                        "002",
                        format!("Failed to read Config directory: {}", e),
                    )
                })?;
                while let Some(entry) = entries.next_entry().await.map_err(|e| {
                    CisError::new(
                        crate::error::ErrorCategory::Io,
                        "003",
                        format!("Failed to read config entry: {}", e),
                    )
                })? {
                    info.add_config_file(entry.path());
                }
            }

            return Ok(Some(info));
        }

        Ok(None)
    }

    /// Detect Unity project
    async fn detect_unity(&self, directory: &Path) -> Result<Option<EngineInfo>> {
        // Check for Assets and ProjectSettings directories
        let assets_dir = directory.join("Assets");
        let project_settings = directory.join("ProjectSettings");

        if assets_dir.exists() && project_settings.exists() {
            let mut info = EngineInfo::new(EngineType::Unity2022, directory.to_path_buf());

            // Add key config files
            let settings_file = project_settings.join("ProjectSettings.asset");
            if settings_file.exists() {
                info.add_config_file(settings_file);
            }

            // Check for manifest
            let manifest = directory.join("Packages").join("manifest.json");
            if manifest.exists() {
                info.add_config_file(manifest);
            }

            return Ok(Some(info));
        }

        Ok(None)
    }

    /// Detect Godot project
    async fn detect_godot(&self, directory: &Path) -> Result<Option<EngineInfo>> {
        // Check for project.godot file
        let godot_project = directory.join("project.godot");

        if godot_project.exists() {
            let mut info = EngineInfo::new(EngineType::Godot4, directory.to_path_buf());
            info.add_config_file(godot_project);

            // Check for export presets
            let export_presets = directory.join("export_presets.cfg");
            if export_presets.exists() {
                info.add_config_file(export_presets);
            }

            return Ok(Some(info));
        }

        Ok(None)
    }

    /// Scan a single file for injection points
    async fn scan_file(
        &self,
        file_path: &Path,
        engine_info: &Option<EngineInfo>,
    ) -> Result<Vec<InjectibleLocation>> {
        let mut locations = Vec::new();

        // Check file size
        let metadata = fs::metadata(file_path).await.map_err(|e| {
            CisError::new(
                crate::error::ErrorCategory::Io,
                "004",
                format!("Failed to get file metadata: {}", e),
            )
        })?;

        if metadata.len() as usize > self.max_file_size {
            return Ok(locations);
        }

        // Read file content
        let content = fs::read_to_string(file_path).await.map_err(|e| {
            CisError::new(
                crate::error::ErrorCategory::Io,
                "005",
                format!("Failed to read file: {}", e),
            )
        })?;

        // Get applicable patterns
        let patterns = if let Some(ref engine) = engine_info {
            self.library.for_engine(&engine.engine_type)
        } else {
            self.library.all().iter().collect()
        };

        // Scan each line
        for (line_num, line) in content.lines().enumerate() {
            for pattern in &patterns {
                if let Some(regex) = self.get_or_compile_regex(&pattern.pattern) {
                    if let Some(mat) = regex.find(line) {
                        let location = InjectibleLocation::new(
                            file_path.to_path_buf(),
                            line_num + 1, // 1-indexed
                            pattern.injection_type.clone(),
                            line.trim().to_string(),
                            pattern.name.clone(),
                        )
                        .with_confidence(pattern.confidence)
                        .with_column(mat.start() + 1); // 1-indexed

                        locations.push(location);
                    }
                }
            }
        }

        Ok(locations)
    }

    /// Get or compile regex pattern
    fn get_or_compile_regex(&mut self, pattern: &str) -> Option<&Regex> {
        if !self.regex_cache.contains_key(pattern) {
            match Regex::new(pattern) {
                Ok(regex) => {
                    self.regex_cache.insert(pattern.to_string(), regex);
                }
                Err(_) => {
                    tracing::warn!("Failed to compile regex pattern: {}", pattern);
                    return None;
                }
            }
        }

        self.regex_cache.get(pattern)
    }

    /// Scan a single file (synchronous version)
    pub fn scan_file_sync(&self, file_path: &Path) -> Result<Vec<InjectibleLocation>> {
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| CisError::new(
                crate::error::ErrorCategory::Internal,
                "006",
                format!("Failed to create runtime: {}", e),
            ))?;

        runtime.block_on(self.scan_file(file_path, &None))
    }

    /// Get reference to pattern library
    pub fn library(&self) -> &PatternLibrary {
        &self.library
    }
}

impl Default for EngineScanner {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if directory should be excluded from scanning
fn is_excluded_dir(path: &Path) -> bool {
    let name = match path.file_name() {
        Some(n) => n.to_string_lossy().to_string(),
        None => return false,
    };

    matches!(
        name.as_str(),
        "node_modules" | "target" | "build" | ".git" | ".svn" | "Binaries"
            | "Intermediate" | "Saved" | "DerivedDataCache" | "Library"
            | "Temp" | "obj" | ".vs" | ".idea" | "dist" | "vendor"
    )
}

/// Check if file is a source file for given engine
fn is_source_file(path: &Path, engine_type: &EngineType) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    engine_type.source_extensions().contains(&ext)
}

/// Check if file is a common source file
fn is_common_source_file(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    matches!(
        ext,
        "cpp" | "h" | "hpp" | "c" | "cs" | "gd" | "py" | "js" | "ts"
            | "java" | "lua" | "rs" | "go" | "cc" | "cxx"
    )
}

/// List files with given extension in directory
fn list_files(directory: &Path, extension: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();

    if let Ok(entries) = std::fs::read_dir(directory) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some(extension) {
                files.push(path);
            }
        }
    }

    files
}

/// Count lines in file
fn count_lines(path: &Path) -> usize {
    if let Ok(content) = std::fs::read_to_string(path) {
        content.lines().count()
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::create_dir_all;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_scanner_creation() {
        let scanner = EngineScanner::new();
        assert!(!scanner.library().all().is_empty());
    }

    #[tokio::test]
    async fn test_detect_unreal_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create .uproject file
        let project_file = project_dir.join("TestProject.uproject");
        fs::write(
            &project_file,
            r#"{
                "FileVersion": 3,
                "EngineAssociation": "5.7",
                "Category": "",
                "Description": "Test Project"
            }"#,
        )
        .await
        .unwrap();

        // Create Config directory
        let config_dir = project_dir.join("Config");
        create_dir_all(&config_dir).unwrap();
        fs::write(config_dir.join("DefaultEngine.ini"), "[/Script/Engine.Engine]")
            .await
            .unwrap();

        let scanner = EngineScanner::new();
        let engine_info = scanner.detect_unreal(project_dir).await.unwrap();

        assert!(engine_info.is_some());
        let info = engine_info.unwrap();
        assert_eq!(info.engine_type, EngineType::Unreal5_7);
        assert!(!info.config_files.is_empty());
    }

    #[tokio::test]
    async fn test_detect_unity_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create Assets directory
        create_dir_all(project_dir.join("Assets")).unwrap();

        // Create ProjectSettings directory
        let settings_dir = project_dir.join("ProjectSettings");
        create_dir_all(&settings_dir).unwrap();
        fs::write(
            settings_dir.join("ProjectSettings.asset"),
            "TestUnitySettings",
        )
        .await
        .unwrap();

        let scanner = EngineScanner::new();
        let engine_info = scanner.detect_unity(project_dir).await.unwrap();

        assert!(engine_info.is_some());
        let info = engine_info.unwrap();
        assert_eq!(info.engine_type, EngineType::Unity2022);
    }

    #[tokio::test]
    async fn test_detect_godot_project() {
        let temp_dir = TempDir::new().unwrap();
        let project_dir = temp_dir.path();

        // Create project.godot file
        fs::write(
            project_dir.join("project.godot"),
            r#"config_version=5
[application]
config/name="Test Project""#,
        )
        .await
        .unwrap();

        let scanner = EngineScanner::new();
        let engine_info = scanner.detect_godot(project_dir).await.unwrap();

        assert!(engine_info.is_some());
        let info = engine_info.unwrap();
        assert_eq!(info.engine_type, EngineType::Godot4);
    }

    #[test]
    fn test_is_excluded_dir() {
        assert!(is_excluded_dir(Path::new("node_modules")));
        assert!(is_excluded_dir(Path::new("target")));
        assert!(is_excluded_dir(Path::new(".git")));
        assert!(!is_excluded_dir(Path::new("src")));
        assert!(!is_excluded_dir(Path::new("Assets")));
    }

    #[test]
    fn test_is_source_file() {
        let cpp_file = Path::new("test.cpp");
        assert!(is_source_file(cpp_file, &EngineType::Unreal5_7));

        let cs_file = Path::new("test.cs");
        assert!(is_source_file(cs_file, &EngineType::Unity2022));

        let gd_file = Path::new("test.gd");
        assert!(is_source_file(gd_file, &EngineType::Godot4));
    }
}
