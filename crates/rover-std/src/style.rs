use console::style;

pub enum Style {
    Link,          // URLs and graph refs
    Command,       // Commands, inline code, env variable keys, and profile names
    Path,          // File paths
    HintPrefix,    // "HINT:" text
    WarningPrefix, // "WARN:" text
    ErrorPrefix,   // "ERROR:", "error:", and "error[code]:" text
    Heading,
    CallToAction,
    WhoAmIKey,
    Version,
}

impl Style {
    pub fn paint<S: AsRef<str>>(&self, message: S) -> String {
        let message_ref = message.as_ref();

        if should_disable_color() {
            return message_ref.to_string();
        }

        match &self {
            Style::Link => style(message_ref).cyan(),
            Style::Command => style(message_ref).yellow(),
            Style::CallToAction => style(message_ref).yellow().italic(),
            Style::WhoAmIKey => style(message_ref).green(),
            Style::HintPrefix => style(message_ref).cyan().bold(),
            Style::WarningPrefix => style(message_ref).red(),
            Style::ErrorPrefix => style(message_ref).red().bold(),
            Style::Version => style(message_ref).cyan(),
            Style::Path | Style::Heading => style(message_ref).bold(),
        }
        .to_string()
    }
}

fn should_disable_color() -> bool {
    is_bool_env_var_set("NO_COLOR")
        || is_bool_env_var_set("APOLLO_NO_COLOR")
        || !atty::is(atty::Stream::Stdout)
        || !atty::is(atty::Stream::Stderr)
}

fn is_bool_env_var_set(key: &str) -> bool {
    !matches!(
        std::env::var(key).as_deref(),
        Err(..) | Ok("") | Ok("0") | Ok("false") | Ok("False") | Ok("FALSE")
    )
}
