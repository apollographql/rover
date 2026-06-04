use std::fmt;

#[cfg(any(test, feature = "testing"))]
use super::MockPrint;
use super::{Print, Term};
use crate::style::StyledText;

/// Printer that writes to standard output.
pub struct Stdout<P>(P)
where
    P: Print;

/// Construct a real terminal printer for stdout with explicit color handling.
pub fn term(with_color: bool) -> Stdout<Term> {
    Stdout(Term {
        term: console::Term::stdout(),
        with_color,
    })
}

/// Construct a stdout printer with color auto-detected from the environment.
pub fn default() -> Stdout<Term> {
    term(detect_color_settings())
}

#[cfg(any(test, feature = "testing"))]
pub fn mock() -> Stdout<MockPrint> {
    Stdout(MockPrint::new())
}

impl Default for Stdout<Term> {
    fn default() -> Self {
        default()
    }
}

// Implemented separately to avoid requiring type constraints throughout consumer code
impl<P> fmt::Debug for Stdout<P>
where
    P: Print + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_tuple("Stdout").field(&self.0).finish()
    }
}

impl<P> Print for Stdout<P>
where
    P: Print,
{
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        self.0.print(message)
    }

    fn print_line(&self, segments: &[StyledText]) -> std::io::Result<()> {
        self.0.print_line(segments)
    }

    fn render(&self, text: &StyledText) -> String {
        self.0.render(text)
    }
}

/// Color decision for stdout: `console`'s `NO_COLOR`/`CLICOLOR`/TTY detection,
/// plus Apollo's `APOLLO_NO_COLOR` opt-out.
fn detect_color_settings() -> bool {
    console::colors_enabled() && !crate::print::is_apollo_no_color_set()
}
