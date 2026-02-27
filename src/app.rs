use crate::model::{AppInfo, ScanResult};

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
    pub error_count: usize,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
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
            error_count: 0,
            should_quit: false,
        }
    }

    pub fn set_results(&mut self, result: ScanResult) {
        self.error_count = result.errors.len();
        self.total_scanned = result.apps.len() + self.error_count;
        self.apps = result.apps;
        self.apply_filter_and_sort();
        self.screen = Screen::Main;
    }

    pub fn selected_app(&self) -> Option<&AppInfo> {
        self.filtered_indices
            .get(self.selected_index)
            .and_then(|&i| self.apps.get(i))
    }

    pub fn outdated_count(&self) -> usize {
        self.apps.iter().filter(|a| a.has_update).count()
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected_index =
                (self.selected_index + 1).min(self.filtered_indices.len() - 1);
            self.detail_scroll = 0;
            self.detail_focus = DetailFocus::Actions;
            self.selected_action = 0;
        }
    }

    pub fn select_previous(&mut self) {
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
        self.selected_index = 0;
        self.detail_scroll = 0;
        self.apply_filter_and_sort();
    }

    pub fn cycle_sort(&mut self) {
        self.sort = self.sort.next();
        self.apply_filter_and_sort();
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
        }
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
        let app = App::new();
        assert_eq!(app.screen, Screen::Loading);
        assert_eq!(app.active_pane, Pane::List);
        assert!(!app.should_quit);
    }

    #[test]
    fn test_set_results() {
        let mut app = App::new();
        app.set_results(sample_result());
        assert_eq!(app.screen, Screen::Main);
        assert_eq!(app.apps.len(), 4);
        // Default filter is Outdated, so only 2 shown
        assert_eq!(app.filtered_indices.len(), 2);
    }

    #[test]
    fn test_filter_all() {
        let mut app = App::new();
        app.set_results(sample_result());
        app.cycle_filter(); // Outdated -> UpToDate
        app.cycle_filter(); // UpToDate -> All
        assert_eq!(app.filter, FilterMode::All);
        assert_eq!(app.filtered_indices.len(), 4);
    }

    #[test]
    fn test_navigation() {
        let mut app = App::new();
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
        let mut app = App::new();
        app.set_results(sample_result());
        // Default is Outdated (Firefox, Slack). Search for "fi" → Firefox only
        app.is_searching = true;
        app.update_search('f');
        app.update_search('i');
        assert_eq!(app.filtered_indices.len(), 1);
    }

    #[test]
    fn test_sort_by_status() {
        let mut app = App::new();
        app.set_results(sample_result());
        app.sort = SortMode::Status;
        app.apply_filter_and_sort();
        let first = app.selected_app().unwrap();
        assert!(first.has_update);
    }

    #[test]
    fn test_outdated_count() {
        let mut app = App::new();
        app.set_results(sample_result());
        assert_eq!(app.outdated_count(), 2);
    }

    #[test]
    fn test_initial_detail_focus() {
        let app = App::new();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
    }

    #[test]
    fn test_detail_focus_resets_on_select_next() {
        let mut app = App::new();
        app.set_results(sample_result());
        app.detail_focus = DetailFocus::Scroll;
        app.select_next();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
        assert_eq!(app.selected_action, 0);
        assert_eq!(app.detail_scroll, 0);
    }

    #[test]
    fn test_detail_focus_resets_on_select_previous() {
        let mut app = App::new();
        app.set_results(sample_result());
        app.select_next(); // move to index 1
        app.detail_focus = DetailFocus::Scroll;
        app.select_previous();
        assert_eq!(app.detail_focus, DetailFocus::Actions);
    }
}
