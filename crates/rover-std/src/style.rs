use console::style;

pub enum Style {
    Link,    // URLs and graph refs
    Command, // Commands, inline code, env variable keys, and profile names
    Failure,
    Path, // File paths
    Pending,
    HintPrefix,    // "HINT:" text
    InfoPrefix,    // "==>": text
    DebugPrefix,   // "DEBUG" text
    WarningPrefix, // "WARN:" text
    ErrorPrefix,   // "ERROR:", "error:", and "error[code]:" text
    Heading,
    CallToAction,
    WhoAmIKey,
    Variant,
    Version,
    Success,
    TotalOperationCount,
    NewOperationCount,
    PersistedQueryList,
}

impl Style {
    pub fn paint<S: AsRef<str>>(&self, message: S) -> String {
        let message_ref = message.as_ref();

        if is_no_color_set() {
            return message_ref.to_string();
        }

        match &self {
            Style::Link => style(message_ref).cyan().underlined(),
            Style::PersistedQueryList | Style::Version => style(message_ref).cyan(),
            Style::Command | Style::TotalOperationCount => style(message_ref).yellow(),
            Style::CallToAction => style(message_ref).yellow().italic(),
            Style::Failure => style(message_ref).red(),
            Style::WhoAmIKey | Style::NewOperationCount => style(message_ref).green(),
            Style::HintPrefix => style(message_ref).cyan().bold(),
            Style::InfoPrefix => style(message_ref).blue().bold(),
            Style::DebugPrefix => style(message_ref).color256(8).bold(),
            Style::WarningPrefix => style(message_ref).yellow(),
            Style::ErrorPrefix => style(message_ref).red().bold(),
            Style::Variant => style(message_ref).white().bold(),
            Style::Path | Style::Heading => style(message_ref).bold(),
            Style::Pending => style(message_ref).yellow(),
            Style::Success => style(message_ref).green(),
        }
        .to_string()
    }
}

pub fn is_no_color_set() -> bool {
    is_bool_env_var_set("NO_COLOR") || is_bool_env_var_set("APOLLO_NO_COLOR")
}

fn is_bool_env_var_set(key: &str) -> bool {
    !matches!(
        std::env::var(key).as_deref(),
        Err(..) | Ok("") | Ok("0") | Ok("false") | Ok("False") | Ok("FALSE")
    )
}
