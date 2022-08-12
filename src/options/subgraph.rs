use dialoguer::Input;
use saucer::{
    clap::{self, ErrorKind as ClapErrorKind},
    CommandFactory, Parser,
};
use serde::{Deserialize, Serialize};

use crate::{cli::Rover, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct SubgraphOpt {
    /// The name of the subgraph
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct OptionalSubgraphOpt {
    /// The name of the subgraph. This must be unique to each `rover dev` session.
    #[clap(long = "name")]
    #[serde(skip_serializing)]
    pub subgraph_name: Option<String>,
}

impl OptionalSubgraphOpt {
    pub fn prompt_for_name(&self) -> Result<String> {
        if let Some(name) = &self.subgraph_name {
            Ok(name.to_string())
        } else if atty::is(atty::Stream::Stderr) {
            let mut input = Input::new();
            input.with_prompt("what is the name of this subgraph?");
            if let Some(dirname) = Self::maybe_name_from_dir() {
                input.default(dirname);
            }
            let name: String = input.interact_text()?;
            Ok(name)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "--name <SUBGRAPH_NAME> is required when not attached to a TTY",
            )
            .exit();
        }
    }

    fn maybe_name_from_dir() -> Option<String> {
        std::env::current_dir()
            .ok()
            .and_then(|x| x.file_name().map(|x| x.to_string_lossy().to_lowercase()))
    }
}
