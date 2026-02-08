//! # E2E Test Helpers
//!
//! Common utilities and helper functions for E2E tests.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, TempDir};

/// Test environment context that provides isolated test directories
pub struct TestEnv {
    /// Temporary directory that will be cleaned up after test
    pub temp_dir: TempDir,
    /// Path to CIS data directory
    pub data_dir: PathBuf,
    /// Path to CIS config file
    pub config_file: PathBuf,
}

impl TestEnv {
    /// Create a new test environment with isolated directories
    pub fn new() -> anyhow::Result<Self> {
        let temp_dir = tempdir()?;
        let data_dir = temp_dir.path().join(".cis");
        let config_file = data_dir.join("config.toml");

        fs::create_dir_all(&data_dir)?;

        Ok(Self {
            temp_dir,
            data_dir,
            config_file,
        })
    }

    /// Get the path to the temporary directory
    pub fn path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a CIS command with the test environment configured
    pub fn cis_cmd(&self) -> Command {
        let mut cmd = Command::cargo_bin("cis-node").unwrap();
        cmd.env("CIS_DATA_DIR", &self.data_dir);
        cmd.env("CIS_CONFIG_FILE", &self.config_file);
        cmd
    }

    /// Initialize CIS in the test environment (non-interactive)
    pub fn init_cis(&self) -> anyhow::Result<()> {
        self.cis_cmd()
            .arg("init")
            .arg("--non-interactive")
            .arg("--skip-checks")
            .assert()
            .success();
        Ok(())
    }

    /// Check if a file exists in the test environment
    pub fn file_exists(&self, relative_path: &str) -> bool {
        self.data_dir.join(relative_path).exists()
    }

    /// Read a file from the test environment
    pub fn read_file(&self, relative_path: &str) -> anyhow::Result<String> {
        let path = self.data_dir.join(relative_path);
        Ok(fs::read_to_string(path)?)
    }

    /// Write a file to the test environment
    pub fn write_file(&self, relative_path: &str, content: &str) -> anyhow::Result<()> {
        let path = self.data_dir.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new().expect("Failed to create test environment")
    }
}

/// Create a simple test DAG file
pub fn create_test_dag_file(dir: &Path, name: &str) -> PathBuf {
    let dag_content = r#"
[[tasks]]
id = "task1"
command = "echo 'Hello from task1'"
description = "First test task"

[[tasks]]
id = "task2"
command = "echo 'Hello from task2'"
description = "Second test task"
dependencies = ["task1"]

[[tasks]]
id = "task3"
command = "echo 'Hello from task3'"
description = "Third test task"
dependencies = ["task1", "task2"]

[policy]
max_retries = 3
continue_on_failure = false
"#;

    let dag_path = dir.join(name);
    fs::write(&dag_path, dag_content).expect("Failed to write DAG file");
    dag_path
}

/// Create a test DAG file with skill-based tasks
pub fn create_skill_dag_file(dir: &Path, name: &str) -> PathBuf {
    let dag_content = r#"
[[tasks]]
id = "setup"
skill = "init-wizard"
description = "Setup task"

[[tasks]]
id = "process"
skill = "ai-executor"
description = "Processing task"
dependencies = ["setup"]

[policy]
max_retries = 2
continue_on_failure = false
"#;

    let dag_path = dir.join(name);
    fs::write(&dag_path, dag_content).expect("Failed to write skill DAG file");
    dag_path
}

/// Create an invalid DAG file for error testing
pub fn create_invalid_dag_file(dir: &Path, name: &str) -> PathBuf {
    let dag_content = r#"
[[tasks]]
id = "task1"
# Missing required 'command' or 'skill' field

[policy]
max_retries = 3
"#;

    let dag_path = dir.join(name);
    fs::write(&dag_path, dag_content).expect("Failed to write invalid DAG file");
    dag_path
}

/// Create a circular dependency DAG file for error testing
pub fn create_circular_dag_file(dir: &Path, name: &str) -> PathBuf {
    let dag_content = r#"
[[tasks]]
id = "task1"
command = "echo 'task1'"
dependencies = ["task3"]

[[tasks]]
id = "task2"
command = "echo 'task2'"
dependencies = ["task1"]

[[tasks]]
id = "task3"
command = "echo 'task3'"
dependencies = ["task2"]
"#;

    let dag_path = dir.join(name);
    fs::write(&dag_path, dag_content).expect("Failed to write circular DAG file");
    dag_path
}

/// Predicate helpers for common assertions
pub mod predicates {
    use predicates::Predicate;

    /// Check if output contains success message
    pub fn success() -> impl Predicate<str> {
        predicates::str::contains("✅")
            .or(predicates::str::contains("success"))
            .or(predicates::str::contains("完成"))
    }

    /// Check if output contains error message
    pub fn error() -> impl Predicate<str> {
        predicates::str::contains("❌")
            .or(predicates::str::contains("Error"))
            .or(predicates::str::contains("error"))
    }

    /// Check if output contains warning message
    pub fn warning() -> impl Predicate<str> {
        predicates::str::contains("⚠️")
            .or(predicates::str::contains("Warning"))
            .or(predicates::str::contains("warning"))
    }
}

/// Clean up test artifacts (useful for debugging)
pub fn cleanup_test_artifacts() {
    // In normal operation, temp directories are cleaned up automatically.
    // This function is a placeholder for any custom cleanup if needed.
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_creation() {
        let env = TestEnv::new().unwrap();
        assert!(env.path().exists());
        assert!(env.data_dir.exists());
    }

    #[test]
    fn test_file_operations() {
        let env = TestEnv::new().unwrap();

        env.write_file("test.txt", "hello world").unwrap();
        assert!(env.file_exists("test.txt"));

        let content = env.read_file("test.txt").unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_dag_file_creation() {
        let env = TestEnv::new().unwrap();
        let dag_path = create_test_dag_file(env.path(), "test.dag");

        assert!(dag_path.exists());
        let content = fs::read_to_string(dag_path).unwrap();
        assert!(content.contains("task1"));
        assert!(content.contains("task2"));
    }
}
