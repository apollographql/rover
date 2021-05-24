mod build;
mod docs;
mod installers;
mod npm;

use anyhow::Result;

pub(crate) fn run(verbose: bool) -> Result<()> {
    npm::prep(verbose)?;
    installers::prep()?;
    docs::prep()?;
    build::prep()?;
    Ok(())
}
