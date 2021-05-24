mod commands;
pub(crate) mod utils;
use commands::{dist, help, lint, test};

use anyhow::Result;
use std::env;

fn main() -> Result<()> {
    let mut args = env::args();
    let _xtask = args.next();
    let task = args.next();
    let flag = args.next();
    let verbose = match flag.as_deref() {
        Some("--verbose") | Some("-v") => true,
        _ => false,
    };

    // when adding a new xtask, please update commands/help.rs
    match task.as_deref() {
        Some("dist") => dist::run(verbose)?,
        Some("lint") => lint::run(verbose)?,
        Some("test") => test::run(verbose)?,
        _ => help::run(),
    }
    Ok(())
}
