use super::Print;
use crate::style::StyledText;

/// Printing for Humans
#[derive(Clone, Debug)]
pub struct Term {
    pub(super) term: console::Term,
    pub(super) with_color: bool,
}

impl Print for Term {
    fn print(&self, message: &StyledText) {
        self.print_line(std::slice::from_ref(message))
    }

    fn print_line(&self, segments: &[StyledText]) {
        let line: String = segments
            .iter()
            .map(|segment| self.render(segment))
            .collect();
        // A failed terminal write (e.g. a closed handle) must not lose the
        // message entirely, so fall back to logging it instead of
        // propagating the error to callers, none of which can do anything
        // about a broken stream anyway.
        if let Err(error) = self.term.write_line(&line) {
            tracing::error!(%error, %line, "failed to write to terminal");
        }
    }

    fn render(&self, text: &StyledText) -> String {
        text.paint(self.with_color)
    }
}
