use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

static LOGGER: OnceLock<TraceLogger> = OnceLock::new();

struct TraceLogger {
    file: Mutex<File>,
    start: Instant,
}

/// Initialize file-based tracing. Creates (or truncates) the log file.
/// Must be called at most once; subsequent calls are ignored.
pub fn init(path: &Path) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(file) = File::create(path) {
        let _ = LOGGER.set(TraceLogger {
            file: Mutex::new(file),
            start: Instant::now(),
        });
    }
}

/// Write a timestamped line to the trace log.
/// No-op if `init` was never called — zero cost beyond an `OnceLock::get()` check.
pub fn log(msg: &str) {
    if let Some(logger) = LOGGER.get() {
        let elapsed = logger.start.elapsed();
        let secs = elapsed.as_secs();
        let millis = elapsed.subsec_millis();
        if let Ok(mut f) = logger.file.lock() {
            let _ = writeln!(f, "[{secs:>4}.{millis:03}s] {msg}");
        }
    }
}
