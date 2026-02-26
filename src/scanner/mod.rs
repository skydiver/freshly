pub mod appstore;
pub mod homebrew;
pub mod sparkle;

use std::collections::HashMap;
use async_trait::async_trait;
use crate::model::{AppInfo, DiscoveredApp, ScanResult};

#[async_trait]
pub trait Scanner {
    fn name(&self) -> &str;
    async fn scan(&self, apps: &[DiscoveredApp]) -> ScanResult;
}

/// Abstraction over HTTP GET requests for testability.
#[async_trait]
pub trait HttpClient: Send + Sync {
    async fn get_text(&self, url: &str) -> Result<String, String>;
    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String>;
}

/// Production HTTP client using reqwest.
pub struct ReqwestClient {
    client: reqwest::Client,
}

impl ReqwestClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("Failed to build HTTP client"),
        }
    }
}

#[async_trait]
impl HttpClient for ReqwestClient {
    async fn get_text(&self, url: &str) -> Result<String, String> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?
            .text()
            .await
            .map_err(|e| format!("Failed to read response: {}", e))
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, String> {
        self.client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {}", e))?
            .json::<T>()
            .await
            .map_err(|e| format!("Failed to parse JSON: {}", e))
    }
}

/// Run all three scanners concurrently and merge results.
pub async fn run_scanners(
    apps: &[DiscoveredApp],
    http: &impl HttpClient,
) -> ScanResult {
    let appstore = appstore::AppStoreScanner::new(http);
    let sparkle = sparkle::SparkleScanner::new(http);
    let homebrew = homebrew::HomebrewScanner::new(http);

    let (r1, r2, r3) = tokio::join!(
        appstore.scan(apps),
        sparkle.scan(apps),
        homebrew.scan(apps),
    );

    let mut merged = ScanResult {
        apps: Vec::new(),
        errors: Vec::new(),
    };

    for r in [r1, r2, r3] {
        merged.apps.extend(r.apps);
        merged.errors.extend(r.errors);
    }

    // Deduplicate by bundle_id — prefer the entry that has an update,
    // or the first one seen if neither/both do.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut deduped: Vec<AppInfo> = Vec::new();
    for app in merged.apps {
        if let Some(&idx) = seen.get(&app.bundle_id) {
            if app.has_update && !deduped[idx].has_update {
                deduped[idx] = app;
            }
        } else {
            seen.insert(app.bundle_id.clone(), deduped.len());
            deduped.push(app);
        }
    }
    merged.apps = deduped;

    merged
        .apps
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    merged
}
