pub mod compact;
pub mod description;
pub mod sdl;

// Unicode box-drawing and symbol constants used across formatters.
pub(crate) const DEPRECATED_MARKER: char = '\u{26a0}'; // ⚠
pub(crate) const SEPARATOR: char = '\u{203a}'; // ›
pub(crate) const ARROW: char = '\u{2192}'; // →
pub(crate) const DASH: char = '\u{2500}'; // ─
pub(crate) const DOTTED: char = '\u{2508}'; // ┈
pub(crate) const HOOK_ARROW: char = '\u{21b3}'; // ↳

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
