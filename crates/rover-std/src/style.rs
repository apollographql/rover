use ansi_term::Colour::{Cyan, Green, Red, Yellow};

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
            Style::Link => Cyan.normal().paint(message_ref).to_string(),
            Style::Command => Yellow.normal().paint(message_ref).to_string(),
            Style::CallToAction => Yellow.italic().paint(message_ref).to_string(),
            Style::WhoAmIKey => Green.normal().paint(message_ref).to_string(),
            Style::HintPrefix => Cyan.bold().paint(message_ref).to_string(),
            Style::WarningPrefix => Red.normal().paint(message_ref).to_string(),
            Style::ErrorPrefix => Red.bold().paint(message_ref).to_string(),
            Style::Version => Cyan.normal().paint(message_ref).to_string(),
            Style::Path | Style::Heading => ansi_term::Style::new()
                .bold()
                .paint(message_ref)
                .to_string(),
        }
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
