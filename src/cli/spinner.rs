use std::borrow::Cow;
use std::time::Duration;

use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

/// Tick interval for spinner animation.
const TICK_MS: u64 = 80;

/// Indentation prefix for sub-operation output lines.
const INDENT: &str = "  ";

/// 256-color index for spinner dots (208 = orange).
const SPINNER_COLOR: u8 = 208;

/// Create an indented spinner for sub-operations under a header.
#[must_use]
pub fn start(message: impl Into<Cow<'static, str>>) -> ProgressBar {
    create(&format!("{INDENT}{{spinner:.{SPINNER_COLOR}}} {{msg}}"), message)
}

/// Create a root-level spinner (no indent) for top-level operations.
#[must_use]
pub fn start_root(message: impl Into<Cow<'static, str>>) -> ProgressBar {
    create(&format!("{{spinner:.{SPINNER_COLOR}}} {{msg}}"), message)
}

fn create(template: &str, message: impl Into<Cow<'static, str>>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", " "])
            .template(template)
            .expect("valid template"),
    );
    pb.set_message(message.into());
    pb.enable_steady_tick(Duration::from_millis(TICK_MS));
    pb
}

/// Finish the spinner with a green "ok" prefix.
pub fn finish_ok(pb: &ProgressBar, message: &str) {
    pb.finish_and_clear();
    println!("{INDENT}{} {message}", "ok".green());
}

/// Finish the spinner with a red "failed" prefix.
pub fn finish_failed(pb: &ProgressBar, message: &str) {
    pb.finish_and_clear();
    println!("{INDENT}{} {message}", "failed".red());
}

/// Finish the spinner with a yellow "warn" prefix.
pub fn finish_warn(pb: &ProgressBar, message: &str) {
    pb.finish_and_clear();
    println!("{INDENT}{} {message}", "warn".yellow());
}

/// Clear the spinner without printing a status line.
pub fn clear(pb: &ProgressBar) {
    pb.finish_and_clear();
}
