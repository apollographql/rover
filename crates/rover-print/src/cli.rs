use clap::builder::{
    Styles,
    styling::{AnsiColor, Effects},
};

pub const fn default_styles(with_color: bool) -> Styles {
    if with_color {
        Styles::styled()
            .header(AnsiColor::Green.on_default().effects(Effects::BOLD))
            .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
            .literal(AnsiColor::Cyan.on_default().effects(Effects::BOLD))
            .placeholder(AnsiColor::Cyan.on_default())
    } else {
        Styles::plain()
    }
}
