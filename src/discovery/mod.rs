use crate::model::DiscoveredApp;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PlistInfo {
    pub bundle_id: String,
    pub name: String,
    pub version: String,
    pub sparkle_feed_url: Option<String>,
}

/// Parse an Info.plist file and extract app metadata.
pub fn parse_info_plist(plist_path: &Path) -> Result<PlistInfo, String> {
    let value = plist::Value::from_file(plist_path)
        .map_err(|e| format!("Failed to read plist: {}", e))?;

    let dict = value
        .as_dictionary()
        .ok_or("Plist root is not a dictionary")?;

    let bundle_id = dict
        .get("CFBundleIdentifier")
        .and_then(|v| v.as_string())
        .ok_or("Missing CFBundleIdentifier")?
        .to_string();

    let name = dict
        .get("CFBundleName")
        .or_else(|| dict.get("CFBundleDisplayName"))
        .and_then(|v| v.as_string())
        .unwrap_or("Unknown")
        .to_string();

    let version = dict
        .get("CFBundleShortVersionString")
        .or_else(|| dict.get("CFBundleVersion"))
        .and_then(|v| v.as_string())
        .unwrap_or("0.0.0")
        .to_string();

    let sparkle_feed_url = dict
        .get("SUFeedURL")
        .and_then(|v| v.as_string())
        .filter(|url| url.starts_with("https://") || url.starts_with("http://"))
        .map(|s| s.to_string());

    Ok(PlistInfo {
        bundle_id,
        name,
        version,
        sparkle_feed_url,
    })
}

/// Scan a directory for .app bundles and parse their Info.plist files.
/// Typically called with `/Applications` as the base path.
pub fn discover_apps(applications_dir: &Path) -> Vec<DiscoveredApp> {
    let mut apps = Vec::new();

    let entries = match std::fs::read_dir(applications_dir) {
        Ok(entries) => entries,
        Err(_) => return apps,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().is_none_or(|ext| ext != "app") {
            continue;
        }

        let plist_path = path.join("Contents").join("Info.plist");
        let info = match parse_info_plist(&plist_path) {
            Ok(info) => info,
            Err(_) => continue,
        };

        let has_mas_receipt = path.join("Contents").join("_MASReceipt").is_dir();

        apps.push(DiscoveredApp {
            name: info.name,
            bundle_id: info.bundle_id,
            version: info.version,
            path,
            has_mas_receipt,
            sparkle_feed_url: info.sparkle_feed_url,
        });
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    #[test]
    fn test_parse_basic_plist() {
        let info = parse_info_plist(&fixture_path("TestApp.plist")).unwrap();
        assert_eq!(info.bundle_id, "com.example.TestApp");
        assert_eq!(info.name, "TestApp");
        assert_eq!(info.version, "1.2.3");
        assert!(info.sparkle_feed_url.is_none());
    }

    #[test]
    fn test_parse_sparkle_plist() {
        let info = parse_info_plist(&fixture_path("SparkleApp.plist")).unwrap();
        assert_eq!(info.bundle_id, "com.example.SparkleApp");
        assert_eq!(
            info.sparkle_feed_url,
            Some("https://example.com/appcast.xml".to_string())
        );
    }

    #[test]
    fn test_parse_nonexistent_plist() {
        let result = parse_info_plist(Path::new("/nonexistent/Info.plist"));
        assert!(result.is_err());
    }

    #[test]
    fn test_discover_apps_with_temp_dir() {
        let dir = tempfile::tempdir().unwrap();

        // Create a fake .app bundle with Info.plist
        let app_dir = dir.path().join("FakeApp.app").join("Contents");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::copy(fixture_path("TestApp.plist"), app_dir.join("Info.plist")).unwrap();

        let apps = discover_apps(dir.path());
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, "TestApp");
        assert_eq!(apps[0].bundle_id, "com.example.TestApp");
        assert!(!apps[0].has_mas_receipt);
    }

    #[test]
    fn test_discover_apps_with_mas_receipt() {
        let dir = tempfile::tempdir().unwrap();

        let app_dir = dir.path().join("StoreApp.app").join("Contents");
        std::fs::create_dir_all(&app_dir).unwrap();
        std::fs::copy(fixture_path("TestApp.plist"), app_dir.join("Info.plist")).unwrap();
        std::fs::create_dir_all(app_dir.join("_MASReceipt")).unwrap();

        let apps = discover_apps(dir.path());
        assert_eq!(apps.len(), 1);
        assert!(apps[0].has_mas_receipt);
    }

    #[test]
    fn test_discover_apps_skips_non_app_dirs() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("NotAnApp")).unwrap();
        std::fs::create_dir_all(dir.path().join("Also.not")).unwrap();

        let apps = discover_apps(dir.path());
        assert!(apps.is_empty());
    }

    #[test]
    fn test_discover_apps_nonexistent_dir() {
        let apps = discover_apps(Path::new("/nonexistent/path"));
        assert!(apps.is_empty());
    }
}
