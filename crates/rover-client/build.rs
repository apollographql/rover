use anyhow::{Context, Result};
use rover_std::{Fs, Style};

fn main() -> Result<()> {
    println!(
        "{}",
        Style::Command.paint("cargo:rerun-if-changed=.schema/schema.graphql")
    );
    Fs::read_file(".schema/schema.graphql")
        .context(format!("no schema found at ./.schema/schema.graphql, which is needed to generate types for Rover's GraphQL queries. You should run `{}` to update the schema before building.", Style::Command.paint("cargo xtask prep --schema-only")))?;
    Ok(())
}
