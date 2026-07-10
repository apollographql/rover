use std::fmt;

#[cfg(any(test, feature = "testing"))]
use super::MockPrint;
use super::{Print, Term};
use crate::style::StyledText;

/// Printer that writes to standard error.
pub struct Stderr<P>(P)
where
    P: Print;

/// Construct a real terminal printer for stderr with explicit color handling.
pub fn term(with_color: bool) -> Stderr<Term> {
    Stderr(Term {
        term: console::Term::stderr(),
        with_color,
    })
}

/// Construct a stderr printer with color auto-detected from the environment.
pub fn default() -> Stderr<Term> {
    term(detect_color_settings())
}

#[cfg(any(test, feature = "testing"))]
pub fn mock() -> Stderr<MockPrint> {
    Stderr(MockPrint::new())
}

impl Default for Stderr<Term> {
    fn default() -> Self {
        default()
    }
}

// Implemented separately to avoid requiring type constraints throughout consumer code
impl<P> fmt::Debug for Stderr<P>
where
    P: Print + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_tuple("Stderr").field(&self.0).finish()
    }
}

impl<P> Print for Stderr<P>
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

/// Color decision for stderr: `console`'s `NO_COLOR`/`CLICOLOR`/TTY detection,
/// plus Apollo's `APOLLO_NO_COLOR` opt-out.
fn detect_color_settings() -> bool {
    console::colors_enabled_stderr() && !crate::print::is_apollo_no_color_set()
}
