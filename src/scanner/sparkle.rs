use async_trait::async_trait;
use quick_xml::events::Event;
use quick_xml::Reader;

use crate::model::{is_newer_version, AppInfo, DiscoveredApp, ScanError, ScanResult, Source};
use crate::scanner::{HttpClient, Scanner};

pub struct SparkleScanner<'a, H: HttpClient> {
    http: &'a H,
    concurrency_limit: usize,
}

impl<'a, H: HttpClient> SparkleScanner<'a, H> {
    pub fn new(http: &'a H) -> Self {
        Self {
            http,
            concurrency_limit: 5,
        }
    }
}

#[derive(Debug, Clone)]
struct AppcastItem {
    version: Option<String>,
    short_version: Option<String>,
    description: Option<String>,
}

/// Maximum description text size (1 MB) to prevent memory exhaustion.
const MAX_DESCRIPTION_LEN: usize = 1_000_000;

/// Parse a Sparkle appcast XML feed and return the latest version info.
fn parse_appcast(xml: &str) -> Result<AppcastItem, String> {
    let mut reader = Reader::from_str(xml);
    let mut result_item: Option<AppcastItem> = None;
    let mut current_item: Option<AppcastItem> = None;
    let mut in_description = false;
    let mut description_text = String::new();
    // Track when inside standalone version elements
    let mut in_version = false;
    let mut in_short_version = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) => match e.name().as_ref() {
                b"item" => {
                    current_item = Some(AppcastItem {
                        version: None,
                        short_version: None,
                        description: None,
                    });
                }
                b"description" if current_item.is_some() => {
                    in_description = true;
                    description_text.clear();
                }
                b"enclosure" if current_item.is_some() => {
                    if let Some(ref mut item) = current_item {
                        parse_enclosure_attrs(e.attributes(), item);
                    }
                }
                b"sparkle:version" if current_item.is_some() => {
                    in_version = true;
                }
                b"sparkle:shortVersionString" if current_item.is_some() => {
                    in_short_version = true;
                }
                _ => {}
            },
            Ok(Event::Empty(ref e)) => {
                if e.name().as_ref() == b"enclosure" {
                    if let Some(ref mut item) = current_item {
                        parse_enclosure_attrs(e.attributes(), item);
                    }
                }
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"item" => {
                    if let Some(item) = current_item.take() {
                        result_item = Some(item);
                        break; // Only need the first item
                    }
                }
                b"description" => {
                    if in_description {
                        if let Some(ref mut item) = current_item {
                            item.description = Some(description_text.clone());
                        }
                        in_description = false;
                    }
                }
                b"sparkle:version" => in_version = false,
                b"sparkle:shortVersionString" => in_short_version = false,
                _ => {}
            },
            Ok(Event::Text(ref e)) => {
                if in_description {
                    if description_text.len() < MAX_DESCRIPTION_LEN {
                        description_text.push_str(&e.unescape().unwrap_or_default());
                    }
                } else if let Some(ref mut item) = current_item {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if in_short_version && item.short_version.is_none() {
                        item.short_version = Some(text);
                    } else if in_version && item.version.is_none() {
                        item.version = Some(text);
                    }
                }
            }
            Ok(Event::CData(ref e)) => {
                if in_description && description_text.len() < MAX_DESCRIPTION_LEN {
                    if let Ok(text) = std::str::from_utf8(e.as_ref()) {
                        description_text.push_str(text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {}", e)),
            _ => {}
        }
    }

    result_item.ok_or("No items found in appcast".to_string())
}

fn parse_enclosure_attrs(
    attrs: quick_xml::events::attributes::Attributes,
    item: &mut AppcastItem,
) {
    for attr in attrs.flatten() {
        let key = std::str::from_utf8(attr.key.as_ref()).unwrap_or("");
        let val = attr.unescape_value().unwrap_or_default().to_string();
        if key == "sparkle:shortVersionString" || key.ends_with(":shortVersionString") {
            item.short_version = Some(val);
        } else if (key == "sparkle:version" || key.ends_with(":version")) && item.version.is_none()
        {
            item.version = Some(val);
        }
    }
}

/// Strip HTML to plain text for changelog display.
fn strip_html(html: &str) -> String {
    html2text::from_read(html.as_bytes(), 80).unwrap_or_else(|_| html.to_string())
}

#[async_trait]
impl<H: HttpClient> Scanner for SparkleScanner<'_, H> {
    fn name(&self) -> &str {
        "Sparkle"
    }

    async fn scan(&self, apps: &[DiscoveredApp]) -> ScanResult {
        let sparkle_apps: Vec<&DiscoveredApp> =
            apps.iter().filter(|a| a.sparkle_feed_url.is_some()).collect();

        crate::trace::log(&format!(
            "[sparkle:scan] {} Sparkle candidates out of {} apps",
            sparkle_apps.len(),
            apps.len()
        ));

        let mut result = ScanResult {
            apps: Vec::new(),
            errors: Vec::new(),
        };

        // Process in chunks to limit concurrency
        for chunk in sparkle_apps.chunks(self.concurrency_limit) {
            let futures: Vec<_> = chunk
                .iter()
                .map(|app| {
                    let feed_url = app.sparkle_feed_url.as_ref().unwrap().clone();
                    async move {
                        let xml = self.http.get_text(&feed_url).await;
                        (*app, xml)
                    }
                })
                .collect();

            let results = futures::future::join_all(futures).await;

            for (app, xml_result) in results {
                match xml_result {
                    Ok(xml) => match parse_appcast(&xml) {
                        Ok(item) => {
                            let latest = item.short_version.or(item.version);
                            let has_update = latest
                                .as_ref()
                                .map(|v| is_newer_version(&app.version, v))
                                .unwrap_or(false);
                            let changelog = item.description.map(|d| strip_html(&d));

                            result.apps.push(AppInfo {
                                name: app.name.clone(),
                                bundle_id: app.bundle_id.clone(),
                                installed_version: app.version.clone(),
                                latest_version: latest,
                                source: Source::Sparkle,
                                has_update,
                                changelog,
                                app_path: app.path.clone(),
                            });
                        }
                        Err(e) => {
                            result.errors.push(ScanError {
                                scanner: self.name().to_string(),
                                app_name: Some(app.name.clone()),
                                message: e,
                            });
                        }
                    },
                    Err(e) => {
                        result.errors.push(ScanError {
                            scanner: "Sparkle".to_string(),
                            app_name: Some(app.name.clone()),
                            message: e,
                        });
                    }
                }
            }
        }

        let outdated = result.apps.iter().filter(|a| a.has_update).count();
        crate::trace::log(&format!(
            "[sparkle:scan] {} matched, {} outdated, {} errors",
            result.apps.len(),
            outdated,
            result.errors.len()
        ));

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_parse_appcast_basic() {
        let xml = std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/appcast.xml"),
        )
        .unwrap();

        let item = parse_appcast(&xml).unwrap();
        assert_eq!(item.short_version, Some("2.0.0".to_string()));
        assert!(item.description.is_some());
    }

    #[test]
    fn test_parse_appcast_empty() {
        let xml = r#"<?xml version="1.0"?><rss><channel></channel></rss>"#;
        let result = parse_appcast(xml);
        assert!(result.is_err());
    }

    #[test]
    fn test_strip_html() {
        let html = "<ul><li>New feature</li><li>Bug fix</li></ul>";
        let text = strip_html(html);
        assert!(text.contains("New feature"));
        assert!(text.contains("Bug fix"));
    }
}
