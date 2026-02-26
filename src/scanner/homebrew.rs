use async_trait::async_trait;
use serde::Deserialize;
use std::collections::HashMap;

use crate::model::{is_newer_version, AppInfo, DiscoveredApp, ScanError, ScanResult, Source};
use crate::scanner::{HttpClient, Scanner};

const CASK_API_URL: &str = "https://formulae.brew.sh/api/cask.json";

pub struct HomebrewScanner<'a, H: HttpClient> {
    http: &'a H,
}

impl<'a, H: HttpClient> HomebrewScanner<'a, H> {
    pub fn new(http: &'a H) -> Self {
        Self { http }
    }
}

/// A single cask entry from the Homebrew Cask API.
#[derive(Deserialize)]
struct CaskEntry {
    token: String,
    name: Vec<String>,
    version: String,
    artifacts: Vec<CaskArtifact>,
}

/// Artifacts are heterogeneous objects in the JSON array.
/// We only care about the ones containing `"app": ["Foo.app"]`.
#[derive(Deserialize)]
#[serde(untagged)]
enum CaskArtifact {
    App { app: Vec<String> },
    #[serde(deserialize_with = "deserialize_ignore")]
    Other,
}

/// Deserialize-and-discard for artifact types we don't care about.
fn deserialize_ignore<'de, D: serde::Deserializer<'de>>(d: D) -> Result<(), D::Error> {
    serde::de::IgnoredAny::deserialize(d)?;
    Ok(())
}

impl CaskEntry {
    /// Extract `.app` filenames from this cask's artifacts.
    fn app_names(&self) -> Vec<&str> {
        self.artifacts
            .iter()
            .filter_map(|a| match a {
                CaskArtifact::App { app } => Some(app.iter().map(|s| s.as_str())),
                CaskArtifact::Other => None,
            })
            .flatten()
            .collect()
    }

    /// Best display name: first entry in `name`, falling back to `token`.
    fn display_name(&self) -> &str {
        self.name.first().map(|s| s.as_str()).unwrap_or(&self.token)
    }
}

/// Strip Homebrew build metadata after a comma (e.g. "5.7.2,2312" → "5.7.2").
fn strip_build_metadata(version: &str) -> String {
    version.split(',').next().unwrap_or(version).to_string()
}

#[async_trait]
impl<H: HttpClient> Scanner for HomebrewScanner<'_, H> {
    fn name(&self) -> &str {
        "Homebrew"
    }

    async fn scan(&self, apps: &[DiscoveredApp]) -> ScanResult {
        let mut result = ScanResult {
            apps: Vec::new(),
            errors: Vec::new(),
        };

        let casks: Vec<CaskEntry> = match self.http.get_json(CASK_API_URL).await {
            Ok(c) => c,
            Err(e) => {
                result.errors.push(ScanError {
                    scanner: "Homebrew".to_string(),
                    app_name: None,
                    message: format!("Failed to fetch cask catalog: {}", e),
                });
                return result;
            }
        };

        // Build lookup: ".app filename" → &CaskEntry
        // Prefer stable casks (no "@" in token) over taps like @beta, @snapshot.
        let mut lookup: HashMap<&str, &CaskEntry> = HashMap::new();
        for cask in &casks {
            let is_variant = cask.token.contains('@');
            for app_name in cask.app_names() {
                if is_variant && lookup.contains_key(app_name) {
                    continue;
                }
                lookup.insert(app_name, cask);
            }
        }

        for app in apps {
            // Skip apps with no version info — can't compare meaningfully
            if app.version == "0.0.0" {
                continue;
            }

            let file_name = match app.path.file_name().and_then(|f| f.to_str()) {
                Some(name) => name,
                None => continue,
            };

            let cask = match lookup.get(file_name) {
                Some(c) => c,
                None => continue,
            };

            // Homebrew cask versions can include build metadata after a comma
            // (e.g. "5.7.2,2312") — strip it for cleaner display and comparison.
            let latest_version = strip_build_metadata(&cask.version);
            let has_update = is_newer_version(&app.version, &latest_version);

            result.apps.push(AppInfo {
                name: cask.display_name().to_string(),
                bundle_id: app.bundle_id.clone(),
                installed_version: app.version.clone(),
                latest_version: Some(latest_version),
                source: Source::Homebrew,
                has_update,
                changelog: None,
                app_path: app.path.clone(),
            });
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Mutex;

    struct MockHttpClient {
        responses: Mutex<Vec<Result<String, String>>>,
    }

    impl MockHttpClient {
        fn with_json(json: &str) -> Self {
            Self {
                responses: Mutex::new(vec![Ok(json.to_string())]),
            }
        }

        fn with_error(msg: &str) -> Self {
            Self {
                responses: Mutex::new(vec![Err(msg.to_string())]),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockHttpClient {
        async fn get_text(&self, _url: &str) -> Result<String, String> {
            self.responses
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Err("No response".into()))
        }

        async fn get_json<T: serde::de::DeserializeOwned>(
            &self,
            _url: &str,
        ) -> Result<T, String> {
            let text = self
                .responses
                .lock()
                .unwrap()
                .pop()
                .unwrap_or(Err("No response".into()))?;
            serde_json::from_str(&text).map_err(|e| format!("Parse error: {}", e))
        }
    }

    fn make_app(name: &str, bundle_id: &str, version: &str) -> DiscoveredApp {
        DiscoveredApp {
            name: name.to_string(),
            bundle_id: bundle_id.to_string(),
            version: version.to_string(),
            path: PathBuf::from(format!("/Applications/{}.app", name)),
            has_mas_receipt: false,
            sparkle_feed_url: None,
        }
    }

    fn cask_json(token: &str, name: &str, version: &str, app_file: &str) -> String {
        format!(
            r#"[{{"token":"{}","name":["{}"],"version":"{}","artifacts":[{{"app":["{}"]}}]}}]"#,
            token, name, version, app_file
        )
    }

    #[tokio::test]
    async fn test_scan_finds_update() {
        let json = cask_json("firefox", "Mozilla Firefox", "125.0.1", "Firefox.app");
        let http = MockHttpClient::with_json(&json);
        let scanner = HomebrewScanner::new(&http);
        let apps = vec![make_app("Firefox", "org.mozilla.firefox", "124.0")];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps.len(), 1);
        assert!(result.apps[0].has_update);
        assert_eq!(
            result.apps[0].latest_version,
            Some("125.0.1".to_string())
        );
        assert_eq!(result.apps[0].installed_version, "124.0");
    }

    #[tokio::test]
    async fn test_scan_no_update() {
        let json = cask_json("firefox", "Mozilla Firefox", "124.0", "Firefox.app");
        let http = MockHttpClient::with_json(&json);
        let scanner = HomebrewScanner::new(&http);
        let apps = vec![make_app("Firefox", "org.mozilla.firefox", "124.0")];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps.len(), 1);
        assert!(!result.apps[0].has_update);
    }

    #[tokio::test]
    async fn test_unmatched_app_skipped() {
        let json = cask_json("firefox", "Mozilla Firefox", "125.0", "Firefox.app");
        let http = MockHttpClient::with_json(&json);
        let scanner = HomebrewScanner::new(&http);
        // App filename doesn't match any cask
        let apps = vec![make_app("MyCustomApp", "com.custom.app", "1.0.0")];

        let result = scanner.scan(&apps).await;

        assert!(result.apps.is_empty());
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn test_api_failure() {
        let http = MockHttpClient::with_error("Network error");
        let scanner = HomebrewScanner::new(&http);
        let apps = vec![make_app("Firefox", "org.mozilla.firefox", "124.0")];

        let result = scanner.scan(&apps).await;

        assert!(result.apps.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].scanner, "Homebrew");
    }

    #[tokio::test]
    async fn test_uses_display_name() {
        let json = cask_json(
            "visual-studio-code",
            "Microsoft Visual Studio Code",
            "1.109.5",
            "Visual Studio Code.app",
        );
        let http = MockHttpClient::with_json(&json);
        let scanner = HomebrewScanner::new(&http);
        let apps = vec![make_app(
            "Visual Studio Code",
            "com.microsoft.VSCode",
            "1.100.0",
        )];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps.len(), 1);
        assert_eq!(result.apps[0].name, "Microsoft Visual Studio Code");
    }

    #[tokio::test]
    async fn test_uses_real_bundle_id() {
        let json = cask_json("firefox", "Mozilla Firefox", "125.0", "Firefox.app");
        let http = MockHttpClient::with_json(&json);
        let scanner = HomebrewScanner::new(&http);
        let apps = vec![make_app("Firefox", "org.mozilla.firefox", "124.0")];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps[0].bundle_id, "org.mozilla.firefox");
    }
}
