use anyhow::Result;

use crate::utils;

pub(crate) fn run() -> Result<()> {
    utils::info("TODO: run cargo fmt --check");
    utils::info("TODO: run cargo clippy --check");
    utils::info("TODO: run cargo test --workspace --locked --target {target}");
    utils::info(
        "TODO: run cargo test --workspace --locked --no-default-features --target {target}",
    );
    Ok(())
}
