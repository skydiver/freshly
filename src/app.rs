use crate::model::{AppInfo, ScanError, ScanResult};
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
pub enum DetailFocus {
    Scroll,
    Actions,
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

pub struct App {
    pub screen: Screen,
    pub active_pane: Pane,
    pub apps: Vec<AppInfo>,
    pub filtered_indices: Vec<usize>,
    pub selected_index: usize,
    pub detail_scroll: u16,
    pub detail_focus: DetailFocus,
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
            detail_focus: DetailFocus::Actions,
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
        }
    }

    pub fn set_results(&mut self, result: ScanResult) {
        self.total_scanned = result.apps.len() + result.errors.len();
        self.errors = result.errors;
        self.show_errors = false;
        self.apps = result.apps;
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
            self.detail_focus = DetailFocus::Actions;
            self.selected_action = 0;
        }
    }

    pub fn select_previous(&mut self) {
        self.status_message = None;
        self.selected_index = self.selected_index.saturating_sub(1);
        self.detail_scroll = 0;
        self.detail_focus = DetailFocus::Actions;
        self.selected_action = 0;
    }

    pub fn page_down(&mut self, page_size: usize) {
        if !self.filtered_indices.is_empty() {
            self.selected_index =
                (self.selected_index + page_size).min(self.filtered_indices.len() - 1);
            self.detail_scroll = 0;
            self.detail_focus = DetailFocus::Actions;
            self.selected_action = 0;
        }
    }

    pub fn page_up(&mut self, page_size: usize) {
        self.selected_index = self.selected_index.saturating_sub(page_size);
        self.detail_scroll = 0;
        self.detail_focus = DetailFocus::Actions;
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

    /// Number of action items in the detail pane.
    const DETAIL_ACTION_COUNT: usize = 2; // "Open App", "Hide App"

    pub fn navigate_detail_down(&mut self) {
        self.status_message = None;
        match self.detail_focus {
            DetailFocus::Scroll => {
                // Switch from scrolling to actions
                self.detail_focus = DetailFocus::Actions;
                self.selected_action = 0;
            }
            DetailFocus::Actions => {
                // Move to next action (currently only one, so clamp)
                if self.selected_action + 1 < Self::DETAIL_ACTION_COUNT {
                    self.selected_action += 1;
                }
            }
        }
    }

    pub fn navigate_detail_up(&mut self) {
        self.status_message = None;
        match self.detail_focus {
            DetailFocus::Actions => {
                if self.selected_action > 0 {
                    self.selected_action -= 1;
                } else {
                    // At first action, switch to scroll mode
                    self.detail_focus = DetailFocus::Scroll;
                }
            }
            DetailFocus::Scroll => {
                self.detail_scroll = self.detail_scroll.saturating_sub(1);
            }
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
    fn test_initial_detail_focus() {
        let app = test_app();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_detail_focus_resets_on_select_next() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.detail_focus = DetailFocus::Scroll;
        app.select_next();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn test_detail_focus_resets_on_select_previous() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.select_next(); // move to index 1
        app.detail_focus = DetailFocus::Scroll;
        app.select_previous();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
    }

    #[test]
    fn test_navigate_detail_down_from_scroll_to_actions() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.active_pane = Pane::Detail;
        app.detail_focus = DetailFocus::Scroll;
        app.navigate_detail_down();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_navigate_detail_up_from_actions_to_scroll() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.active_pane = Pane::Detail;
        app.detail_focus = DetailFocus::Actions;
        app.selected_action = 0;
        app.navigate_detail_up();
        assert_eq!(app.detail_focus, DetailFocus::Scroll);
    }

    #[test]
    fn test_navigate_detail_down_in_actions_stays() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.active_pane = Pane::Detail;
        app.detail_focus = DetailFocus::Actions;
        app.selected_action = 0;
        // Two actions exist, so down moves to action 1
        app.navigate_detail_down();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 1);
        // Down again stays at 1 (last action)
        app.navigate_detail_down();
        assert_eq!(app.selected_action, 1);
    }

    #[test]
    fn test_navigate_detail_up_in_scroll_scrolls() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.active_pane = Pane::Detail;
        app.detail_focus = DetailFocus::Scroll;
        app.detail_scroll = 5;
        app.navigate_detail_up();
        assert_eq!(app.detail_scroll, 4);
        assert_eq!(app.detail_focus, DetailFocus::Scroll);
    }

    #[test]
    fn test_detail_focus_resets_on_page_down() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.detail_focus = DetailFocus::Scroll;
        app.page_down(1);
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_detail_focus_resets_on_page_up() {
        let mut app = test_app();
        app.set_results(sample_result());
        app.select_next(); // move off 0 so page_up has effect
        app.detail_focus = DetailFocus::Scroll;
        app.page_up(1);
        assert_eq!(app.detail_focus, DetailFocus::Actions);
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
        app.detail_focus = DetailFocus::Actions;
        app.selected_action = 0;

        // Down moves to Hide App
        app.navigate_detail_down();
        assert_eq!(app.selected_action, 1);

        // Up moves back to Open App
        app.navigate_detail_up();
        assert_eq!(app.selected_action, 0);

        // Up again switches to Scroll
        app.navigate_detail_up();
        assert_eq!(app.detail_focus, DetailFocus::Scroll);
    }
}
