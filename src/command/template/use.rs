use std::fs::read_dir;
use std::str::FromStr;

use saucer::{clap, Context, Parser, Utf8PathBuf};
use serde::Serialize;

use crate::options::{GithubTemplate, TemplateOpt};
use crate::utils::client::StudioClientConfig;
use crate::Suggestion;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

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
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
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
        let path = self.get_path(&template)?;

        // download and extract a tarball from github
        template.extract_github_tarball(&path, &client_config.get_reqwest_client()?)?;

        Ok(RoverOutput::TemplateUseSuccess { template, path })
    }

    pub(crate) fn get_path(&self, template: &GithubTemplate) -> Result<Utf8PathBuf> {
        let path = if let Some(path) = &self.path {
            path.clone()
        } else {
            Utf8PathBuf::from_str(template.id)?
        };

        match read_dir(&path) {
            Ok(dir) => {
                if dir.count() > 1 {
                    let mut err = RoverError::new(anyhow!(
                        "Cannot use the template because the '{}' directory is not empty.",
                        &path
                    ));
                    err.set_suggestion(Suggestion::Adhoc(format!("Either rename or remove the existing '{}' directory, or re-run this command with a different `<PATH>` argument.", &path)));
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
