use async_trait::async_trait;
use std::path::PathBuf;

use crate::model::{AppInfo, DiscoveredApp, ScanError, ScanResult, Source};
use crate::scanner::{CommandRunner, Scanner};

pub struct HomebrewScanner<'a, C: CommandRunner> {
    runner: &'a C,
}

impl<'a, C: CommandRunner> HomebrewScanner<'a, C> {
    pub fn new(runner: &'a C) -> Self {
        Self { runner }
    }
}

/// Parse `brew outdated --cask --greedy` output.
/// Each line is like: `firefox (124.0) != 125.0.1`
fn parse_outdated_line(line: &str) -> Option<(String, String, String)> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let parts: Vec<&str> = line.splitn(2, " (").collect();
    if parts.len() != 2 {
        return None;
    }

    let name = parts[0].trim().to_string();
    let rest = parts[1];

    let parts: Vec<&str> = rest.splitn(2, ") != ").collect();
    if parts.len() != 2 {
        return None;
    }

    let installed = parts[0].trim().to_string();
    let latest = parts[1].trim().to_string();

    Some((name, installed, latest))
}

#[async_trait]
impl<C: CommandRunner> Scanner for HomebrewScanner<'_, C> {
    fn name(&self) -> &str {
        "Homebrew"
    }

    async fn scan(&self, _apps: &[DiscoveredApp]) -> ScanResult {
        let mut result = ScanResult {
            apps: Vec::new(),
            errors: Vec::new(),
        };

        // Check if brew is available
        let cask_list = match self.runner.run("brew", &["list", "--cask"]).await {
            Ok(output) => output,
            Err(e) => {
                result.errors.push(ScanError {
                    scanner: "Homebrew".to_string(),
                    app_name: None,
                    message: format!("Homebrew not available: {}", e),
                });
                return result;
            }
        };

        let installed_casks: Vec<String> = cask_list
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();

        // Get outdated casks
        let outdated_output: String = self
            .runner
            .run("brew", &["outdated", "--cask", "--greedy"])
            .await
            .unwrap_or_default();

        let mut outdated_map: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();

        for line in outdated_output.lines() {
            if let Some((name, installed, latest)) = parse_outdated_line(line) {
                outdated_map.insert(name, (installed, latest));
            }
        }

        for cask_name in &installed_casks {
            if let Some((installed, latest)) = outdated_map.get(cask_name) {
                result.apps.push(AppInfo {
                    name: cask_name.clone(),
                    bundle_id: format!("homebrew.cask.{}", cask_name),
                    installed_version: installed.clone(),
                    latest_version: Some(latest.clone()),
                    source: Source::Homebrew,
                    has_update: true,
                    changelog: None,
                    app_path: PathBuf::from(format!("/opt/homebrew/Caskroom/{}", cask_name)),
                });
            } else {
                result.apps.push(AppInfo {
                    name: cask_name.clone(),
                    bundle_id: format!("homebrew.cask.{}", cask_name),
                    installed_version: "latest".to_string(),
                    latest_version: None,
                    source: Source::Homebrew,
                    has_update: false,
                    changelog: None,
                    app_path: PathBuf::from(format!("/opt/homebrew/Caskroom/{}", cask_name)),
                });
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockCommandRunner {
        responses: Mutex<std::collections::HashMap<String, Result<String, String>>>,
    }

    impl MockCommandRunner {
        fn new() -> Self {
            Self {
                responses: Mutex::new(std::collections::HashMap::new()),
            }
        }

        fn with_response(self, key: &str, response: Result<String, String>) -> Self {
            self.responses
                .lock()
                .unwrap()
                .insert(key.to_string(), response);
            self
        }
    }

    #[async_trait]
    impl CommandRunner for MockCommandRunner {
        async fn run(&self, command: &str, args: &[&str]) -> Result<String, String> {
            let key = format!("{} {}", command, args.join(" "));
            self.responses
                .lock()
                .unwrap()
                .remove(&key)
                .unwrap_or(Err(format!("Unexpected command: {}", key)))
        }
    }

    #[test]
    fn test_parse_outdated_line() {
        let (name, installed, latest) =
            parse_outdated_line("firefox (124.0) != 125.0.1").unwrap();
        assert_eq!(name, "firefox");
        assert_eq!(installed, "124.0");
        assert_eq!(latest, "125.0.1");
    }

    #[test]
    fn test_parse_outdated_line_empty() {
        assert!(parse_outdated_line("").is_none());
        assert!(parse_outdated_line("   ").is_none());
    }

    #[tokio::test]
    async fn test_scan_with_outdated_casks() {
        let runner = MockCommandRunner::new()
            .with_response("brew list --cask", Ok("firefox\nslack\niterm2".to_string()))
            .with_response(
                "brew outdated --cask --greedy",
                Ok("firefox (124.0) != 125.0.1".to_string()),
            );

        let scanner = HomebrewScanner::new(&runner);
        let result = scanner.scan(&[]).await;

        assert_eq!(result.apps.len(), 3);
        let firefox = result.apps.iter().find(|a| a.name == "firefox").unwrap();
        assert!(firefox.has_update);
        assert_eq!(firefox.latest_version, Some("125.0.1".to_string()));

        let slack = result.apps.iter().find(|a| a.name == "slack").unwrap();
        assert!(!slack.has_update);
    }

    #[tokio::test]
    async fn test_scan_brew_not_installed() {
        let runner =
            MockCommandRunner::new().with_response("brew list --cask", Err("command not found".to_string()));

        let scanner = HomebrewScanner::new(&runner);
        let result = scanner.scan(&[]).await;

        assert!(result.apps.is_empty());
        assert_eq!(result.errors.len(), 1);
    }
}
