use std::convert::TryFrom;
use std::fs::read_dir;
use std::io::Write;

use console::Term;
use dialoguer::Select;
use saucer::{clap, Parser};
use saucer::{Utf8PathBuf, ValueEnum};
use serde::Serialize;

use crate::options::{GithubTemplate, ProjectLanguage, ProjectType, TemplateOpt};
use crate::utils::client::StudioClientConfig;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

#[derive(Debug, Clone, Parser, Serialize)]
pub struct Template {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Clone, Debug, Parser, Serialize)]
enum Command {
    /// Use a template to generate code
    Use(UseTemplate),
    /// List available templates that can be used
    List(List),
}

impl Template {
    pub(crate) fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        match &self.command {
            Command::Use(use_template) => use_template.run(client_config),
            Command::List(list) => list.run(),
        }
    }
}

#[derive(Clone, Debug, Parser, Serialize)]
struct List {
    #[clap(flatten)]
    options: TemplateOpt,
}

impl List {
    pub fn run(&self) -> Result<RoverOutput> {
        let templates = get_templates(self.options.project_type, self.options.language);
        Ok(RoverOutput::TemplateList(templates))
    }
}

/// Display the optionally filtered template list
///
/// TODO: Fetch templates from an API instead of embedding them
fn get_templates(
    project_type: Option<ProjectType>,
    project_language: Option<ProjectLanguage>,
) -> Vec<GithubTemplate> {
    TEMPLATES
        .into_iter()
        .filter(|template| {
            project_type
                .map(|project_type| project_type == template.project_type)
                .unwrap_or(true)
        })
        .filter(|template| {
            project_language
                .map(|project_language| project_language == template.language)
                .unwrap_or(true)
        })
        .collect()
}

const TEMPLATES: [GithubTemplate; 3] = [
    GithubTemplate {
        id: "subgraph-javascript-apollo-server",
        git_url: "https://github.com/apollographql/subgraph-template-javascript-apollo-server-boilerplate",
        display: "Apollo Server",
        language: ProjectLanguage::Javascript,
        project_type: ProjectType::Subgraph,
    },
    GithubTemplate {
        id: "subgraph-python-strawberry-fastapi",
        git_url: "https://github.com/strawberry-graphql/subgraph-template-strawberry-fastapi",
        display: "Strawberry with FastAPI",
        language: ProjectLanguage::Python,
        project_type: ProjectType::Subgraph,
    },
    GithubTemplate {
        id: "subgraph-rust-async-graphql",
        git_url: "https://github.com/apollographql/subgraph-template-rust-async-graphql-boilerplate",
        display: "async-graphql with Axum",
        language: ProjectLanguage::Rust,
        project_type: ProjectType::Subgraph,
    }
];

#[derive(Clone, Debug, Parser, Serialize)]
struct UseTemplate {
    #[clap(flatten)]
    options: TemplateOpt,

    /// The ID for the official template to use.
    /// Use `rover template list` to see available options.
    #[clap(short = 't', long = "template")]
    pub template: Option<String>,

    /// The relative or absolute path to create the template directory.
    path: String,
}

impl UseTemplate {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        let template_to_clone: GithubTemplate = if let Some(template_id) = &self.template {
            Self::get_template_by_id(template_id)?
        } else {
            let project_type = self
                .options
                .project_type
                .map(Ok)
                .unwrap_or_else(|| self.prompt_project_type())?;

            let project_language = self
                .options
                .language
                .map(Ok)
                .unwrap_or_else(|| self.prompt_language())?;

            let templates = get_templates(Some(project_type), Some(project_language));

            self.template_prompt(&templates)?
        };

        match read_dir(&self.path) {
            Ok(dir) => {
                if dir.count() > 1 {
                    return Err(
                        RoverError::new(anyhow!(
                            "You can only create projects in a blank folder. This is to prevent from accidentally overwriting any work."
                        ))
                    );
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    std::fs::create_dir_all(&self.path)?;
                } else {
                    return Err(RoverError::new(anyhow!(e)));
                }
            }
        }

        Self::extract_github_tarball(
            &template_to_clone,
            &self.path,
            &client_config.get_reqwest_client()?,
        )?;
        Ok(RoverOutput::TemplateUseSuccess {
            template: template_to_clone,
            path: self.path.clone(),
        })
    }

    pub fn prompt_language(&self) -> Result<ProjectLanguage> {
        let languages = <ProjectLanguage as ValueEnum>::value_variants();
        let selection = Select::new()
            .with_prompt("What language are you planning on using for the project?")
            .items(languages)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(languages[index]),
            None => Err(RoverError::new(anyhow!("No language selected"))),
        }
    }

    pub fn prompt_project_type(&self) -> Result<ProjectType> {
        let types = <ProjectType as ValueEnum>::value_variants();
        let selection = Select::new()
            .with_prompt("What GraphQL project are you planning on building?")
            .items(types)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(types[index]),
            None => Err(RoverError::new(anyhow!("No project type selected"))),
        }
    }

    pub(crate) fn template_prompt(&self, templates: &[GithubTemplate]) -> Result<GithubTemplate> {
        let selection = Select::new()
            .with_prompt("Which template would you like to use?")
            .items(templates)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(templates[index].clone()),
            None => Err(RoverError::new(anyhow!("No template selected"))),
        }
    }
    pub(crate) fn extract_github_tarball(
        template: &GithubTemplate,
        template_path: &str,
        client: &reqwest::blocking::Client,
    ) -> Result<()> {
        let download_dir = tempdir::TempDir::new(template.id)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let git_repo_slug = template.repo_slug()?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", git_repo_slug));
        let tarball_url = format!("{}/archive/refs/heads/main.tar.gz", template.git_url);
        let mut f = std::fs::File::create(&tarball_path)?;
        eprintln!("Downloading {}", template.git_url);
        eprintln!("\tfrom {}", tarball_url);
        let response_bytes = client
            .get(tarball_url)
            .header(reqwest::header::USER_AGENT, "rover-client")
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .send()?
            .error_for_status()?
            .bytes()?;
        f.write_all(&response_bytes[..])?;
        f.sync_all()?;
        let f = std::fs::File::open(&tarball_path)?;
        let tar = flate2::read::GzDecoder::new(f);
        let mut archive = tar::Archive::new(tar);
        archive.unpack(&template_path)?;

        // The unpacked tar will be in the folder{git_repo_id}-{branch}
        // For this reason, we must copy the contents of the folder, then delete it
        let template_folder_path = std::path::Path::new(&template_path);
        saucer::Fs::copy_dir_all(
            Utf8PathBuf::try_from(template_folder_path.join(format!("{}-main", git_repo_slug)))?,
            Utf8PathBuf::try_from(template_folder_path.to_path_buf())?,
            "",
        )?;
        //Delete old unpacked zip
        saucer::Fs::remove_dir_all(
            Utf8PathBuf::try_from(template_folder_path.join(format!("{}-main", git_repo_slug)))?,
            "",
        )?;

        Ok(())
    }

    pub(crate) fn get_template_by_id(id: &str) -> Result<GithubTemplate> {
        TEMPLATES
            .into_iter()
            .find(|template| template.id == id)
            .ok_or_else(|| RoverError::new(anyhow!("No template found with id {}", id)))
    }
}
