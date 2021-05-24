use anyhow::Result;

use crate::utils;

pub(crate) fn run(_verbose: bool) -> Result<()> {
    utils::info("TODO: run cargo test --workspace --locked --target {target}");
    utils::info(
        "TODO: run cargo test --workspace --locked --no-default-features --target {target}",
    );
    Ok(())
}
