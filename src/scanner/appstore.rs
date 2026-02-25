use async_trait::async_trait;
use serde::Deserialize;

use crate::model::{is_newer_version, AppInfo, DiscoveredApp, ScanError, ScanResult, Source};
use crate::scanner::{HttpClient, Scanner};

pub struct AppStoreScanner<'a, H: HttpClient> {
    http: &'a H,
}

impl<'a, H: HttpClient> AppStoreScanner<'a, H> {
    pub fn new(http: &'a H) -> Self {
        Self { http }
    }
}

#[derive(Deserialize)]
struct ItunesResponse {
    results: Vec<ItunesApp>,
}

#[derive(Deserialize)]
struct ItunesApp {
    #[serde(rename = "bundleId")]
    bundle_id: String,
    version: String,
    #[serde(rename = "releaseNotes")]
    release_notes: Option<String>,
}

#[async_trait]
impl<H: HttpClient> Scanner for AppStoreScanner<'_, H> {
    fn name(&self) -> &str {
        "App Store"
    }

    async fn scan(&self, apps: &[DiscoveredApp]) -> ScanResult {
        let mas_apps: Vec<&DiscoveredApp> = apps.iter().filter(|a| a.has_mas_receipt).collect();
        let mut result = ScanResult {
            apps: Vec::new(),
            errors: Vec::new(),
        };

        if mas_apps.is_empty() {
            return result;
        }

        // Batch lookup: up to 50 bundle IDs per request
        for chunk in mas_apps.chunks(50) {
            let bundle_ids: Vec<&str> = chunk.iter().map(|a| a.bundle_id.as_str()).collect();
            let url = format!(
                "https://itunes.apple.com/lookup?bundleId={}&country=us",
                bundle_ids.join(",")
            );

            match self.http.get_json::<ItunesResponse>(&url).await {
                Ok(response) => {
                    for app in chunk {
                        if let Some(itunes_app) =
                            response.results.iter().find(|r| r.bundle_id == app.bundle_id)
                        {
                            let has_update =
                                is_newer_version(&app.version, &itunes_app.version);
                            result.apps.push(AppInfo {
                                name: app.name.clone(),
                                bundle_id: app.bundle_id.clone(),
                                installed_version: app.version.clone(),
                                latest_version: Some(itunes_app.version.clone()),
                                source: Source::AppStore,
                                has_update,
                                changelog: itunes_app.release_notes.clone(),
                                app_path: app.path.clone(),
                            });
                        } else {
                            result.apps.push(AppInfo {
                                name: app.name.clone(),
                                bundle_id: app.bundle_id.clone(),
                                installed_version: app.version.clone(),
                                latest_version: None,
                                source: Source::AppStore,
                                has_update: false,
                                changelog: None,
                                app_path: app.path.clone(),
                            });
                        }
                    }
                }
                Err(e) => {
                    for app in chunk {
                        result.errors.push(ScanError {
                            scanner: "App Store".to_string(),
                            app_name: Some(app.name.clone()),
                            message: e.clone(),
                        });
                    }
                }
            }
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

    fn make_mas_app(name: &str, bundle_id: &str, version: &str) -> DiscoveredApp {
        DiscoveredApp {
            name: name.to_string(),
            bundle_id: bundle_id.to_string(),
            version: version.to_string(),
            path: PathBuf::from(format!("/Applications/{}.app", name)),
            has_mas_receipt: true,
            sparkle_feed_url: None,
        }
    }

    #[tokio::test]
    async fn test_scan_finds_update() {
        let json = r#"{"resultCount":1,"results":[{"bundleId":"com.test.app","version":"2.0.0","releaseNotes":"New stuff"}]}"#;
        let http = MockHttpClient::with_json(json);
        let scanner = AppStoreScanner::new(&http);
        let apps = vec![make_mas_app("TestApp", "com.test.app", "1.0.0")];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps.len(), 1);
        assert!(result.apps[0].has_update);
        assert_eq!(result.apps[0].latest_version, Some("2.0.0".to_string()));
        assert_eq!(result.apps[0].changelog, Some("New stuff".to_string()));
    }

    #[tokio::test]
    async fn test_scan_no_update() {
        let json = r#"{"resultCount":1,"results":[{"bundleId":"com.test.app","version":"1.0.0"}]}"#;
        let http = MockHttpClient::with_json(json);
        let scanner = AppStoreScanner::new(&http);
        let apps = vec![make_mas_app("TestApp", "com.test.app", "1.0.0")];

        let result = scanner.scan(&apps).await;

        assert_eq!(result.apps.len(), 1);
        assert!(!result.apps[0].has_update);
    }

    #[tokio::test]
    async fn test_scan_skips_non_mas_apps() {
        let http = MockHttpClient::with_json("{}");
        let scanner = AppStoreScanner::new(&http);
        let apps = vec![DiscoveredApp {
            name: "NonMAS".to_string(),
            bundle_id: "com.test.nonmas".to_string(),
            version: "1.0.0".to_string(),
            path: PathBuf::from("/Applications/NonMAS.app"),
            has_mas_receipt: false,
            sparkle_feed_url: None,
        }];

        let result = scanner.scan(&apps).await;
        assert!(result.apps.is_empty());
    }

    #[tokio::test]
    async fn test_scan_handles_http_error() {
        let http = MockHttpClient::with_error("Network error");
        let scanner = AppStoreScanner::new(&http);
        let apps = vec![make_mas_app("TestApp", "com.test.app", "1.0.0")];

        let result = scanner.scan(&apps).await;
        assert!(result.apps.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert_eq!(result.errors[0].scanner, "App Store");
    }
}
