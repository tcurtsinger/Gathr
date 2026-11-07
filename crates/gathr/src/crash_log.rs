//! The plain log for what happens before the database opens, or after it can't take a
//! write (0019). Lines are redacted as they're written; the next good start copies them
//! into the event record and empties the file.

use std::io::Write;
use std::path::PathBuf;

use crate::ids::{now_us, utc};
use crate::redact::redact;

#[derive(Debug, Clone)]
pub struct CrashLog {
    path: PathBuf,
}

impl CrashLog {
    pub fn new(path: PathBuf) -> Self {
        CrashLog { path }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Appends one line. Errors are dropped: this is the place of last resort.
    pub fn append(&self, line: &str) {
        let text = format!(
            "{} {}\n",
            utc(now_us()),
            redact(line.trim_end()).replace('\n', " | ")
        );
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = file.write_all(text.as_bytes());
        }
    }

    /// The last `n` lines, oldest first. Empty when there's no log.
    pub fn last_lines(&self, n: usize) -> Vec<String> {
        let text = std::fs::read_to_string(&self.path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().collect();
        lines[lines.len().saturating_sub(n)..]
            .iter()
            .map(|l| l.to_string())
            .collect()
    }

    /// Every line, emptying the file.
    pub fn take(&self) -> Vec<String> {
        let lines = self.last_lines(usize::MAX);
        if !lines.is_empty() {
            let _ = std::fs::write(&self.path, b"");
        }
        lines
    }
}
