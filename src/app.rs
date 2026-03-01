use crate::model::{AppInfo, ScanError, ScanResult, Source};
use crate::settings::Settings;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    Loading,
    Main,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pane {
    List,
    Detail,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FilterMode {
    All,
    Outdated,
    UpToDate,
}

impl FilterMode {
    pub fn next(&self) -> Self {
        match self {
            FilterMode::All => FilterMode::Outdated,
            FilterMode::Outdated => FilterMode::UpToDate,
            FilterMode::UpToDate => FilterMode::All,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            FilterMode::All => "All",
            FilterMode::Outdated => "Outdated",
            FilterMode::UpToDate => "Up to date",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum SortMode {
    Name,
    Source,
    Status,
}

impl SortMode {
    pub fn next(&self) -> Self {
        match self {
            SortMode::Name => SortMode::Source,
            SortMode::Source => SortMode::Status,
            SortMode::Status => SortMode::Name,
        }
    }

    pub fn label(&self) -> &str {
        match self {
            SortMode::Name => "Name",
            SortMode::Source => "Source",
            SortMode::Status => "Status",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Update,
    OpenApp,
    HideApp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateResult {
    /// No action taken (no selection, no update available).
    None,
    /// Open the Mac App Store updates page.
    OpenAppStore,
    /// Open the app to trigger its Sparkle update check.
    OpenSparkle { app_name: String, app_path: PathBuf },
    /// Homebrew upgrade needed — caller should spawn the brew process.
    BrewUpgrade { cask_token: String, app_name: String },
}

pub struct App {
    pub screen: Screen,
    pub active_pane: Pane,
    pub apps: Vec<AppInfo>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub detail_scroll: u16,

    pub selected_action: usize,
    pub filter: FilterMode,
    pub sort: SortMode,
    pub search_query: String,
    pub is_searching: bool,
    pub total_scanned: usize,
    pub errors: Vec<ScanError>,
    pub show_errors: bool,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub hidden_bundle_ids: Vec<String>,
    pub settings_path: PathBuf,
    pub brew_overlay: Option<crate::updater::BrewOverlay>,
}

impl App {
    pub fn new(settings_path: PathBuf) -> Self {
        let settings = Settings::load(&settings_path);
        Self {
            screen: Screen::Loading,
            active_pane: Pane::List,
            apps: Vec::new(),
            filtered_indices: Vec::new(),
            selected_index: 0,
            detail_scroll: 0,

            selected_action: 0,
            filter: FilterMode::Outdated,
            sort: SortMode::Name,
            search_query: String::new(),
            is_searching: false,
            total_scanned: 0,
            errors: Vec::new(),
            show_errors: false,
            should_quit: false,
            status_message: None,
            hidden_bundle_ids: settings.hidden_apps,
            settings_path,
            brew_overlay: None,
        }
    }

    pub fn set_results(&mut self, result: ScanResult) {
        self.total_scanned = result.apps.len() + result.errors.len();
        self.errors = result.errors;
        self.show_errors = false;
        self.apps = result.apps;
        self.status_message = None;
        self.apply_filter_and_sort();
        self.screen = Screen::Main;
    }

    pub fn error_count(&self) -> usize {
        self.errors.len()
    }

    pub fn toggle_errors(&mut self) {
        self.show_errors = !self.show_errors;
    }

    pub fn selected_app(&self) -> Option<&AppInfo> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&i| self.apps.get(i))
    }

    pub fn open_selected_app(&mut self) {
        let Some(selected) = self.selected_app() else {
            return;
        };
        let path = selected.app_path.clone();
        match std::process::Command::new("open").arg(&path).spawn() {
            Ok(_) => {
                self.status_message = None;
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to open: {}", e));
            }
        }
    }

    /// Determine the update action for the selected app.
    /// Returns an intent — the caller is responsible for executing the action.
    pub fn update_selected_app(&self) -> UpdateResult {
        let Some(selected) = self.selected_app() else {
            return UpdateResult::None;
        };
        if !selected.has_update {
            return UpdateResult::None;
        }

        match selected.source {
            Source::AppStore => UpdateResult::OpenAppStore,
            Source::Sparkle => UpdateResult::OpenSparkle {
                app_name: selected.name.clone(),
                app_path: selected.app_path.clone(),
            },
            Source::Homebrew => {
                let Some(cask_token) = selected.cask_token.clone() else {
                    return UpdateResult::None;
                };
                UpdateResult::BrewUpgrade {
                    cask_token,
                    app_name: selected.name.clone(),
                }
            }
        }
    }

    pub fn hide_selected_app(&mut self) {
        let Some(selected) = self.selected_app() else {
            return;
        };
        let bundle_id = selected.bundle_id.clone();
        self.hidden_bundle_ids.push(bundle_id);

        let mut settings = Settings::load(&self.settings_path);
        settings.hidden_apps = self.hidden_bundle_ids.clone();
        if let Err(e) = settings.save(&self.settings_path) {
            self.status_message = Some(format!("Failed to save settings: {}", e));
        }

        self.apply_filter_and_sort();
    }

    pub fn outdated_count(&self) -> usize {
        self.apps.iter().filter(|a| a.has_update).count()
    }

    pub fn select_next(&mut self) {
        self.status_message = None;
        if !self.filtered_indices.is_empty() {
            self.selected_index =
                (self.selected_index + 1).min(self.filtered_indices.len() - 1);
            self.detail_scroll = 0;

            self.selected_action = 0;
        }
    }

    pub fn select_previous(&mut self) {
        self.status_message = None;
        self.selected_index = self.selected_index.saturating_sub(1);
        self.detail_scroll = 0;

        self.selected_action = 0;
    }

    pub fn page_down(&mut self, page_size: usize) {
        if !self.filtered_indices.is_empty() {
            self.selected_index =
                (self.selected_index + page_size).min(self.filtered_indices.len() - 1);
            self.detail_scroll = 0;

            self.selected_action = 0;
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected_index = self.selected_index.saturating_sub(page_size);
        self.detail_scroll = 0;

        self.selected_action = 0;
    }

    pub fn toggle_pane(&mut self) {
        self.active_pane = match self.active_pane {
            Pane::List => Pane::Detail,
            Pane::Detail => Pane::List,
        };
    }

    pub fn cycle_filter(&mut self) {
        self.filter = self.filter.next();
        // Status sort is meaningless when filter constrains to one status
        if self.sort == SortMode::Status && self.filter != FilterMode::All {
            self.sort = SortMode::Name;
        }
        self.selected_index = 0;
        self.detail_scroll = 0;
        self.apply_filter_and_sort();
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        // Status sort is meaningless when filter already constrains to one status
        if self.sort == SortMode::Status && self.filter != FilterMode::All {
            self.sort = self.sort.next();
        }
        self.apply_filter_and_sort();
    }

    /// Compute the available actions for the currently selected app.
    pub fn actions_for_selected(&self) -> Vec<Action> {
        let Some(selected) = self.selected_app() else {
            return vec![];
        };
        if selected.has_update {
            match selected.source {
                Source::Sparkle => vec![Action::Update, Action::HideApp],
                _ => vec![Action::Update, Action::OpenApp, Action::HideApp],
            }
        } else {
            vec![Action::OpenApp, Action::HideApp]
        }
    }

    /// Return the currently focused action, if any.
    pub fn selected_action_enum(&self) -> Option<Action> {
        let actions = self.actions_for_selected();
        actions.get(self.selected_action).cloned()
    }

    fn detail_action_count(&self) -> usize {
        self.actions_for_selected().len()
    }

    pub fn navigate_detail_down(&mut self) {
        self.status_message = None;
        if self.selected_action + 1 < self.detail_action_count() {
            self.selected_action += 1;
        }
    }

    pub fn navigate_detail_up(&mut self) {
        self.status_message = None;
        if self.selected_action > 0 {
            self.selected_action -= 1;
        }
    }

    pub fn scroll_detail_down(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_add(1);
    }

    pub fn scroll_detail_up(&mut self) {
        self.detail_scroll = self.detail_scroll.saturating_sub(1);
    }

    pub fn toggle_search(&mut self) {
        self.is_searching = !self.is_searching;
        if !self.is_searching {
            self.search_query.clear();
            self.apply_filter_and_sort();
        }
    }

    pub fn update_search(&mut self, c: char) {
        self.search_query.push(c);
        self.selected_index = 0;
        self.apply_filter_and_sort();
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.selected_index = 0;
        self.apply_filter_and_sort();
    }

    /// Update an app's installed version after a successful brew upgrade.
    /// Re-checks has_update against latest_version, then re-applies filter/sort.
    /// An app that is now up to date will be removed from the Outdated filter view.
    pub fn rescan_app_version(&mut self, bundle_id: &str, new_version: &str) {
        if let Some(app) = self.apps.iter_mut().find(|a| a.bundle_id == bundle_id) {
            app.installed_version = new_version.to_string();
            app.has_update = app
                .latest_version
                .as_ref()
                .map(|v| crate::model::is_newer_version(new_version, v))
                .unwrap_or(false);
        }
        self.apply_filter_and_sort();
    }

    pub fn apply_filter_and_sort(&mut self) {
        self.filtered_indices = self
            .apps
            .iter()
            .enumerate()
            .filter(|(_, app)| {
                if self.hidden_bundle_ids.contains(&app.bundle_id) {
                    return false;
                }
                let filter_match = match self.filter {
                    FilterMode::All => true,
                    FilterMode::Outdated => app.has_update,
                    FilterMode::UpToDate => !app.has_update,
                };
                let search_match = self.search_query.is_empty()
                    || app
                        .name
                        .to_lowercase()
                        .contains(&self.search_query.to_lowercase());
                filter_match && search_match
            })
            .map(|(i, _)| i)
            .collect();

        let apps = &self.apps;
        self.filtered_indices.sort_by(|&a, &b| match self.sort {
            SortMode::Name => apps[a]
                .name
                .to_lowercase()
                .cmp(&apps[b].name.to_lowercase()),
            SortMode::Source => apps[a]
                .source
                .to_string()
                .cmp(&apps[b].source.to_string())
                .then(apps[a].name.to_lowercase().cmp(&apps[b].name.to_lowercase())),
            SortMode::Status => apps[b]
                .has_update
                .cmp(&apps[a].has_update)
                .then(apps[a].name.to_lowercase().cmp(&apps[b].name.to_lowercase())),
        });

        if !self.filtered_indices.is_empty() {
            self.selected_index = self.selected_index.min(self.filtered_indices.len() - 1);
        } else {
            self.selected_index = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{AppInfo, ScanResult, Source};
    use std::path::PathBuf;

    fn make_app(name: &str, has_update: bool, source: Source) -> AppInfo {
        AppInfo {
            name: name.to_string(),
            bundle_id: format!("com.test.{}", name.to_lowercase()),
            installed_version: "1.0.0".to_string(),
            latest_version: if has_update {
                Some("2.0.0".to_string())
            } else {
                None
            },
            source,
            has_update,
            changelog: None,
            app_path: PathBuf::from(format!("/Applications/{}.app", name)),
            cask_token: None,
        }
    }

    fn test_app() -> App {
        App::new(PathBuf::from("/tmp/freshly-test-settings.json"))
    }

    fn sample_result() -> ScanResult {
        ScanResult {
            apps: vec![
                make_app("Firefox", true, Source::Sparkle),
                make_app("Slack", true, Source::AppStore),
                make_app("iTerm2", false, Source::Sparkle),
                make_app("Raycast", false, Source::Sparkle),
            ],
            errors: vec![],
        }
    }

    #[test]
    fn test_initial_state() {
        let app = test_app();
        assert_eq!(app.screen, Screen::Loading);
        assert_eq!(app.active_pane, Pane::List);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_set_results() {
        let mut app = test_app();
        app.set_results(sample_result());
        assert_eq!(app.screen, Screen::Main);
        assert_eq!(app.apps.len(), 4);
        // Default filter is Outdated, so only 2 shown
        assert_eq!(app.filtered_indices.len(), 2);
    }

    #[test]
    fn test_filter_all() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.cycle_filter(); // Outdated -> UpToDate
        app.cycle_filter(); // UpToDate -> All
        assert_eq!(app.filter, FilterMode::All);
        assert_eq!(app.filtered_indices.len(), 4);
    }

    #[test]
    fn test_navigation() {
        let mut app = test_app();
        app.set_results(sample_result());
        assert_eq!(app.selected_index, 0);
        app.select_next();
        assert_eq!(app.selected_index, 1);
        app.select_previous();
        assert_eq!(app.selected_index, 0);
        app.select_previous();
        assert_eq!(app.selected_index, 0);
    }

    #[test]
    fn test_search_filter() {
        let mut app = test_app();
        app.set_results(sample_result());
        // Default is Outdated (Firefox, Slack). Search for "fi" → Firefox only
        app.is_searching = true;
        app.update_search('f');
        app.update_search('i');
        assert_eq!(app.filtered_indices.len(), 1);
    }

    #[test]
    fn test_sort_by_status() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.filter = FilterMode::All;
        app.sort = SortMode::Status;
        app.apply_filter_and_sort();
        let first = app.selected_app().unwrap();
        assert!(first.has_update);
    }

    #[test]
    fn test_cycle_sort_skips_status_when_filtered() {
        let mut app = test_app();
        app.set_results(sample_result());
        // Default filter is Outdated
        assert_eq!(app.filter, FilterMode::Outdated);
        app.sort = SortMode::Source;
        app.cycle_sort(); // Source → Status → skipped → Name
        assert_eq!(app.sort, SortMode::Name);
    }

    #[test]
    fn test_cycle_filter_resets_status_sort() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.filter = FilterMode::All;
        app.sort = SortMode::Status;
        app.apply_filter_and_sort();
        app.cycle_filter(); // All → Outdated, Status sort should reset
        assert_eq!(app.filter, FilterMode::Outdated);
        assert_eq!(app.sort, SortMode::Name);
    }

    #[test]
    fn test_cycle_filter_keeps_name_sort() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.filter = FilterMode::All;
        app.sort = SortMode::Name;
        app.apply_filter_and_sort();
        app.cycle_filter(); // All → Outdated, Name sort should stay
        assert_eq!(app.sort, SortMode::Name);
    }

    #[test]
    fn test_cycle_sort_allows_status_when_all() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.filter = FilterMode::All;
        app.sort = SortMode::Source;
        app.cycle_sort(); // Source → Status
        assert_eq!(app.sort, SortMode::Status);
    }

    #[test]
    fn test_outdated_count() {
        let mut app = test_app();
        app.set_results(sample_result());
        assert_eq!(app.outdated_count(), 2);
    }

    #[test]
    fn test_navigate_detail_up_clamps_at_first_action() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.selected_action = 0;
        app.navigate_detail_up();
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_navigate_detail_down_in_actions() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.selected_action = 0;
        // Two actions exist, so down moves to action 1
        app.navigate_detail_down();
        assert_eq!(app.selected_action, 1);
        // Down again stays at 1 (last action)
        app.navigate_detail_down();
        assert_eq!(app.selected_action, 1);
    }

    #[test]
    fn test_open_selected_app_clears_status_on_success() {
        let mut app = test_app();
        app.set_results(sample_result());
        // Use /dev/null — `open` can handle it without launching a visible app
        app.apps[0].app_path = std::path::PathBuf::from("/dev/null");
        app.filtered_indices = vec![0];
        app.selected_index = 0;
        app.status_message = Some("previous error".to_string());
        app.open_selected_app();
        // open /dev/null succeeds, so status_message should be cleared
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_status_message_clears_on_navigation() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.status_message = Some("error".to_string());
        app.select_next();
        assert!(app.status_message.is_none());
    }

    #[test]
    fn test_open_selected_app_no_crash_when_no_selection() {
        let mut app = test_app();
        // No results loaded, no selected app
        app.open_selected_app();
        // Should not panic, just no-op
    }

    #[test]
    fn test_hidden_apps_excluded_from_filter() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.filter = FilterMode::All;
        app.apply_filter_and_sort();
        assert_eq!(app.filtered_indices.len(), 4);

        // Hide Firefox
        app.hidden_bundle_ids.push("com.test.firefox".to_string());
        app.apply_filter_and_sort();
        assert_eq!(app.filtered_indices.len(), 3);
        // Verify Firefox is not in the filtered list
        for &i in &app.filtered_indices {
            assert_ne!(app.apps[i].bundle_id, "com.test.firefox");
        }
    }

    #[test]
    fn test_hidden_apps_works_with_outdated_filter() {
        let mut app = test_app();
        app.set_results(sample_result());
        // Default filter is Outdated → Firefox, Slack
        assert_eq!(app.filtered_indices.len(), 2);

        app.hidden_bundle_ids.push("com.test.firefox".to_string());
        app.apply_filter_and_sort();
        // Only Slack remains
        assert_eq!(app.filtered_indices.len(), 1);
        let remaining = app.selected_app().unwrap();
        assert_eq!(remaining.name, "Slack");
    }

    #[test]
    fn test_hide_selected_app_no_crash_when_no_selection() {
        let mut app = test_app();
        app.hide_selected_app();
        // Should not panic, just no-op
    }

    #[test]
    fn test_navigate_detail_between_two_actions() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.active_pane = Pane::Detail;
        app.selected_action = 0;

        // Down moves to Hide App
        app.navigate_detail_down();
        assert_eq!(app.selected_action, 1);

        // Up moves back to Open App
        app.navigate_detail_up();
        assert_eq!(app.selected_action, 0);

        // Up again stays on first action (clamped)
        app.navigate_detail_up();
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_actions_for_outdated_homebrew() {
        let mut app = test_app();
        let mut brew_app = make_app("Firefox", true, Source::Homebrew);
        brew_app.cask_token = Some("firefox".to_string());
        app.set_results(ScanResult {
            apps: vec![brew_app],
            errors: vec![],
        });
        let actions = app.actions_for_selected();
        assert_eq!(actions, vec![Action::Update, Action::OpenApp, Action::HideApp]);
    }

    #[test]
    fn test_actions_for_outdated_sparkle() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Firefox", true, Source::Sparkle)],
            errors: vec![],
        });
        let actions = app.actions_for_selected();
        assert_eq!(actions, vec![Action::Update, Action::HideApp]);
    }

    #[test]
    fn test_actions_for_outdated_appstore() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Pages", true, Source::AppStore)],
            errors: vec![],
        });
        let actions = app.actions_for_selected();
        assert_eq!(actions, vec![Action::Update, Action::OpenApp, Action::HideApp]);
    }

    #[test]
    fn test_actions_for_up_to_date() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Firefox", false, Source::Sparkle)],
            errors: vec![],
        });
        app.filter = FilterMode::All;
        app.apply_filter_and_sort();
        let actions = app.actions_for_selected();
        assert_eq!(actions, vec![Action::OpenApp, Action::HideApp]);
    }

    #[test]
    fn test_actions_empty_when_no_selection() {
        let app = test_app();
        let actions = app.actions_for_selected();
        assert!(actions.is_empty());
    }

    #[test]
    fn test_update_appstore_returns_open_appstore() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Pages", true, Source::AppStore)],
            errors: vec![],
        });
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::OpenAppStore);
    }

    #[test]
    fn test_update_sparkle_returns_open_sparkle() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Firefox", true, Source::Sparkle)],
            errors: vec![],
        });
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::OpenSparkle {
            app_name: "Firefox".to_string(),
            app_path: PathBuf::from("/Applications/Firefox.app"),
        });
    }

    #[test]
    fn test_update_no_op_when_no_selection() {
        let app = test_app();
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::None);
    }

    #[test]
    fn test_update_no_op_when_up_to_date() {
        let mut app = test_app();
        app.set_results(ScanResult {
            apps: vec![make_app("Firefox", false, Source::Sparkle)],
            errors: vec![],
        });
        app.filter = FilterMode::All;
        app.apply_filter_and_sort();
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::None);
    }

    #[test]
    fn test_update_homebrew_returns_brew_upgrade() {
        let mut app = test_app();
        let mut brew_app = make_app("Firefox", true, Source::Homebrew);
        brew_app.cask_token = Some("firefox".to_string());
        app.set_results(ScanResult {
            apps: vec![brew_app],
            errors: vec![],
        });
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::BrewUpgrade {
            cask_token: "firefox".to_string(),
            app_name: "Firefox".to_string(),
        });
    }

    #[test]
    fn test_rescan_app_updates_version() {
        let mut app = test_app();
        let mut brew_app = make_app("Firefox", true, Source::Homebrew);
        brew_app.cask_token = Some("firefox".to_string());
        brew_app.installed_version = "1.0.0".to_string();
        brew_app.latest_version = Some("2.0.0".to_string());
        app.set_results(ScanResult {
            apps: vec![brew_app],
            errors: vec![],
        });

        app.rescan_app_version("com.test.firefox", "2.0.0");
        let updated = &app.apps[0];
        assert_eq!(updated.installed_version, "2.0.0");
        assert!(!updated.has_update);
        // App is no longer outdated, so it should be filtered out in Outdated mode
        assert!(app.filtered_indices.is_empty());
    }

    #[test]
    fn test_update_homebrew_missing_token() {
        let mut app = test_app();
        let brew_app = make_app("Firefox", true, Source::Homebrew);
        // cask_token is None by default from make_app
        app.set_results(ScanResult {
            apps: vec![brew_app],
            errors: vec![],
        });
        let result = app.update_selected_app();
        assert_eq!(result, UpdateResult::None);
    }
}
