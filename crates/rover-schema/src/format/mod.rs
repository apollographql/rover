pub mod compact;
pub mod description;
pub mod sdl;

/// Output format for schema commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable description (default for TTY)
    Description,
    /// Token-efficient compact notation (default for piped output)
    Compact,
    /// Raw SDL
    Sdl,
}

/// Detect whether stdout is a TTY.
pub fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

/// Select the appropriate output format based on flags and TTY detection.
/// `is_json` comes from the global `--format json` flag, which is handled
/// at a higher level and suppresses auto-compact.
pub fn select_format(sdl: bool, compact: bool, is_json: bool) -> OutputFormat {
    if sdl {
        OutputFormat::Sdl
    } else if compact {
        OutputFormat::Compact
    } else if !is_json && !is_tty() {
        // Auto-compact when piped and not already in JSON mode
        OutputFormat::Compact
    } else {
        OutputFormat::Description
    }
}
