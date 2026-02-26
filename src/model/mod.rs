use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Source {
    AppStore,
    Sparkle,
    Homebrew,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::AppStore => write!(f, "App Store"),
            Source::Sparkle => write!(f, "Sparkle"),
            Source::Homebrew => write!(f, "Homebrew"),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AppInfo {
    pub name: String,
    pub bundle_id: String,
    pub installed_version: String,
    pub latest_version: Option<String>,
    pub source: Source,
    pub has_update: bool,
    pub changelog: Option<String>,
    pub app_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiscoveredApp {
    pub name: String,
    pub bundle_id: String,
    pub version: String,
    pub path: PathBuf,
    pub has_mas_receipt: bool,
    pub sparkle_feed_url: Option<String>,
}

#[derive(Debug)]
pub struct ScanError {
    pub scanner: String,
    pub app_name: Option<String>,
    pub message: String,
}

#[derive(Debug)]
pub struct ScanResult {
    pub apps: Vec<AppInfo>,
    pub errors: Vec<ScanError>,
}

/// Compare two version strings. Returns true if `latest` is newer than `installed`.
/// Tries semver first, then pads to semver. Returns false for unrecognizable formats.
pub fn is_newer_version(installed: &str, latest: &str) -> bool {
    if let (Ok(inst), Ok(lat)) = (
        semver::Version::parse(installed),
        semver::Version::parse(latest),
    ) {
        return lat > inst;
    }
    // Fallback: pad with .0 to make semver-compatible
    let pad = |v: &str| -> Option<semver::Version> {
        let parts: Vec<&str> = v.split('.').collect();
        let padded = match parts.len() {
            1 => format!("{}.0.0", v),
            2 => format!("{}.0", v),
            _ => v.to_string(),
        };
        semver::Version::parse(&padded).ok()
    };
    if let (Some(inst), Some(lat)) = (pad(installed), pad(latest)) {
        return lat > inst;
    }
    // Unrecognizable format — assume no update rather than guessing
    false
}

/// Returns true if the major version (first numeric segment) increased.
pub fn is_major_update(installed: &str, latest: &str) -> bool {
    let major = |v: &str| -> Option<u64> {
        v.split('.').next().and_then(|s| s.parse().ok())
    };
    match (major(installed), major(latest)) {
        (Some(a), Some(b)) => b > a,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_display() {
        assert_eq!(Source::AppStore.to_string(), "App Store");
        assert_eq!(Source::Sparkle.to_string(), "Sparkle");
        assert_eq!(Source::Homebrew.to_string(), "Homebrew");
    }

    #[test]
    fn test_is_newer_version_semver() {
        assert!(is_newer_version("1.0.0", "1.0.1"));
        assert!(is_newer_version("1.0.0", "2.0.0"));
        assert!(!is_newer_version("2.0.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.0"));
    }

    #[test]
    fn test_is_newer_version_two_part() {
        assert!(is_newer_version("124.0", "125.0"));
        assert!(!is_newer_version("125.0", "124.0"));
    }

    #[test]
    fn test_is_newer_version_single_part() {
        assert!(is_newer_version("1", "2"));
        assert!(!is_newer_version("2", "1"));
    }

    #[test]
    fn test_app_info_creation() {
        let app = AppInfo {
            name: "TestApp".to_string(),
            bundle_id: "com.test.app".to_string(),
            installed_version: "1.0.0".to_string(),
            latest_version: Some("2.0.0".to_string()),
            source: Source::Sparkle,
            has_update: true,
            changelog: Some("Bug fixes".to_string()),
            app_path: PathBuf::from("/Applications/TestApp.app"),
        };
        assert_eq!(app.name, "TestApp");
        assert!(app.has_update);
    }
}
