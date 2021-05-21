mod commands;
pub(crate) mod utils;
use commands::{dist, help, test};

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let task = env::args().nth(1);

    // when adding a new xtask, please update commands/help.rs
    match task.as_deref() {
        Some("dist") => dist::run()?,
        Some("test") => test::run()?,
        _ => help::run(),
    }
    Ok(())
}
