use std::fs::read_dir;

use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use clap::{CommandFactory, ErrorKind as ClapErrorKind, Parser};
use dialoguer::Input;
use serde::Serialize;

use crate::cli::Rover;
use crate::options::TemplateOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};

use super::templates::GithubTemplates;

#[derive(Clone, Debug, Parser, Serialize)]
pub struct Use {
    #[clap(flatten)]
    options: TemplateOpt,

    /// The ID for the official template to use.
    /// Use `rover template list` to see available options.
    #[clap(short = 't', long = "template")]
    pub template: Option<String>,

    /// The relative or absolute path to create the template directory.
    ///
    /// If omitted, the template will be extracted to a child directory
    /// correlating to the template ID.
    path: Option<Utf8PathBuf>,
}

impl Use {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        // initialize the available templates
        let templates = GithubTemplates::new();

        // find the template to extract
        let template = if let Some(template_id) = &self.template {
            // if they specify an ID, get it
            templates.get(template_id)
        } else {
            // otherwise, ask them what language they want to use
            let project_language = self.options.get_or_prompt_language()?;
            let templates = templates.filter_language(project_language);

            // ask them to select a template from the remaining templates
            templates.selection_prompt()
        }?;

        // find the path to extract the template to
        let path = self.get_or_prompt_path()?;

        // download and extract a tarball from github
        template.extract_github_tarball(&path, &client_config.get_reqwest_client()?)?;

        Ok(RoverOutput::TemplateUseSuccess { template, path })
    }

    pub(crate) fn get_or_prompt_path(&self) -> RoverResult<Utf8PathBuf> {
        let path: Utf8PathBuf = if let Some(path) = &self.path {
            Ok::<Utf8PathBuf, RoverError>(path.clone())
        } else if atty::is(atty::Stream::Stderr) {
            let mut input = Input::new();
            input.with_prompt("What path would you like to extract the template to?");
            let path: Utf8PathBuf = input.interact_text()?;
            Ok(path)
        } else {
            let mut cmd = Rover::command();
            cmd.error(
                ClapErrorKind::MissingRequiredArgument,
                "<PATH> is required when not attached to a TTY",
            )
            .exit();
        }?;

        match read_dir(&path) {
            Ok(dir) => {
                if dir.count() > 1 {
                    let mut err = RoverError::new(anyhow!(
                        "Cannot use the template because the '{}' directory is not empty.",
                        &path
                    ));
                    err.set_suggestion(RoverErrorSuggestion::Adhoc(format!("Either rename or remove the existing '{}' directory, or re-run this command with a different `<PATH>` argument.", &path)));
                    Err(err)
                } else {
                    Ok(path)
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    std::fs::create_dir_all(&path)
                        .with_context(|| format!("Could not create the '{}' directory", &path))?;
                    Ok(path)
                } else {
                    Err(RoverError::new(e))
                }
            }
        }
    }
}
