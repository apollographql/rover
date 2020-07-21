use console::style;
use env_logger::{Builder, Target};
use std::io::Write;

pub fn init() {
    Builder::from_default_env()
        .format(|buf, record| writeln!(buf, "[{}] {}", style(record.level()).cyan(), record.args()))
        .filter(None, log::LevelFilter::Debug)
        .target(Target::Stdout)
        .init();
}
