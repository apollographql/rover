use std::fmt::Display;

use console::Emoji as ConsoleEmoji;

#[derive(Debug, Copy, Clone)]
pub enum Emoji {
    Action,
    Compose,
    Hourglass,
    Listen,
    Memo,
    New,
    Note,
    Person,
    Reload,
    Rocket,
    Skull,
    Sparkle,
    Start,
    Stop,
    Success,
    Warn,
    Watch,
    Web,
}

impl Emoji {
    fn get(&self) -> &str {
        use Emoji::*;
        match self {
            Action => "🎬 ",
            Compose => "🎶 ",
            Hourglass => "⌛ ",
            Listen => "👂 ",
            Memo => "📝 ",
            New => "🐤 ",
            Note => "🗒️  ",
            Person => "🧑 ",
            Reload => "🔃 ",
            Rocket => "🚀 ",
            Skull => "💀 ",
            Sparkle => "✨ ",
            Start => "🛫 ",
            Stop => "✋ ",
            Success => "✅ ",
            Warn => "⚠️  ",
            Watch => "👀 ",
            Web => "🕸️  ",
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
