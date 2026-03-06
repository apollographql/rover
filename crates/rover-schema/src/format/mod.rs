pub mod compact;
pub mod description;
pub mod sdl;

// Unicode box-drawing and symbol constants used across formatters.
pub(crate) const DEPRECATED_MARKER: char = '\u{26a0}'; // ⚠
pub(crate) const SEPARATOR: char = '\u{203a}'; // ›
pub(crate) const ARROW: char = '\u{2192}'; // →
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
