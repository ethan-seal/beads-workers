// Log viewing and tailing functionality

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::sleep;

/// Structured log entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Timestamp in RFC3339 format
    pub timestamp: String,
    /// Log level (TRACE, DEBUG, INFO, WARN, ERROR)
    pub level: String,
    /// Worker ID if applicable
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,
    /// Log target (module path)
    pub target: String,
    /// Log message
    pub message: String,
    /// Additional fields
    #[serde(flatten)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl LogEntry {
    /// Parse a log line into a LogEntry
    pub fn parse(line: &str) -> Result<Self> {
        // Try to parse as JSON first (structured logging)
        if let Ok(entry) = serde_json::from_str::<LogEntry>(line) {
            return Ok(entry);
        }

        // Fall back to parsing plain text logs
        // Format: "YYYY-MM-DD HH:MM:SS LEVEL target: message"
        let parts: Vec<&str> = line.splitn(4, ' ').collect();

        if parts.len() < 4 {
            // Return a basic entry for malformed lines
            return Ok(LogEntry {
                timestamp: chrono::Utc::now().to_rfc3339(),
                level: "INFO".to_string(),
                worker_id: None,
                target: "unknown".to_string(),
                message: line.to_string(),
                fields: serde_json::Map::new(),
            });
        }

        let timestamp = format!("{}T{}", parts[0], parts[1]);
        let level = parts[2].to_string();
        let rest = parts[3];

        // Try to extract target and message
        let (target, message) = if let Some(colon_idx) = rest.find(':') {
            let target = rest[..colon_idx].trim();
            let message = rest[colon_idx + 1..].trim();
            (target.to_string(), message.to_string())
        } else {
            ("unknown".to_string(), rest.to_string())
        };

        // Try to extract worker_id from message or fields
        let worker_id = extract_worker_id(&message);

        Ok(LogEntry {
            timestamp,
            level,
            worker_id,
            target,
            message,
            fields: serde_json::Map::new(),
        })
    }

    /// Check if this log entry matches the given filters
    pub fn matches_filter(&self, level_filter: Option<&str>, worker_filter: Option<&str>) -> bool {
        // Check level filter
        if let Some(filter_level) = level_filter {
            let filter_level = filter_level.to_uppercase();
            if self.level.to_uppercase() != filter_level {
                return false;
            }
        }

        // Check worker filter
        if let Some(filter_worker) = worker_filter {
            match &self.worker_id {
                Some(worker_id) => {
                    if worker_id != filter_worker {
                        return false;
                    }
                }
                None => return false,
            }
        }

        true
    }

    /// Format the log entry for display
    pub fn format(&self, colored: bool) -> String {
        let level_str = if colored {
            colorize_level(&self.level)
        } else {
            self.level.clone()
        };

        let worker_str = if let Some(ref worker_id) = self.worker_id {
            format!(" [{}]", worker_id)
        } else {
            String::new()
        };

        format!(
            "{} {}{} {}: {}",
            self.timestamp, level_str, worker_str, self.target, self.message
        )
    }
}

/// Extract worker ID from message text
fn extract_worker_id(message: &str) -> Option<String> {
    // Look for patterns like "W1", "worker_id=W2", "[W3]", etc.
    if let Some(idx) = message.find("worker_id=") {
        let start = idx + 10;
        let end = message[start..]
            .find(|c: char| !c.is_alphanumeric())
            .map(|i| start + i)
            .unwrap_or(message.len());
        return Some(message[start..end].to_string());
    }

    // Look for [W\d+] pattern
    if let Some(start) = message.find("[W") {
        if let Some(end) = message[start..].find(']') {
            return Some(message[start + 1..start + end].to_string());
        }
    }

    None
}

/// Colorize log level for terminal display
fn colorize_level(level: &str) -> String {
    match level.to_uppercase().as_str() {
        "TRACE" => format!("\x1b[35m{}\x1b[0m", level), // Magenta
        "DEBUG" => format!("\x1b[36m{}\x1b[0m", level), // Cyan
        "INFO" => format!("\x1b[32m{}\x1b[0m", level),  // Green
        "WARN" => format!("\x1b[33m{}\x1b[0m", level),  // Yellow
        "ERROR" => format!("\x1b[31m{}\x1b[0m", level), // Red
        _ => level.to_string(),
    }
}

/// Log viewer that supports tailing and filtering
pub struct LogViewer {
    log_path: PathBuf,
    level_filter: Option<String>,
    worker_filter: Option<String>,
    colored: bool,
}

impl LogViewer {
    /// Create a new log viewer
    pub fn new(log_path: PathBuf) -> Self {
        LogViewer {
            log_path,
            level_filter: None,
            worker_filter: None,
            colored: atty::is(atty::Stream::Stdout),
        }
    }

    /// Set level filter
    pub fn with_level_filter(mut self, level: Option<String>) -> Self {
        self.level_filter = level;
        self
    }

    /// Set worker filter
    pub fn with_worker_filter(mut self, worker: Option<String>) -> Self {
        self.worker_filter = worker;
        self
    }

    /// Set colored output
    pub fn with_colored(mut self, colored: bool) -> Self {
        self.colored = colored;
        self
    }

    /// Display the last N lines of logs
    pub fn tail(&self, lines: usize) -> Result<()> {
        let file = File::open(&self.log_path)
            .with_context(|| format!("Failed to open log file: {:?}", self.log_path))?;

        let reader = BufReader::new(file);
        let all_lines: Vec<String> = reader.lines().collect::<std::io::Result<Vec<_>>>()?;

        // Filter and display last N lines
        let mut displayed = 0;
        for line in all_lines.iter().rev() {
            if displayed >= lines {
                break;
            }

            if let Ok(entry) = LogEntry::parse(line) {
                if entry.matches_filter(
                    self.level_filter.as_deref(),
                    self.worker_filter.as_deref(),
                ) {
                    println!("{}", entry.format(self.colored));
                    displayed += 1;
                }
            }
        }

        Ok(())
    }

    /// Follow logs in real-time
    pub async fn follow(&self, initial_lines: usize) -> Result<()> {
        // First, display the last N lines
        self.tail(initial_lines)?;

        // Now follow the file for new lines
        let mut file = File::open(&self.log_path)
            .with_context(|| format!("Failed to open log file: {:?}", self.log_path))?;

        // Seek to end of file
        file.seek(SeekFrom::End(0))?;

        let mut reader = BufReader::new(file);
        let mut line = String::new();

        loop {
            // Try to read a line
            let bytes_read = reader.read_line(&mut line)?;

            if bytes_read > 0 {
                // We got a new line
                if let Ok(entry) = LogEntry::parse(&line) {
                    if entry.matches_filter(
                        self.level_filter.as_deref(),
                        self.worker_filter.as_deref(),
                    ) {
                        println!("{}", entry.format(self.colored));
                    }
                }
                line.clear();
            } else {
                // No new data, wait a bit
                sleep(Duration::from_millis(100)).await;
            }
        }
    }
}

/// Get the default log file path for the beads-workers daemon
pub fn get_log_path(beads_dir: &Path) -> PathBuf {
    beads_dir.join("workers.log")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_log() {
        let json_log = r#"{"timestamp":"2024-01-01T12:00:00Z","level":"INFO","target":"test","message":"Test message"}"#;
        let entry = LogEntry::parse(json_log).unwrap();
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.message, "Test message");
    }

    #[test]
    fn test_parse_plain_text_log() {
        let plain_log = "2024-01-01 12:00:00 INFO beads_workers::test: Test message";
        let entry = LogEntry::parse(plain_log).unwrap();
        assert_eq!(entry.level, "INFO");
        assert!(entry.message.contains("Test message"));
    }

    #[test]
    fn test_extract_worker_id() {
        assert_eq!(
            extract_worker_id("Processing task for worker_id=W1"),
            Some("W1".to_string())
        );
        assert_eq!(
            extract_worker_id("Message from [W2]"),
            Some("W2".to_string())
        );
        assert_eq!(extract_worker_id("No worker here"), None);
    }

    #[test]
    fn test_level_filter() {
        let entry = LogEntry {
            timestamp: "2024-01-01T12:00:00Z".to_string(),
            level: "ERROR".to_string(),
            worker_id: None,
            target: "test".to_string(),
            message: "Test".to_string(),
            fields: serde_json::Map::new(),
        };

        assert!(entry.matches_filter(Some("ERROR"), None));
        assert!(!entry.matches_filter(Some("INFO"), None));
        assert!(entry.matches_filter(None, None));
    }

    #[test]
    fn test_worker_filter() {
        let entry = LogEntry {
            timestamp: "2024-01-01T12:00:00Z".to_string(),
            level: "INFO".to_string(),
            worker_id: Some("W1".to_string()),
            target: "test".to_string(),
            message: "Test".to_string(),
            fields: serde_json::Map::new(),
        };

        assert!(entry.matches_filter(None, Some("W1")));
        assert!(!entry.matches_filter(None, Some("W2")));
    }

    #[test]
    fn test_format_entry() {
        let entry = LogEntry {
            timestamp: "2024-01-01T12:00:00Z".to_string(),
            level: "INFO".to_string(),
            worker_id: Some("W1".to_string()),
            target: "test".to_string(),
            message: "Test message".to_string(),
            fields: serde_json::Map::new(),
        };

        let formatted = entry.format(false);
        assert!(formatted.contains("INFO"));
        assert!(formatted.contains("[W1]"));
        assert!(formatted.contains("Test message"));
    }
}
