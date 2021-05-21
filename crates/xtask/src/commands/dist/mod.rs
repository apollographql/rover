mod build;
mod docs;
mod installers;
mod npm;

use anyhow::Result;

pub(crate) fn run() -> Result<()> {
    npm::prep()?;
    installers::prep()?;
    docs::prep()?;
    build::prep()?;
    Ok(())
}
