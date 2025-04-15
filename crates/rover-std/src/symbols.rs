use console::style;

/// Returns a styled success checkmark
pub fn success_checkmark() -> String {
    style("✓").green().bold().to_string()
}

/// Formats a success message with a checkmark
pub fn success_message(message: &str) -> String {
    format!("{} {}", success_checkmark(), message)
}
