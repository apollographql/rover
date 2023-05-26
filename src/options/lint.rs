use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct LintOpts {
    /// If the lint should be run and compared to the most recently published schema
    #[arg(long)]
    pub ignore_existing_lint_violations: bool,
}
