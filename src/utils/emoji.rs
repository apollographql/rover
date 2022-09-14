use std::fmt::Display;

use console::Emoji as ConsoleEmoji;
use strum_macros::EnumIter;

#[derive(Debug, Copy, Clone, EnumIter)]
pub enum Emoji {
    Person,
    Web,
    Note,
}

impl Emoji {
    fn get(&self) -> &str {
        use Emoji::*;
        match self {
            Person => "🧑 ",
            Web => "🕸️  ",
            Note => "🗒️  ",
        }
    }
}

impl Display for Emoji {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if std::env::var_os("NO_EMOJI").is_some() {
            Ok(())
        } else {
            write!(f, "{}", ConsoleEmoji::new(self.get(), ""))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use strum::IntoEnumIterator;

    #[test]
    fn each_emoji_has_width_of_three() {
        for emoji in Emoji::iter() {
            assert_eq!(console::measure_text_width(&emoji.to_string()), 3)
        }
    }
}
