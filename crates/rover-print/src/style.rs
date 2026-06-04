use console::style;

pub struct StyledText {
    style: Style,
    message: String,
}

impl StyledText {
    pub fn new<S: Into<String>>(style: Style, value: S) -> StyledText {
        StyledText {
            style,
            message: value.into(),
        }
    }

    /// An unstyled segment — text that should print verbatim.
    pub fn plain<S: Into<String>>(value: S) -> StyledText {
        StyledText::new(Style::None, value)
    }

    pub const fn style(&self) -> &Style {
        &self.style
    }

    pub fn text(&self) -> &str {
        &self.message
    }

    /// Render the message, forcing color on or off via `with_color` rather
    /// than letting `console` re-derive it from the ambient stream state.
    pub fn paint(&self, with_color: bool) -> String {
        let message_ref = &self.message;

        match &self.style {
            Style::PersistedQueryList | Style::Version => style(message_ref).cyan(),
            Style::Link => style(message_ref).underlined().bold(),
            Style::Command | Style::TotalOperationCount | Style::GraphRef => {
                style(message_ref).cyan()
            }
            Style::Prompt => style(message_ref).bold(),
            Style::CallToAction => style(message_ref).yellow().italic(),
            Style::Failure => style(message_ref).red().bold(),
            Style::WhoAmIKey | Style::NewOperationCount => style(message_ref).green(),
            Style::Hint => style(message_ref).cyan().bold(),
            Style::Info => style(message_ref).blue().bold(),
            Style::Warning => style(message_ref).yellow(),
            Style::Error => style(message_ref).red().bold(),
            Style::Variant => style(message_ref).white().bold(),
            Style::FilePath | Style::Heading => style(message_ref).bold(),
            Style::Pending => style(message_ref).yellow(),
            Style::Success => style(message_ref).green(),
            Style::WarningHeading => style(message_ref).yellow().bold(),
            Style::File => style(message_ref).magenta(),
            Style::SuccessHeading => style(message_ref).green().bold(),
            Style::None => style(message_ref),
        }
        .force_styling(with_color)
        .to_string()
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Style {
    Link,    // URLs and graph refs
    Command, // Commands, inline code, env variable keys, and profile names
    Failure,
    FilePath, // File paths
    Pending,
    Hint,
    Info,
    Warning,
    Error,
    Success,
    Heading,
    CallToAction,
    WhoAmIKey,
    Variant,
    Version,
    TotalOperationCount,
    NewOperationCount,
    PersistedQueryList,
    Prompt,
    WarningHeading,
    File,
    SuccessHeading,
    GraphRef,
    None,
}
