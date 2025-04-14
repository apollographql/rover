use console::style;

// ----- Symbols -----

/// Returns a styled success checkmark
pub fn success_checkmark() -> String {
    style("✓").green().bold().to_string()
}

/// Formats a success message with a checkmark
pub fn success_message(message: &str) -> String {
    format!("{} {}", success_checkmark(), message)
}

/// Formats text as a clickable hyperlink with custom color (#7DC0FF)
pub fn hyperlink(text: &str, url: &str) -> String {
    let colored_text = format!("\u{001b}[38;2;125;192;255m{}\u{001b}[0m", text);
    format!(
        "\u{001b}]8;;{}\u{001b}\\{}\u{001b}]8;;\u{001b}\\",
        url, colored_text
    )
}
