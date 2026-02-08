//! # Element App Detection
//!
//! Detect installed Element (Matrix client) applications on the system.

use std::path::PathBuf;
use tracing::{debug, info, warn};

/// Information about detected Element app
#[derive(Debug, Clone)]
pub struct ElementAppInfo {
    /// App name (Element, Element Nightly, etc.)
    pub name: String,
    /// Path to the executable
    pub path: PathBuf,
    /// Version if detectable
    pub version: Option<String>,
    /// Platform-specific app bundle path
    pub app_bundle_path: Option<PathBuf>,
}

/// Detect Element apps on the system
pub fn detect_element_apps() -> Vec<ElementAppInfo> {
    let mut apps = Vec::new();

    // Check for various Element executable names
    let executable_names = vec![
        "element",           // Linux
        "element-desktop",   // Linux (alternative)
        "Element",           // macOS
        "element-nightly",   // Nightly builds
        "Element Nightly",   // macOS nightly
    ];

    for name in executable_names {
        if let Ok(path) = which::which(name) {
            debug!("Found Element executable: {}", path.display());
            
            // Try to get version
            let version = get_element_version(&path);
            
            // Find app bundle path
            let app_bundle = find_app_bundle(&path, name);
            
            apps.push(ElementAppInfo {
                name: name.to_string(),
                path,
                version,
                app_bundle_path: app_bundle,
            });
        }
    }

    // Platform-specific detection
    #[cfg(target_os = "macos")]
    {
        apps.extend(detect_macos_element_apps());
    }

    #[cfg(target_os = "linux")]
    {
        apps.extend(detect_linux_element_apps());
    }

    #[cfg(target_os = "windows")]
    {
        apps.extend(detect_windows_element_apps());
    }

    info!("Detected {} Element app(s)", apps.len());
    apps
}

/// Get Element version by running --version flag
fn get_element_version(path: &PathBuf) -> Option<String> {
    // Try common version flags
    for flag in &["--version", "-v", "version"] {
        if let Ok(output) = std::process::Command::new(path)
            .arg(flag)
            .output()
        {
            if output.status.success() {
                let version = String::from_utf8_lossy(&output.stdout);
                let version = version.trim();
                if !version.is_empty() {
                    return Some(version.to_string());
                }
            }
        }
    }
    None
}

/// Find app bundle path (macOS .app, Linux .desktop, etc.)
fn find_app_bundle(path: &PathBuf, name: &str) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        // On macOS, look for .app bundle
        let mut current = path.clone();
        while let Some(parent) = current.parent() {
            if parent.extension().map(|e| e == "app").unwrap_or(false) {
                return Some(parent.to_path_buf());
            }
            current = parent.to_path_buf();
        }

        // Common install locations
        let app_paths = vec![
            format!("/Applications/{}.app", name),
            format!("/Applications/{} Desktop.app", name),
            format!("/Users/{}/Applications/{}.app", whoami::username(), name),
        ];

        for app_path in app_paths {
            let path = PathBuf::from(app_path);
            if path.exists() {
                return Some(path);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Look for .desktop files
        let desktop_dirs = vec![
            format!("/usr/share/applications/{}.desktop", name.to_lowercase()),
            format!("/usr/local/share/applications/{}.desktop", name.to_lowercase()),
            format!(
                "{}/.local/share/applications/{}.desktop",
                std::env::var("HOME").unwrap_or_default(),
                name.to_lowercase()
            ),
        ];

        for desktop_path in desktop_dirs {
            let path = PathBuf::from(desktop_path);
            if path.exists() {
                return Some(path);
            }
        }
    }

    None
}

#[cfg(target_os = "macos")]
fn detect_macos_element_apps() -> Vec<ElementAppInfo> {
    let mut apps = Vec::new();
    let app_names = vec!["Element", "Element Nightly", "Element Dev"];

    for name in app_names {
        let app_path = PathBuf::from(format!("/Applications/{}.app", name));
        if app_path.exists() {
            let executable_path = app_path.join("Contents/MacOS/Element");
            if executable_path.exists() {
                let version = get_element_version(&executable_path);
                apps.push(ElementAppInfo {
                    name: name.to_string(),
                    path: executable_path,
                    version,
                    app_bundle_path: Some(app_path),
                });
            }
        }
    }

    apps
}

#[cfg(target_os = "linux")]
fn detect_linux_element_apps() -> Vec<ElementAppInfo> {
    let mut apps = Vec::new();
    
    // Check Flatpak installation
    let flatpak_path = PathBuf::from("/var/lib/flatpak/app/im.riot.Riot/current/active/files/bin/element-desktop");
    if flatpak_path.exists() {
        apps.push(ElementAppInfo {
            name: "Element (Flatpak)".to_string(),
            path: flatpak_path.clone(),
            version: get_element_version(&flatpak_path),
            app_bundle_path: None,
        });
    }

    // Check Snap installation
    let snap_path = PathBuf::from("/snap/bin/element-desktop");
    if snap_path.exists() {
        apps.push(ElementAppInfo {
            name: "Element (Snap)".to_string(),
            path: snap_path.clone(),
            version: get_element_version(&snap_path),
            app_bundle_path: None,
        });
    }

    apps
}

#[cfg(target_os = "windows")]
fn detect_windows_element_apps() -> Vec<ElementAppInfo> {
    let mut apps = Vec::new();
    
    // Check common Windows install locations
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        let element_path = PathBuf::from(format!("{}/Programs/element/Element.exe", local_app_data));
        if element_path.exists() {
            apps.push(ElementAppInfo {
                name: "Element".to_string(),
                path: element_path.clone(),
                version: get_element_version(&element_path),
                app_bundle_path: None,
            });
        }
    }

    apps
}

/// Check if Element is installed
pub fn is_element_installed() -> bool {
    !detect_element_apps().is_empty()
}

/// Get the primary Element app (first found)
pub fn get_primary_element_app() -> Option<ElementAppInfo> {
    detect_element_apps().into_iter().next()
}

/// Print Element detection results
pub fn print_element_status() {
    let apps = detect_element_apps();

    if apps.is_empty() {
        println!("❌ Element app not detected");
        println!("   Install from: https://element.io/download");
    } else {
        println!("✅ Found {} Element app(s):", apps.len());
        for (i, app) in apps.iter().enumerate() {
            println!("\n   [{}] {}", i + 1, app.name);
            println!("       Path: {}", app.path.display());
            if let Some(ref version) = app.version {
                println!("       Version: {}", version);
            }
            if let Some(ref bundle) = app.app_bundle_path {
                println!("       Bundle: {}", bundle.display());
            }
        }
    }
}

/// Launch Element with a specific homeserver URL
pub fn launch_element_with_homeserver(homeserver_url: &str) -> Result<(), String> {
    let app = get_primary_element_app()
        .ok_or("Element app not found")?;

    info!("Launching Element from: {}", app.path.display());
    info!("Homeserver URL: {}", homeserver_url);

    // Different launch strategies based on platform
    #[cfg(target_os = "macos")]
    {
        // On macOS, use open command
        if let Some(bundle) = app.app_bundle_path {
            std::process::Command::new("open")
                .arg(&bundle)
                .spawn()
                .map_err(|e| format!("Failed to launch Element: {}", e))?;
        } else {
            std::process::Command::new(&app.path)
                .spawn()
                .map_err(|e| format!("Failed to launch Element: {}", e))?;
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        std::process::Command::new(&app.path)
            .spawn()
            .map_err(|e| format!("Failed to launch Element: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_element() {
        let apps = detect_element_apps();
        println!("Detected {} Element apps", apps.len());
        for app in &apps {
            println!("  - {} at {}", app.name, app.path.display());
        }
    }

    #[test]
    fn test_is_element_installed() {
        let installed = is_element_installed();
        println!("Element installed: {}", installed);
    }
}
