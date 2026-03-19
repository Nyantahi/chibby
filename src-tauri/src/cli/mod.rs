// Chibby CLI - Styled terminal output
//
// A creative, colorful CLI that stands out from boring plain text.
// Colors are used consistently to help users focus on results:
// - Green: Success, passed, good
// - Red: Failed, error, bad
// - Blue/Cyan: Running, in progress
// - Yellow: Warning, skipped, cancelled
// - Magenta: Secrets, keys, sensitive
// - White/Bold: Important info, headers

use console::Term;
use indicatif::{ProgressBar, ProgressStyle};
use owo_colors::OwoColorize;
use std::time::Duration;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ASCII Art Banner
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Main ASCII art banner
pub const BANNER: &str = r#"
     _____ _     _ _     _           
    / ____| |   (_) |   | |          
   | |    | |__  _| |__ | |__  _   _ 
   | |    | '_ \| | '_ \| '_ \| | | |
   | |____| | | | | |_) | |_) | |_| |
    \_____|_| |_|_|_.__/|_.__/ \__, |
                                __/ |
    local-first CI/CD          |___/ 
"#;

/// Compact single-line banner
pub const BANNER_INLINE: &str = "▸ chibby";

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Icons - Unicode symbols that work in most terminals
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub mod icons {
    // Status icons
    pub const SUCCESS: &str = "✓";
    pub const FAILURE: &str = "✗";
    pub const RUNNING: &str = "●";
    pub const PENDING: &str = "○";
    pub const SKIP: &str = "⊘";
    pub const WARN: &str = "⚠";
    pub const INFO: &str = "ℹ";
    pub const CANCELLED: &str = "◌";

    // Action icons
    pub const ARROW_RIGHT: &str = "→";
    pub const ARROW_DOWN: &str = "↓";
    pub const PLAY: &str = "▶";
    pub const STOP: &str = "■";
    pub const RETRY: &str = "↻";
    pub const ROLLBACK: &str = "↩";

    // Object icons
    pub const ROCKET: &str = "🚀";
    pub const PACKAGE: &str = "📦";
    pub const LOCK: &str = "🔒";
    pub const KEY: &str = "🔑";
    pub const FOLDER: &str = "📁";
    pub const FILE: &str = "📄";
    pub const CLOCK: &str = "⏱";
    pub const SPARKLE: &str = "✨";
    pub const FIRE: &str = "🔥";
    pub const LINK: &str = "🔗";
    pub const GEAR: &str = "⚙";
    pub const SHIELD: &str = "🛡";
    pub const BUG: &str = "🐛";
    pub const CHECK: &str = "☑";
    pub const UNCHECK: &str = "☐";

    // Pipeline stage icons
    pub const BUILD: &str = "🔨";
    pub const TEST: &str = "🧪";
    pub const DEPLOY: &str = "🚀";
    pub const SIGN: &str = "✍";
    pub const SCAN: &str = "🔍";
    pub const NOTIFY: &str = "🔔";
    pub const CLEAN: &str = "🧹";
    pub const VERSION: &str = "🏷";

    // Decorative
    pub const DOT: &str = "·";
    pub const BULLET: &str = "•";
    pub const PIPE: &str = "│";
    pub const CORNER: &str = "└";
    pub const TEE: &str = "├";
    pub const DASH: &str = "─";
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Stage Status
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StageStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
    Cancelled,
}

impl From<&str> for StageStatus {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "running" => StageStatus::Running,
            "success" => StageStatus::Success,
            "failed" => StageStatus::Failed,
            "skipped" => StageStatus::Skipped,
            "cancelled" => StageStatus::Cancelled,
            _ => StageStatus::Pending,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Styled Printer - Main output interface
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct Printer {
    term: Term,
    verbose: bool,
}

impl Printer {
    pub fn new(verbose: bool) -> Self {
        Self {
            term: Term::stderr(),
            verbose,
        }
    }

    /// Print the startup banner with gradient colors
    pub fn banner(&self) {
        println!();
        for (i, line) in BANNER.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            // Gradient from cyan to blue
            match i % 4 {
                0 => println!("{}", line.bright_cyan().bold()),
                1 => println!("{}", line.cyan()),
                2 => println!("{}", line.blue()),
                _ => println!("{}", line.bright_blue()),
            }
        }
        println!();
    }

    /// Print a section header with decorative line
    pub fn header(&self, text: &str) {
        let line_len = 50 - text.len().min(40);
        let line = icons::DASH.repeat(line_len);
        println!();
        println!(
            " {} {} {}",
            "━━━".bright_black(),
            text.bold().white(),
            line.bright_black()
        );
        println!();
    }

    /// Print a sub-header
    pub fn subheader(&self, text: &str) {
        println!();
        println!("   {} {}", icons::BULLET.bright_black(), text.white().bold());
    }

    // ─────────────────────────────────────────────────────────────
    // Status Messages
    // ─────────────────────────────────────────────────────────────

    /// Print success message (green)
    pub fn success(&self, msg: &str) {
        println!(
            "  {} {}",
            icons::SUCCESS.green().bold(),
            msg.green()
        );
    }

    /// Print error message (red)
    pub fn error(&self, msg: &str) {
        eprintln!(
            "  {} {}",
            icons::FAILURE.red().bold(),
            msg.red().bold()
        );
    }

    /// Print warning message (yellow)
    pub fn warn(&self, msg: &str) {
        println!(
            "  {} {}",
            icons::WARN.yellow().bold(),
            msg.yellow()
        );
    }

    /// Print info message (blue)
    pub fn info(&self, msg: &str) {
        println!(
            "  {} {}",
            icons::INFO.blue(),
            msg.bright_white()
        );
    }

    /// Print debug message (only in verbose mode)
    pub fn debug(&self, msg: &str) {
        if self.verbose {
            println!(
                "  {} {}",
                icons::DOT.bright_black(),
                msg.bright_black()
            );
        }
    }

    // ─────────────────────────────────────────────────────────────
    // Key-Value and Structured Output
    // ─────────────────────────────────────────────────────────────

    /// Print a key-value pair
    pub fn kv(&self, key: &str, value: &str) {
        println!(
            "    {} {} {}",
            key.bright_black(),
            icons::ARROW_RIGHT.bright_black(),
            value.white()
        );
    }

    /// Print a key-value pair with colored value
    pub fn kv_colored(&self, key: &str, value: &str, status: StageStatus) {
        let styled_value = match status {
            StageStatus::Success => value.green().to_string(),
            StageStatus::Failed => value.red().to_string(),
            StageStatus::Running => value.blue().to_string(),
            StageStatus::Skipped => value.bright_black().to_string(),
            StageStatus::Cancelled => value.yellow().to_string(),
            StageStatus::Pending => value.white().to_string(),
        };
        println!(
            "    {} {} {}",
            key.bright_black(),
            icons::ARROW_RIGHT.bright_black(),
            styled_value
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Stage Display
    // ─────────────────────────────────────────────────────────────

    /// Print a stage with status icon and color
    pub fn stage(&self, name: &str, status: StageStatus) {
        let (icon, styled_name) = match status {
            StageStatus::Running => (
                icons::RUNNING.blue().bold().to_string(),
                name.blue().bold().to_string(),
            ),
            StageStatus::Success => (
                icons::SUCCESS.green().bold().to_string(),
                name.green().to_string(),
            ),
            StageStatus::Failed => (
                icons::FAILURE.red().bold().to_string(),
                name.red().bold().to_string(),
            ),
            StageStatus::Skipped => (
                icons::SKIP.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
            StageStatus::Cancelled => (
                icons::CANCELLED.yellow().to_string(),
                name.yellow().to_string(),
            ),
            StageStatus::Pending => (
                icons::PENDING.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
        };
        println!("  {} {}", icon, styled_name);
    }

    /// Print a stage with duration
    pub fn stage_with_duration(&self, name: &str, status: StageStatus, duration_ms: Option<u64>) {
        let (icon, styled_name) = match status {
            StageStatus::Running => (
                icons::RUNNING.blue().bold().to_string(),
                name.blue().bold().to_string(),
            ),
            StageStatus::Success => (
                icons::SUCCESS.green().bold().to_string(),
                name.green().to_string(),
            ),
            StageStatus::Failed => (
                icons::FAILURE.red().bold().to_string(),
                name.red().bold().to_string(),
            ),
            StageStatus::Skipped => (
                icons::SKIP.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
            StageStatus::Cancelled => (
                icons::CANCELLED.yellow().to_string(),
                name.yellow().to_string(),
            ),
            StageStatus::Pending => (
                icons::PENDING.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
        };

        if let Some(ms) = duration_ms {
            let duration = format_duration(ms);
            println!(
                "  {} {} {}",
                icon,
                styled_name,
                format!("({})", duration).bright_black()
            );
        } else {
            println!("  {} {}", icon, styled_name);
        }
    }

    /// Print a stage with icon prefix (e.g., build icon for build stage)
    pub fn stage_typed(&self, stage_type: &str, name: &str, status: StageStatus) {
        let type_icon = match stage_type.to_lowercase().as_str() {
            "build" => icons::BUILD,
            "test" => icons::TEST,
            "deploy" => icons::DEPLOY,
            "sign" | "signing" => icons::SIGN,
            "scan" | "security" => icons::SCAN,
            "notify" | "notification" => icons::NOTIFY,
            "clean" | "cleanup" => icons::CLEAN,
            "version" | "bump" => icons::VERSION,
            "preflight" => icons::SHIELD,
            _ => icons::GEAR,
        };

        let (status_icon, styled_name) = match status {
            StageStatus::Running => (
                icons::RUNNING.blue().bold().to_string(),
                name.blue().bold().to_string(),
            ),
            StageStatus::Success => (
                icons::SUCCESS.green().bold().to_string(),
                name.green().to_string(),
            ),
            StageStatus::Failed => (
                icons::FAILURE.red().bold().to_string(),
                name.red().bold().to_string(),
            ),
            StageStatus::Skipped => (
                icons::SKIP.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
            StageStatus::Cancelled => (
                icons::CANCELLED.yellow().to_string(),
                name.yellow().to_string(),
            ),
            StageStatus::Pending => (
                icons::PENDING.bright_black().to_string(),
                name.bright_black().to_string(),
            ),
        };

        println!("  {} {} {}", type_icon, status_icon, styled_name);
    }

    // ─────────────────────────────────────────────────────────────
    // Command and Log Output
    // ─────────────────────────────────────────────────────────────

    /// Print command being executed
    pub fn cmd(&self, cmd: &str) {
        if self.verbose {
            println!(
                "     {} {}",
                "$".bright_black(),
                cmd.bright_black().italic()
            );
        }
    }

    /// Print log line with styling based on type
    pub fn log(&self, log_type: &str, line: &str) {
        let (prefix, styled_line) = match log_type {
            "stdout" => (
                icons::PIPE.bright_black().to_string(),
                line.white().to_string(),
            ),
            "stderr" => (
                icons::PIPE.yellow().to_string(),
                line.yellow().to_string(),
            ),
            "error" => (
                icons::PIPE.red().to_string(),
                line.red().to_string(),
            ),
            "cmd" => (
                "$".bright_black().to_string(),
                line.bright_black().italic().to_string(),
            ),
            "info" => (
                icons::INFO.blue().to_string(),
                line.bright_white().to_string(),
            ),
            "warn" => (
                icons::WARN.yellow().to_string(),
                line.yellow().to_string(),
            ),
            _ => (
                icons::PIPE.bright_black().to_string(),
                line.bright_black().to_string(),
            ),
        };
        println!("     {} {}", prefix, styled_line);
    }

    // ─────────────────────────────────────────────────────────────
    // Project Display
    // ─────────────────────────────────────────────────────────────

    /// Print a project entry
    pub fn project(&self, name: &str, path: &str, has_pipeline: bool) {
        let status = if has_pipeline {
            icons::SUCCESS.green().to_string()
        } else {
            icons::PENDING.yellow().to_string()
        };
        println!(
            "  {} {} {}",
            status,
            name.white().bold(),
            format!("({})", path).bright_black()
        );
    }

    /// Print a project with last run status
    pub fn project_with_status(&self, name: &str, path: &str, last_status: Option<StageStatus>) {
        let status_icon = match last_status {
            Some(StageStatus::Success) => icons::SUCCESS.green().to_string(),
            Some(StageStatus::Failed) => icons::FAILURE.red().to_string(),
            Some(StageStatus::Running) => icons::RUNNING.blue().to_string(),
            _ => icons::PENDING.bright_black().to_string(),
        };
        println!(
            "  {} {} {}",
            status_icon,
            name.white().bold(),
            format!("({})", path).bright_black()
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Run Summary
    // ─────────────────────────────────────────────────────────────

    /// Print pipeline run summary
    pub fn run_summary(&self, status: &str, duration_ms: u64, stages_passed: usize, stages_total: usize) {
        println!();
        let duration = format_duration(duration_ms);
        let progress = format!("{}/{}", stages_passed, stages_total);

        match status {
            "success" => {
                println!(
                    "  {} {} in {} {}",
                    icons::SPARKLE,
                    "Pipeline succeeded".green().bold(),
                    duration.cyan().bold(),
                    format!("({})", progress).bright_black()
                );
            }
            "failed" => {
                println!(
                    "  {} {} after {} {}",
                    icons::FIRE,
                    "Pipeline failed".red().bold(),
                    duration.cyan().bold(),
                    format!("({})", progress).bright_black()
                );
            }
            "cancelled" => {
                println!(
                    "  {} {} after {} {}",
                    icons::WARN,
                    "Pipeline cancelled".yellow().bold(),
                    duration.cyan().bold(),
                    format!("({})", progress).bright_black()
                );
            }
            "running" => {
                println!(
                    "  {} {} {} {}",
                    icons::RUNNING.blue(),
                    "Pipeline running".blue().bold(),
                    duration.cyan(),
                    format!("({})", progress).bright_black()
                );
            }
            _ => {
                println!(
                    "  {} {} {} {}",
                    icons::INFO,
                    format!("Status: {}", status).white(),
                    duration.cyan(),
                    format!("({})", progress).bright_black()
                );
            }
        }
        println!();
    }

    // ─────────────────────────────────────────────────────────────
    // History Display
    // ─────────────────────────────────────────────────────────────

    /// Print a history entry
    pub fn history_entry(
        &self,
        id: &str,
        status: StageStatus,
        when: &str,
        duration_ms: u64,
        environment: Option<&str>,
    ) {
        let icon = match status {
            StageStatus::Success => icons::SUCCESS.green().to_string(),
            StageStatus::Failed => icons::FAILURE.red().to_string(),
            StageStatus::Running => icons::RUNNING.blue().to_string(),
            StageStatus::Cancelled => icons::CANCELLED.yellow().to_string(),
            _ => icons::PENDING.bright_black().to_string(),
        };

        let duration = format_duration(duration_ms);
        let env_str = environment
            .map(|e| format!(" {}", e.cyan()))
            .unwrap_or_default();

        println!(
            "  {} {} {} {}{}",
            icon,
            id.bright_black(),
            when.white(),
            format!("({})", duration).bright_black(),
            env_str
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Preflight Display
    // ─────────────────────────────────────────────────────────────

    /// Print preflight check result
    pub fn preflight_check(&self, name: &str, passed: bool, message: Option<&str>) {
        if passed {
            println!(
                "  {} {}",
                icons::SUCCESS.green().bold(),
                name.green()
            );
        } else {
            println!(
                "  {} {}",
                icons::FAILURE.red().bold(),
                name.red()
            );
            if let Some(msg) = message {
                println!(
                    "     {} {}",
                    icons::ARROW_RIGHT.red(),
                    msg.red()
                );
            }
        }
    }

    /// Print preflight warning (not blocking)
    pub fn preflight_warn(&self, name: &str, message: &str) {
        println!(
            "  {} {}",
            icons::WARN.yellow().bold(),
            name.yellow()
        );
        println!(
            "     {} {}",
            icons::ARROW_RIGHT.yellow(),
            message.bright_black()
        );
    }

    // ─────────────────────────────────────────────────────────────
    // Secrets Display
    // ─────────────────────────────────────────────────────────────

    /// Print secret status (masked value)
    pub fn secret(&self, key: &str, is_set: bool) {
        let icon = if is_set {
            icons::LOCK.green().to_string()
        } else {
            icons::KEY.yellow().to_string()
        };
        let status = if is_set {
            "configured".green().to_string()
        } else {
            "not set".yellow().to_string()
        };
        println!("  {} {} {}", icon, key.white(), status);
    }

    // ─────────────────────────────────────────────────────────────
    // Decorative Elements
    // ─────────────────────────────────────────────────────────────

    /// Print a horizontal divider
    pub fn divider(&self) {
        println!(
            "  {}",
            "─".repeat(48).bright_black()
        );
    }

    /// Print empty line
    pub fn newline(&self) {
        println!();
    }

    /// Print a count/stats line
    pub fn stats(&self, label: &str, count: usize, color: StageStatus) {
        let styled_count = match color {
            StageStatus::Success => count.to_string().green().bold().to_string(),
            StageStatus::Failed => count.to_string().red().bold().to_string(),
            StageStatus::Running => count.to_string().blue().bold().to_string(),
            _ => count.to_string().white().bold().to_string(),
        };
        println!(
            "    {} {} {}",
            " ".on_bright_black(),
            styled_count,
            label.bright_black()
        );
    }

    /// Clear the terminal
    pub fn clear(&self) {
        let _ = self.term.clear_screen();
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Progress Indicators
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Create a spinner for long-running operations
pub fn spinner(msg: &str) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("  {spinner:.cyan} {msg}")
            .expect("Invalid spinner template"),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb
}

/// Create a progress bar for multi-step operations
pub fn progress_bar(total: u64, msg: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {msg} [{bar:30.cyan/bright_black}] {pos}/{len}")
            .expect("Invalid progress bar template")
            .progress_chars("━━╸"),
    );
    pb.set_message(msg.to_string());
    pb
}

/// Create a progress bar for downloads/uploads
pub fn download_bar(total: u64) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("  {spinner:.green} [{bar:30.green/bright_black}] {bytes}/{total_bytes} {bytes_per_sec}")
            .expect("Invalid download bar template")
            .progress_chars("━━╸"),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Helper Functions
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Format duration in human-readable form
pub fn format_duration(ms: u64) -> String {
    if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else if ms < 3_600_000 {
        let mins = ms / 60_000;
        let secs = (ms % 60_000) / 1000;
        format!("{}m {}s", mins, secs)
    } else {
        let hours = ms / 3_600_000;
        let mins = (ms % 3_600_000) / 60_000;
        format!("{}h {}m", hours, mins)
    }
}

/// Format relative time
pub fn format_relative_time(seconds_ago: i64) -> String {
    if seconds_ago < 60 {
        "just now".to_string()
    } else if seconds_ago < 3600 {
        format!("{}m ago", seconds_ago / 60)
    } else if seconds_ago < 86400 {
        format!("{}h ago", seconds_ago / 3600)
    } else {
        format!("{}d ago", seconds_ago / 86400)
    }
}

/// Check if terminal supports Unicode
pub fn supports_unicode() -> bool {
    std::env::var("TERM")
        .map(|t| !t.contains("dumb"))
        .unwrap_or(true)
        && std::env::var("LANG")
            .map(|l| l.contains("UTF") || l.contains("utf"))
            .unwrap_or(true)
}

/// Check if terminal supports colors
pub fn supports_color() -> bool {
    console::colors_enabled_stderr()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(500), "500ms");
        assert_eq!(format_duration(1500), "1.5s");
        assert_eq!(format_duration(65000), "1m 5s");
        assert_eq!(format_duration(3665000), "1h 1m");
    }

    #[test]
    fn test_stage_status_from_str() {
        assert_eq!(StageStatus::from("running"), StageStatus::Running);
        assert_eq!(StageStatus::from("SUCCESS"), StageStatus::Success);
        assert_eq!(StageStatus::from("Failed"), StageStatus::Failed);
        assert_eq!(StageStatus::from("unknown"), StageStatus::Pending);
    }
}
