use std::{
    fs::read_dir,
    io::{self, IsTerminal},
};

use anyhow::{Context, anyhow};
use camino::Utf8PathBuf;
use clap::{CommandFactory, Parser, error::ErrorKind as ClapErrorKind};
use dialoguer::Input;
use serde::Serialize;

use super::templates::{get_template, get_templates_for_language, selection_prompt};
use crate::{
    RoverError, RoverErrorSuggestion, RoverOutput, RoverResult, cli::Rover, options::TemplateOpt,
    utils::template::download_template,
};

#[derive(Clone, Debug, Parser, Serialize)]
pub struct Use {
    #[clap(flatten)]
    options: TemplateOpt,

    /// The ID for the official template to use.
    /// Use `rover template list` to see available options.
    #[arg(short = 't', long = "template")]
    pub template: Option<String>,

    /// The relative or absolute path to create the template directory.
    ///
    /// If omitted, the template will be extracted to a child directory
    /// correlating to the template ID.
    path: Option<Utf8PathBuf>,
}

impl Use {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        // find the template to extract
        let (template_id, download_url) = if let Some(template_id) = &self.template {
            // if they specify an ID, get it
            let result = get_template(template_id).await?;
            if let Some(result) = result {
                (template_id.clone(), result.download_url)
            } else {
                let mut err = RoverError::new(anyhow!("No template found with id {}", template_id));
                err.set_suggestion(RoverErrorSuggestion::Adhoc(
                    "Run `rover template list` to see all available templates.".to_string(),
                ));
                return Err(err);
            }
        } else {
            // otherwise, ask them what language they want to use
            let project_language = self.options.get_or_prompt_language()?;
            let templates = get_templates_for_language(project_language).await?;
            let template = selection_prompt(templates)?;
            (template.id, template.download_url)
        };

        // find the path to extract the template to
        let path = self.get_or_prompt_path()?;

        // download and extract a tarball from github
        download_template(download_url, &path).await?;
        Ok(RoverOutput::TemplateUseSuccess { template_id, path })
    }

    pub(crate) fn get_or_prompt_path(&self) -> RoverResult<Utf8PathBuf> {
        let path: Utf8PathBuf = if let Some(path) = &self.path {
            Ok::<Utf8PathBuf, RoverError>(path.clone())
        } else if io::stderr().is_terminal() {
            let input =
                Input::new().with_prompt("What path would you like to extract the template to?");
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
