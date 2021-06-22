use crate::tools::Runner;
use crate::utils::CommandOutput;

use anyhow::{anyhow, Context, Result};
use camino::Utf8Path;

pub(crate) struct MakeRunner {
    runner: Runner,
}

impl MakeRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let runner = Runner::new("make", verbose)?;

        Ok(MakeRunner { runner })
    }

    pub(crate) fn test_supergraph_demo(&self, base_dir: &Utf8Path) -> Result<()> {
        let output = self.runner.exec(&["demo"], base_dir, None)?;
        assert_demo_includes(&output)
            .with_context(|| "There were problems with the output of 'make'.")
    }
}

fn assert_demo_includes(output: &CommandOutput) -> Result<()> {
    let necessary_stdout = vec![
        "rover supergraph compose",
        "🚀 Graph Router ready at http://localhost:4000/",
    ];
    let necessary_stderr = vec!["Creating network", "allProducts", "Removing network"];

    let mut missing_strings = Vec::with_capacity(necessary_stderr.len() + necessary_stdout.len());
    for necessary_string in necessary_stdout {
        if !output.stdout.contains(necessary_string) {
            missing_strings.push(necessary_string);
        }
    }
    for necessary_string in necessary_stderr {
        if !output.stderr.contains(necessary_string) {
            missing_strings.push(necessary_string);
        }
    }

    if missing_strings.is_empty() {
        Ok(())
    } else {
        Err(anyhow!(
            "The output from 'make` is missing the following strings: {:?}",
            missing_strings
        ))
    }
}
