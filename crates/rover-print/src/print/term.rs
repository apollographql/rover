use super::Print;
use crate::style::StyledText;

/// Printing for Humans
#[derive(Clone, Debug)]
pub struct Term {
    pub(super) term: console::Term,
    pub(super) with_color: bool,
}

impl Print for Term {
    fn print(&self, message: &StyledText) -> std::io::Result<()> {
        self.print_line(std::slice::from_ref(message))
    }

    fn print_line(&self, segments: &[StyledText]) -> std::io::Result<()> {
        let line: String = segments
            .iter()
            .map(|segment| self.render(segment))
            .collect();
        self.term.write_line(&line)
    }

    fn render(&self, text: &StyledText) -> String {
        text.paint(self.with_color)
    }
}
