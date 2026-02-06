//! Project context extraction service

use crate::types::{CapabilityError, GitStatus, ProjectContext, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ContextExtractor;

impl ContextExtractor {
    pub fn new() -> Self {
        Self
    }

    /// Extract project context from given path
    pub async fn extract(&self, start_path: impl AsRef<Path>) -> Result<ProjectContext> {
        let start_path = start_path.as_ref();
        let project_root = self.find_project_root(start_path).await?;
        
        let project_type = self.detect_project_type(&project_root).await?;
        let package_manager = self.detect_package_manager(&project_root).await?;
        let git_status = self.detect_git_status(&project_root).await?;
        let detected_files = self.list_important_files(&project_root).await?;
        let environment = self.collect_environment(&project_root).await?;

        Ok(ProjectContext {
            project_root: Some(project_root),
            project_type,
            package_manager,
            git_branch: git_status.as_ref().map(|g| g.branch.clone()),
            git_status,
            detected_files,
            environment,
        })
    }

    /// Auto-detect from current directory
    pub async fn detect_current(&self) -> Result<ProjectContext> {
        let current = std::env::current_dir()?;
        self.extract(current).await
    }

    /// Find project root by looking for marker files
    async fn find_project_root(&self, start: &Path) -> Result<PathBuf> {
        let markers = [
            ".git",
            "package.json",
            "Cargo.toml",
            "go.mod",
            "pyproject.toml",
            "setup.py",
            "pom.xml",
            "build.gradle",
        ];

        let mut current = start.to_path_buf();
        
        loop {
            for marker in &markers {
                if current.join(marker).exists() {
                    return Ok(current);
                }
            }

            match current.parent() {
                Some(parent) => current = parent.to_path_buf(),
                None => return Ok(start.to_path_buf()),
            }
        }
    }

    /// Detect project type from files
    async fn detect_project_type(&self, root: &Path) -> Result<Option<String>> {
        if root.join("package.json").exists() {
            return Ok(Some("nodejs".to_string()));
        }
        if root.join("Cargo.toml").exists() {
            return Ok(Some("rust".to_string()));
        }
        if root.join("go.mod").exists() {
            return Ok(Some("go".to_string()));
        }
        if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            return Ok(Some("python".to_string()));
        }
        Ok(None)
    }

    /// Detect package manager
    async fn detect_package_manager(&self, root: &Path) -> Result<Option<String>> {
        if root.join("pnpm-lock.yaml").exists() {
            return Ok(Some("pnpm".to_string()));
        }
        if root.join("yarn.lock").exists() {
            return Ok(Some("yarn".to_string()));
        }
        if root.join("package-lock.json").exists() {
            return Ok(Some("npm".to_string()));
        }
        if root.join("Cargo.toml").exists() {
            return Ok(Some("cargo".to_string()));
        }
        if root.join("go.mod").exists() {
            return Ok(Some("go".to_string()));
        }
        Ok(None)
    }

    /// Detect git status
    async fn detect_git_status(&self, root: &Path) -> Result<Option<GitStatus>> {
        let git_dir = root.join(".git");
        if !git_dir.exists() {
            return Ok(None);
        }

        // Use git2 or command
        let output = tokio::process::Command::new("git")
            .args(["status", "--porcelain", "-b"])
            .current_dir(root)
            .output()
            .await?;

        if !output.status.success() {
            return Ok(None);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<_> = stdout.lines().collect();

        let mut branch = "main".to_string();
        let mut modified = Vec::new();
        let mut untracked = Vec::new();

        for line in lines {
            if line.starts_with("## ") {
                // Parse branch info
                let info = &line[3..];
                if let Some(b) = info.split("...").next() {
                    branch = b.to_string();
                }
            } else if line.starts_with(" M ") || line.starts_with("M ") {
                modified.push(line[3..].to_string());
            } else if line.starts_with("?? ") {
                untracked.push(line[3..].to_string());
            }
        }

        Ok(Some(GitStatus {
            branch,
            ahead: 0,
            behind: 0,
            modified,
            untracked,
        }))
    }

    /// List important files in project
    async fn list_important_files(&self, root: &Path) -> Result<Vec<String>> {
        let important = [
            "README.md",
            "README",
            "Makefile",
            "justfile",
            "Dockerfile",
            "docker-compose.yml",
            ".gitignore",
            "LICENSE",
        ];

        let mut files = Vec::new();
        for name in &important {
            if root.join(name).exists() {
                files.push(name.to_string());
            }
        }
        Ok(files)
    }

    /// Collect environment variables
    async fn collect_environment(&self, _root: &Path) -> Result<HashMap<String, String>> {
        let mut env = HashMap::new();
        
        // Collect relevant env vars
        for key in ["PATH", "HOME", "USER", "SHELL"] {
            if let Ok(val) = std::env::var(key) {
                env.insert(key.to_string(), val);
            }
        }
        
        Ok(env)
    }
}

impl Default for ContextExtractor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_detect_rust_project() {
        let extractor = ContextExtractor::new();
        let ctx = extractor.extract(".").await.unwrap();
        
        // This should detect cis-capability itself
        assert_eq!(ctx.project_type, Some("rust".to_string()));
        assert_eq!(ctx.package_manager, Some("cargo".to_string()));
    }
}
