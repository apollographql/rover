use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct LintOpts {
    /// Ignore existing lint violations for a published subgraph. If passed, the command will only report lint violations introduced by recent changes.
    #[arg(long)]
    pub ignore_existing_lint_violations: bool,
}
