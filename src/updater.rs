/// Messages sent from the brew process reader task to the main loop.
pub enum BrewOutputMsg {
    /// A line of output from the brew process.
    Line(String),
    /// The process exited with the given success status.
    Finished(bool),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrewStatus {
    Running,
    Confirming,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct BrewOverlay {
    pub cask_token: String,
    pub app_name: String,
    pub status: BrewStatus,
    pub lines: Vec<String>,
    pub scroll: u16,
}

impl BrewOverlay {
    pub fn new(cask_token: String, app_name: String) -> Self {
        Self {
            cask_token,
            app_name,
            status: BrewStatus::Running,
            lines: Vec::new(),
            scroll: 0,
        }
    }

    pub fn push_line(&mut self, line: &str) {
        self.lines.push(line.to_string());
    }

    pub fn request_cancel(&mut self) {
        if self.status == BrewStatus::Running {
            self.status = BrewStatus::Confirming;
        }
    }

    pub fn confirm_cancel(&mut self) {
        if self.status == BrewStatus::Confirming {
            self.status = BrewStatus::Cancelled;
        }
    }

    pub fn abort_cancel(&mut self) {
        if self.status == BrewStatus::Confirming {
            self.status = BrewStatus::Running;
        }
    }

    pub fn finish(&mut self, success: bool) {
        if self.status == BrewStatus::Running || self.status == BrewStatus::Confirming {
            self.status = if success {
                BrewStatus::Succeeded
            } else {
                BrewStatus::Failed
            };
        }
    }

    /// Whether the overlay is in a terminal state (can be dismissed).
    pub fn is_done(&self) -> bool {
        matches!(
            self.status,
            BrewStatus::Succeeded | BrewStatus::Failed | BrewStatus::Cancelled
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_overlay() {
        let overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        assert_eq!(overlay.status, BrewStatus::Running);
        assert!(overlay.lines.is_empty());
        assert_eq!(overlay.cask_token, "firefox");
    }

    #[test]
    fn test_append_line() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.push_line("==> Downloading...");
        assert_eq!(overlay.lines.len(), 1);
        assert_eq!(overlay.lines[0], "==> Downloading...");
    }

    #[test]
    fn test_request_cancel_from_running() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.request_cancel();
        assert_eq!(overlay.status, BrewStatus::Confirming);
    }

    #[test]
    fn test_confirm_cancel() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.request_cancel();
        overlay.confirm_cancel();
        assert_eq!(overlay.status, BrewStatus::Cancelled);
    }

    #[test]
    fn test_abort_cancel() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.request_cancel();
        overlay.abort_cancel();
        assert_eq!(overlay.status, BrewStatus::Running);
    }

    #[test]
    fn test_finish_success() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.finish(true);
        assert_eq!(overlay.status, BrewStatus::Succeeded);
    }

    #[test]
    fn test_finish_failure() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.finish(false);
        assert_eq!(overlay.status, BrewStatus::Failed);
    }

    #[test]
    fn test_request_cancel_ignored_when_not_running() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.finish(true);
        overlay.request_cancel();
        assert_eq!(overlay.status, BrewStatus::Succeeded);
    }

    #[test]
    fn test_is_done() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        assert!(!overlay.is_done());
        overlay.finish(true);
        assert!(overlay.is_done());
    }

    #[test]
    fn test_finish_from_confirming() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.request_cancel();
        assert_eq!(overlay.status, BrewStatus::Confirming);
        overlay.finish(true); // process exits while user is deciding
        assert_eq!(overlay.status, BrewStatus::Succeeded);
    }

    #[test]
    fn test_finish_ignored_when_cancelled() {
        let mut overlay = BrewOverlay::new("firefox".to_string(), "Firefox".to_string());
        overlay.request_cancel();
        overlay.confirm_cancel();
        assert_eq!(overlay.status, BrewStatus::Cancelled);
        overlay.finish(true); // should be ignored — already in terminal state
        assert_eq!(overlay.status, BrewStatus::Cancelled);
    }
}
