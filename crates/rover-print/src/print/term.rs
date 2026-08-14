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
        // A failed terminal write (e.g. a closed handle) is unrecoverable and
        // not worth propagating to callers, none of which can do anything
        // about a broken stream. Record a diagnostic for `--log`-enabled
        // runs; log the unstyled text rather than `line`, which may carry
        // ANSI escapes.
        if let Err(error) = self.term.write_line(&line) {
            let message: String = segments.iter().map(|segment| segment.text()).collect();
            tracing::error!(%error, %message, "failed to write to terminal");
        }
    }

    fn render(&self, text: &StyledText) -> String {
        text.paint(self.with_color)
    }
}
