//! Test utilities for exercising code that prints via [`Print`]. Gated behind
//! the `testing` feature so downstream crates can assert on what would be
//! printed without writing to a real terminal.

use std::cell::RefCell;

use super::Print;
use crate::style::StyledText;

/// A [`Print`] implementation that records each line that would be written
/// instead of emitting to a terminal, so tests can assert on the exact output.
/// `with_color` mirrors the real `term(bool)` color toggle.
pub struct TerminalCapture {
    with_color: bool,
    lines: RefCell<Vec<String>>,
}

impl TerminalCapture {
    /// Create a recorder that renders with color enabled or disabled.
    pub const fn new(with_color: bool) -> Self {
        Self {
            with_color,
            lines: RefCell::new(Vec::new()),
        }
    }

    /// A snapshot of the lines recorded so far.
    pub fn lines(&self) -> Vec<String> {
        self.lines.borrow().clone()
    }
}

impl Print for TerminalCapture {
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        self.lines.borrow_mut().push(self.render(message));
        Ok(())
    }

    fn print_line(&self, segments: &[StyledText]) -> std::io::Result<()> {
        let line: String = segments
            .iter()
            .map(|segment| self.render(segment))
            .collect();
        self.lines.borrow_mut().push(line);
        Ok(())
    }

    fn render(&self, text: &StyledText) -> String {
        text.paint(self.with_color)
    }
}
