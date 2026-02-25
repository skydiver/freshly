pub mod appstore;
pub mod homebrew;
pub mod sparkle;

use async_trait::async_trait;
use crate::model::{DiscoveredApp, ScanResult};

#[async_trait]
pub trait Scanner {
    #[allow(dead_code)]
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
                .timeout(std::time::Duration::from_secs(10))
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

/// Abstraction over subprocess execution for testability.
#[async_trait]
pub trait CommandRunner: Send + Sync {
    async fn run(&self, command: &str, args: &[&str]) -> Result<String, String>;
}

/// Production command runner using tokio::process.
pub struct TokioCommandRunner;

#[async_trait]
impl CommandRunner for TokioCommandRunner {
    async fn run(&self, command: &str, args: &[&str]) -> Result<String, String> {
        let output = tokio::process::Command::new(command)
            .args(args)
            .output()
            .await
            .map_err(|e| format!("Failed to run {}: {}", command, e))?;

        if output.status.success() {
            String::from_utf8(output.stdout)
                .map_err(|e| format!("Invalid UTF-8 output: {}", e))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            Err(format!("{} failed: {}", command, stderr))
        }
    }
}

/// Run all three scanners concurrently and merge results.
pub async fn run_scanners(
    apps: &[DiscoveredApp],
    http: &impl HttpClient,
    cmd: &impl CommandRunner,
) -> ScanResult {
    let appstore = appstore::AppStoreScanner::new(http);
    let sparkle = sparkle::SparkleScanner::new(http);
    let homebrew = homebrew::HomebrewScanner::new(cmd);

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

    merged
        .apps
        .sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    merged
}
