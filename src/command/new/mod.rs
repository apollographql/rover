use crate::utils::client::StudioClientConfig;
use ansi_term::Colour::Cyan;
use ansi_term::Style;
use console::Term;
use dialoguer::Select;
use saucer::Utf8PathBuf;
use saucer::{clap, Parser};
use serde::Serialize;
use std::convert::TryFrom;
use std::fs::read_dir;
use std::io::Write;
use std::str::FromStr;

use crate::options::{GithubTemplate, ProjectLanguage, ProjectType, TemplateOpt};
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

#[derive(Debug, Clone, Serialize, Parser)]
pub struct New {
    #[clap(flatten)]
    options: TemplateOpt,

    /// The relative path to create the template directory.
    #[clap(name = "path")]
    #[serde(skip_serializing)]
    path: String,
}

impl New {
    pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
        // read_dir will always have the root element, or a count of 1
        //match this error for directory not created yet, create if it doesn't exist
        if read_dir(&self.path).unwrap().count() > 1 {
            return Err(RoverError::new(anyhow!(
        "You can only create projects in a blank folder. This is to prevent from accidentally overwriting any work."
      )));
        }

        let templates = self.get_templates()?;
        let template_to_clone: Option<GithubTemplate> = if self.options.template.is_some() {
            let template_id = self.options.template.clone().unwrap();
            let index = templates.iter().position(|t| t.id == template_id).unwrap();

            Some(templates[index].clone())
        } else {
            let project_type = ProjectType::from_str(
                self.options
                    .project_type
                    .clone()
                    .unwrap_or_else(|| self.prompt_project_type().unwrap().to_string())
                    .as_str(),
            )
            .unwrap();

            let selected_language = ProjectLanguage::from_str(
                self.options
                    .language
                    .clone()
                    .unwrap_or_else(|| self.prompt_language().unwrap().to_string())
                    .as_str(),
            )
            .unwrap();

            let available_templates: Vec<GithubTemplate> = templates
                .into_iter()
                .filter(|template| {
                    project_type.eq(&template.project_type)
                        && template.language == selected_language
                })
                .collect();

            Some(self.template_prompt(&available_templates)?)
        };

        if template_to_clone.is_none() {
            return Err(RoverError::new(anyhow!(
                "An invalid template id was provided"
            )));
        }

        self.extract_github_tarball(
            template_to_clone.unwrap(),
            &self.path,
            &client_config.get_reqwest_client()?,
        )?;
        eprintln!(
            "{}:\n\t{}",
            Style::new()
                .bold()
                .paint("To learn more about GraphQL, head over to our tutorials"),
            Cyan.bold().paint("https://apollographql.com/tutorials")
        );
        Ok(RoverOutput::EmptySuccess)
    }

    pub fn prompt_language(&self) -> Result<ProjectLanguage> {
        let items = vec![
            ProjectLanguage::Java,
            ProjectLanguage::Javascript,
            ProjectLanguage::Python,
            ProjectLanguage::Rust,
            ProjectLanguage::Typescript,
        ];
        let selection = Select::new()
            .with_prompt("What language are you planning on using for the project?")
            .items(&items)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(items[index].clone()),
            None => Err(RoverError::new(anyhow!("No language selected"))),
        }
    }

    pub fn prompt_project_type(&self) -> Result<ProjectType> {
        let items = vec![ProjectType::Subgraph, ProjectType::Client];
        let selection = Select::new()
            .with_prompt("What GraphQL project are you planning on building?")
            .items(&items)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => match index {
                0 => Ok(ProjectType::Subgraph),
                1 => Ok(ProjectType::Client),
                _ => Err(RoverError::new(anyhow!("No project type selected"))),
            },
            None => Ok(ProjectType::All),
        }
    }

    pub fn template_prompt(&self, templates: &[GithubTemplate]) -> Result<GithubTemplate> {
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
    pub fn extract_github_tarball(
        &self,
        template: GithubTemplate,
        template_path: &str,
        client: &reqwest::blocking::Client,
    ) -> Result<()> {
        let download_dir = tempdir::TempDir::new(&template.id)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", template.id));
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
            Utf8PathBuf::try_from(template_folder_path.join(format!("{}-main", &template.id)))?,
            Utf8PathBuf::try_from(template_folder_path.to_path_buf())?,
            "",
        )?;
        //Delete old unpacked zip
        saucer::Fs::remove_dir_all(
            Utf8PathBuf::try_from(template_folder_path.join(format!("{}-main", &template.id)))?,
            "",
        )?;

        Ok(())
    }
    pub fn get_templates(&self) -> Result<Vec<GithubTemplate>> {
        let project_type = match self
            .options
            .project_type
            .clone()
            .unwrap_or_default()
            .as_str()
        {
            "1" => ProjectType::Client,
            "2" => ProjectType::Subgraph,
            _ => ProjectType::All,
        };

        let has_language = self.options.language.is_some();
        // TODO: To be moved to Orbit until "features" is designed out
        let templates = vec![GithubTemplate {
            id: String::from("subgraph-template-javascript-apollo-server-boilerplate"),
            git_url: String::from(
              "https://github.com/apollographql/subgraph-template-javascript-apollo-server-boilerplate",
            ),
            display: String::from("Boilerplate using Apollo Server"),
            language: ProjectLanguage::Javascript,
            project_type: ProjectType::Subgraph,
          },GithubTemplate {
            id: String::from("subgraph-template-javascript-apollo-server-mocked"),
            git_url: String::from(""),
            display: String::from(
                "Simple mocked SDL-based schema using Apollo Server Boilerplate template",
            ),
            language: ProjectLanguage::Javascript,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("subgraph-template-java-springboot-boilerplate"),
            git_url: String::from(""),
            display: String::from("(TBD) Springboot using federation-jvm"),
            language: ProjectLanguage::Java,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("subgraph-template-strawberry-fastapi"),
            git_url: String::from(
                "https://github.com/strawberry-graphql/subgraph-template-strawberry-fastapi",
            ),
            display: String::from("Boilerplate using Strawberry with FastAPI"),
            language: ProjectLanguage::Python,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("subgraph-template-python-ariadne-boilerplate"),
            git_url: String::from(""),
            display: String::from("(TBD) Boilerplate using Ariadne"),
            language: ProjectLanguage::Python,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("subgraph-template-rust-async-graphql-boilerplate"),
            git_url: String::from(
                "https://github.com/apollographql/subgraph-template-rust-async-graphql-boilerplate",
            ),
            display: String::from("Boilerplate using async-graphql"),
            language: ProjectLanguage::Rust,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("subgraph-template-typescript-apollo-server-boilerplate"),
            git_url: String::from(""),
            display: String::from("Boilerplate using Apollo Server"),
            language: ProjectLanguage::Typescript,
            project_type: ProjectType::Subgraph,
        },GithubTemplate {
            id: String::from("apollo-client-javascript"),
            git_url: String::from(""),
            display: String::from("Javascript: Apollo Client"),
            language: ProjectLanguage::Javascript,
            project_type: ProjectType::Client,
        },GithubTemplate {
            id: String::from("apollo-client-typescript"),
            git_url: String::from(""),
            display: String::from("Typescript: Apollo Client"),
            language: ProjectLanguage::Javascript,
            project_type: ProjectType::Client,
        }];

        let temp_iter = templates.into_iter();
        if project_type != ProjectType::All && has_language {
            Ok(temp_iter
                .filter(|template| {
                    project_type.eq(&template.project_type)
                        && template.language == self.options.language.clone().unwrap()
                })
                .collect())
        } else if project_type != ProjectType::All {
            Ok(temp_iter
                .filter(|template| project_type.eq(&template.project_type))
                .collect())
        } else if has_language {
            Ok(temp_iter
                .filter(|template| template.language == self.options.language.clone().unwrap())
                .collect())
        } else {
            Ok(temp_iter.collect())
        }
    }
}
