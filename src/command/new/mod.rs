use crate::utils::client::StudioClientConfig;
use ansi_term::Colour::Cyan;
use ansi_term::Style;
use camino::Utf8PathBuf;
use console::Term;
use dialoguer::Select;
use saucer::Utf8PathBuf as PathBuf;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
use std::fs::read_dir;
use std::io::Write;
use std::str::FromStr;

use crate::options::TemplateOpt;
use crate::{anyhow, command::RoverOutput, error::RoverError, Result};

#[derive(Debug, Clone, Serialize, Parser)]
pub struct New {
  #[clap(flatten)]
  options: TemplateOpt,

  /// Path to create template at
  #[clap(name = "path")]
  #[serde(skip_serializing)]
  path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Parser)]
pub struct GithubTemplate {
  pub id: String,
  pub display: String,
  pub language: String,
  pub project_type: String,
}

impl fmt::Display for GithubTemplate {
  // This trait requires `fmt` with this exact signature.
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    // Write strictly the first element into the supplied output
    // stream: `f`. Returns `fmt::Result` which indicates whether the
    // operation succeeded or failed. Note that `write!` uses syntax which
    // is very similar to `println!`.
    write!(f, "{}", self.display)
  }
}

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub enum ProjectType {
  ALL,
  SUBGRAPH,
  CLIENT,
}

impl PartialEq<String> for ProjectType {
  fn eq(&self, other: &String) -> bool {
    match &self {
      ProjectType::ALL => other.to_lowercase() == "all",
      ProjectType::SUBGRAPH => other.to_lowercase() == "subgraph",
      ProjectType::CLIENT => other.to_lowercase() == "client",
    }
  }
}
impl PartialEq for ProjectType {
  fn eq(&self, other: &ProjectType) -> bool {
    match &self {
      ProjectType::ALL => other.eq(&String::from("all")),
      ProjectType::SUBGRAPH => other.eq(&String::from("subgraph")),
      ProjectType::CLIENT => other.eq(&String::from("client")),
    }
  }
}
impl fmt::Display for ProjectType {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    match self {
      ProjectType::ALL => write!(f, "all"),
      ProjectType::SUBGRAPH => write!(f, "subgraph"),
      ProjectType::CLIENT => write!(f, "client"),
    }
  }
}

impl FromStr for ProjectType {
  type Err = saucer::Error;

  fn from_str(input: &str) -> std::result::Result<Self, Self::Err> {
    match input {
      "all" => Ok(ProjectType::ALL),
      "subgraph" => Ok(ProjectType::SUBGRAPH),
      "client" => Ok(ProjectType::CLIENT),
      _ => Err(anyhow!("Invalid Project Type")),
    }
  }
}

impl New {
  pub fn run(&self, client_config: StudioClientConfig) -> Result<RoverOutput> {
    // read_dir will always have the root element, or a count of 1
    if read_dir(&self.path).unwrap().count() > 1 {
      return Err(RoverError::new(anyhow!(
        "You can only create projects in a blank folder. This is to prevent from accidentally overwriting any work."
      )));
    }

    let client = client_config.get_reqwest_client()?;
    if self.options.template.is_some() {
      // User provided template id
      self.extract_github_tarball(
        &self.options.template.clone().unwrap().as_str(),
        &self.path,
        &client,
      )?;
    } else {
      let template_id: String;
      let templates = self.get_templates();
      let should_prompt_project_type = self.options.project_type.is_none();
      let should_prompt_language = self.options.language.is_none();

      let project_type: ProjectType;
      if should_prompt_project_type {
        project_type = self.prompt_project_type()?;
      } else {
        project_type = ProjectType::from_str(&self.options.project_type.clone().unwrap())?;
      }
      let selected_language: String;
      if should_prompt_language {
        selected_language = self.prompt_language()?;
      } else {
        selected_language = self.options.language.clone().unwrap();
      }

      let available_templates = &templates?
        .into_iter()
        .filter(|template| {
          project_type.eq(&template.project_type)
            && template.language.to_lowercase() == selected_language.to_lowercase()
        })
        .collect();

      template_id = self.template_prompt(available_templates)?;

      self.extract_github_tarball(template_id.as_str(), &self.path, &client)?;
    }

    eprintln!(
      "{}:\n\t{}",
      Style::new()
        .bold()
        .paint("To learn more GraphQL, head over to our tutorials"),
      Cyan.bold().paint("https://apollographql.com/tutorials")
    );
    return Ok(RoverOutput::EmptySuccess);
  }

  pub fn prompt_language(&self) -> Result<String> {
    let items = vec!["Java", "JavaScript", "Python", "Rust", "Typescript"];
    let selection = Select::new()
      .with_prompt("What language are you planning on using for the project?")
      .items(&items)
      .default(0)
      .interact_on_opt(&Term::stderr())?;

    match selection {
      Some(index) => Ok(String::from(items[index])),
      None => Err(RoverError::new(anyhow!("No language selected"))),
    }
  }

  pub fn prompt_project_type(&self) -> Result<ProjectType> {
    let items = vec!["Subgraph", "Client"];
    let selection = Select::new()
      .with_prompt("What GraphQL project are you planning on building?")
      .items(&items)
      .default(0)
      .interact_on_opt(&Term::stderr())?;

    match selection {
      Some(index) => match index {
        0 => Ok(ProjectType::SUBGRAPH),
        1 => Ok(ProjectType::CLIENT),
        _ => Err(RoverError::new(anyhow!("No project type selected"))),
      },
      None => Ok(ProjectType::ALL),
    }
  }

  pub fn template_prompt(&self, templates: &Vec<GithubTemplate>) -> Result<String> {
    let selection = Select::new()
      .with_prompt("Which template would you like to use?")
      .items(&templates)
      .default(0)
      .interact_on_opt(&Term::stderr())?;

    match selection {
      Some(index) => Ok(String::from(templates[index].id.clone())),
      None => Err(RoverError::new(anyhow!("No template selected"))),
    }
  }
  pub fn extract_github_tarball(
    &self,
    id: &str,
    template_path: &str,
    client: &reqwest::blocking::Client,
  ) -> Result<()> {
    let download_dir = tempdir::TempDir::new(id)?;
    let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
    let tarball_path = download_dir_path.join(format!("{}.tar.gz", id));
    let tarball_url = format!(
      "https://github.com/apollographql/{}/archive/refs/heads/main.tar.gz",
      id
    );
    let mut f = std::fs::File::create(&tarball_path)?;
    eprintln!("Downloading {}", id);
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
      PathBuf::try_from(template_folder_path.join(format!("{}-main", id)).clone())?,
      PathBuf::try_from(template_folder_path.to_path_buf())?,
      id,
    )?;
    //Delete old unpacked zip
    saucer::Fs::remove_dir_all(
      PathBuf::try_from(template_folder_path.join(format!("{}-main", id)).clone())?,
      id,
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
      "1" => ProjectType::CLIENT,
      "2" => ProjectType::SUBGRAPH,
      _ => ProjectType::ALL,
    };

    let has_language = self.options.language.is_some();
    let mut templates = Vec::new();
    //Subgraph Project Templates

    // TODO: To be moved to Orbit until "features" is designed out
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-javascript-apollo-server-boilerplate"),
      display: String::from("Boilerplate using Apollo Server"),
      language: String::from("javascript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-javascript-apollo-server-mocked"),
      display: String::from(
        "Simple mocked SDL-based schema using Apollo Server Boilerplate template",
      ),
      language: String::from("javascript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-java-springboot-boilerplate"),
      display: String::from("(TBD) Springboot using federation-jvm"),
      language: String::from("java"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-python-strawberry-boilerplate"),
      display: String::from("(TBD) Boilerplate using Strawberry"),
      language: String::from("python"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-python-ariadne-boilerplate"),
      display: String::from("(TBD) Boilerplate using Ariadne"),
      language: String::from("python"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-rust-async-graphql-boilerplate"),
      display: String::from("Boilerplate using async-graphql"),
      language: String::from("rust"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-typescript-apollo-server-boilerplate"),
      display: String::from("Boilerplate using Apollo Server"),
      language: String::from("typescript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    //Client Project Templates
    templates.push(GithubTemplate {
      id: String::from("apollo-client-javascript"),
      display: String::from("Javascript: Apollo Client"),
      language: String::from("javascript"),
      project_type: ProjectType::CLIENT.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("apollo-client-typescript"),
      display: String::from("Typescript: Apollo Client"),
      language: String::from("typescript"),
      project_type: ProjectType::CLIENT.to_string(),
    });

    let temp_iter = templates.into_iter();
    if project_type != ProjectType::ALL && has_language {
      return Ok(
        temp_iter
          .filter(|template| {
            project_type.eq(&template.project_type)
              && template.language == self.options.language.clone().unwrap()
          })
          .collect(),
      );
    } else if project_type != ProjectType::ALL {
      return Ok(
        temp_iter
          .filter(|template| project_type.eq(&template.project_type))
          .collect(),
      );
    } else if has_language {
      return Ok(
        temp_iter
          .filter(|template| template.language == self.options.language.clone().unwrap())
          .collect(),
      );
    } else {
      return Ok(temp_iter.collect());
    }
  }
}
