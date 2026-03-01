/// A running `brew upgrade` process: groups the output channel and child handle
/// so they are always created and dropped together.
pub struct BrewProcess {
    rx: Option<tokio::sync::mpsc::Receiver<String>>,
    child: std::process::Child,
}

impl BrewProcess {
    /// Receive the next output line, or pend forever once the channel closes.
    pub async fn recv(&mut self) -> String {
        match &mut self.rx {
            Some(rx) => match rx.recv().await {
                Some(line) => line,
                None => {
                    // Reader threads finished — stop polling.
                    self.rx = None;
                    std::future::pending().await
                }
            },
            None => std::future::pending().await,
        }
    }

    /// Try to reap the child without blocking. Returns `Some(success)` when done.
    pub fn try_wait(&mut self) -> Option<bool> {
        match self.child.try_wait() {
            Ok(Some(status)) => Some(status.success()),
            _ => None,
        }
    }

    /// Kill the child process and wait for it to exit.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Spawn `brew upgrade --cask <token>` and return a [`BrewProcess`] handle.
///
/// Returns a specific message when `brew` is not found on PATH so the caller
/// can show a user-friendly hint.
pub fn spawn_brew_upgrade(cask_token: &str) -> Result<BrewProcess, String> {
    let mut child = std::process::Command::new("brew")
        .args(["upgrade", "--cask", cask_token])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Homebrew not found — install from brew.sh".to_string()
            } else {
                format!("Failed to start brew: {}", e)
            }
        })?;

    let (tx, rx) = tokio::sync::mpsc::channel::<String>(100);

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();

    let tx_out = tx.clone();
    if let Some(stdout) = stdout {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                if tx_out.blocking_send(line).is_err() {
                    break;
                }
            }
        });
    }

    if let Some(stderr) = stderr {
        std::thread::spawn(move || {
            use std::io::BufRead;
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if tx.blocking_send(line).is_err() {
                    break;
                }
            }
        });
    }

    Ok(BrewProcess { rx: Some(rx), child })
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
}

impl BrewOverlay {
    pub fn new(cask_token: String, app_name: String) -> Self {
        Self {
            cask_token,
            app_name,
            status: BrewStatus::Running,
            lines: Vec::new(),
        }
    }

    const MAX_LINES: usize = 1000;

    pub fn push_line(&mut self, line: &str) {
        if self.lines.len() >= Self::MAX_LINES {
            self.lines.remove(0);
        }
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
