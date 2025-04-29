use anyhow::{Context, Result};
use rover_std::Fs;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=.schema/schema.graphql");
    Fs::read_file(".schema/schema.graphql")
        .context("no schema found at ./.schema/schema.graphql, which is needed to generate types for Rover's GraphQL queries. You should run `cargo xtask prep --schema-only` to update the schema before building.")?;
    Ok(())
}
