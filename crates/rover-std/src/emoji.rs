use std::fmt::Display;

use console::Emoji as ConsoleEmoji;

#[derive(Debug, Copy, Clone)]
pub enum Emoji {
    Hourglass,
    Person,
    Web,
    Note,
    Rocket,
    Stop,
    Success,
    Watch,
    Reload,
    Listen,
    Start,
    New,
    Sparkle,
    Skull,
    Compose,
    Warn,
}

impl Emoji {
    fn get(&self) -> &str {
        use Emoji::*;
        match self {
            Hourglass => "⌛ ",
            Person => "🧑 ",
            Web => "🕸️  ",
            Note => "🗒️  ",
            Rocket => "🚀 ",
            Stop => "✋ ",
            Success => "✅ ",
            Watch => "👀 ",
            Reload => "🔃 ",
            Listen => "👂 ",
            Start => "🛫 ",
            New => "🐤 ",
            Sparkle => "✨ ",
            Skull => "💀 ",
            Compose => "🎶 ",
            Warn => "⚠️  ",
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
