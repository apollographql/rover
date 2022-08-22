mod list;
mod template;

use crate::utils::client::StudioClientConfig;
use ansi_term::Colour::Cyan;
use ansi_term::Style;
use camino::Utf8PathBuf;
use console::Term;
use saucer::{clap, Parser};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt;
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
    let term = console::Term::stderr();
    let client = client_config.get_reqwest_client();
    if self.options.template.is_some() {
      // User provided template id
      self.extract_github_tarball(
        &self.options.template.clone().unwrap().as_str(),
        &self.path,
        &client,
      );
    } else {
      let mut template_id: String;
      let templates = self.get_templates();
      //If no project type provided, we need to ask which project type user intends to use
      // NOTE: Language is just an additional filter (that has already been applied) on both project types
      // 0=All
      // let mut project_type: ProjectType = ProjectType::ALL;
      if self.options.project_type.is_some() {
        let selected_project_type =
          ProjectType::from_str(&self.options.project_type.clone().unwrap())?;
        template_id = match selected_project_type {
          ProjectType::CLIENT => self.template_prompt(
            &term,
            &templates?
              .into_iter()
              .filter(|template| ProjectType::CLIENT.eq(&template.project_type))
              .collect(),
          )?,
          ProjectType::SUBGRAPH => self.template_prompt(
            &term,
            &templates?
              .into_iter()
              .filter(|template| ProjectType::SUBGRAPH.eq(&template.project_type))
              .collect(),
          )?,
          _ => self.template_prompt(&term, &templates?)?,
        };
      } else {
        let project_type = self.prompt_project_type(&term)?;
        template_id = match project_type {
          ProjectType::CLIENT => self.template_prompt(
            &term,
            &templates?
              .into_iter()
              .filter(|template| ProjectType::CLIENT.eq(&template.project_type))
              .collect(),
          )?,
          ProjectType::SUBGRAPH => self.template_prompt(
            &term,
            &templates?
              .into_iter()
              .filter(|template| ProjectType::SUBGRAPH.eq(&template.project_type))
              .collect(),
          )?,
          _ => self.template_prompt(&term, &templates?)?,
        };
      }

      self.extract_github_tarball(template_id.as_str(), &self.path, &client);
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

  pub fn prompt_project_type(&self, term: &Term) -> Result<ProjectType> {
    // let term = console::Term::stderr();
    eprintln!("What type of project would you like to start?\n");
    eprintln!("\t1 - Subgraph");
    eprintln!("\t2 - Client");
    eprint!("\nChoice: ");
    let project_type = term.read_line()?;
    if project_type == String::from("1") {
      return Ok(ProjectType::SUBGRAPH);
    } else if project_type == String::from("2") {
      return Ok(ProjectType::CLIENT);
    } else {
      return Err(RoverError::from(anyhow!("Invalid Project Type")));
    }
  }
  pub fn template_prompt(&self, term: &Term, templates: &Vec<GithubTemplate>) -> Result<String> {
    eprintln!("\nWhich template would you like to use?");
    let mut counter = 0;
    for template in templates {
      counter = counter + 1;
      eprintln!("\t{} - {}", counter, template.display);
    }
    term.write_str("Template: ")?;
    let entered_template = term.read_line()?;
    let entered_position: i32 = entered_template.to_string().parse::<i32>().unwrap() - 1;
    let my_int = usize::try_from(entered_position).ok().unwrap();
    if my_int <= counter {
      return Ok(String::from(&templates[my_int].id));
    } else {
      return Err(RoverError::new(anyhow!("Invalid template selected")));
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
    eprintln!("Downloading {} from {}", id, tarball_url);
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
    // let template_folder_path = std::path::Path::new(&template_path);
    // let test = template_folder_path.join(id);
    // let copy_results = std::fs::File.(&test, &template_path);
    // std::fs::remove_dir_all(&test);
    // std::fs::copy(from: P, to: Q)
    //Delete old zip
    // fs::remove_file(&template_path)?;
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
    templates.push(GithubTemplate {
      id: String::from("java-federation-jvm-minimal"),
      display: String::from("Java: Federation JVM Minimal"),
      language: String::from("java"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("javascript-apollo-server-minimal"),
      display: String::from("Javascript: Apollo Server Minimal"),
      language: String::from("javascript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("javascript-apollo-server-docker"),
      display: String::from("Javascript: Apollo Server Docker"),
      language: String::from("javascript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("kotlin-federation-jvm-minimal"),
      display: String::from("Kotlin: Federation JVM Minimal"),
      language: String::from("java"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("python-strawberry-minimal"),
      display: String::from("Python: Strawberry Minimal"),
      language: String::from("python"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("rust-async-graphql-minimal"),
      display: String::from("Rust: Async-graphql Minimal"),
      language: String::from("rust"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("typescript-apollo-server-minimal"),
      display: String::from("Typescript: Apollo Server Minimal"),
      language: String::from("typescript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("typescript-apollo-server-docker"),
      display: String::from("Typescript: Apollo Server Docker"),
      language: String::from("typescript"),
      project_type: ProjectType::SUBGRAPH.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("subgraph-template-typescript-apollo-server"),
      display: String::from("Typescript: Apollo Server with REST DataSource"),
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
      id: String::from("apollo-kotlin"),
      display: String::from("Kotlin: Apollo Android"),
      language: String::from("kotlin"),
      project_type: ProjectType::CLIENT.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("apollo-client-typescript"),
      display: String::from("Typescript: Apollo Client"),
      language: String::from("typescript"),
      project_type: ProjectType::CLIENT.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("apollo-ios"),
      display: String::from("Obj-C: Apollo iOS"),
      language: String::from("obj-c"),
      project_type: ProjectType::CLIENT.to_string(),
    });
    templates.push(GithubTemplate {
      id: String::from("apollo-ios"),
      display: String::from("Swift: Apollo iOS"),
      language: String::from("swift"),
      project_type: ProjectType::CLIENT.to_string(),
    });
    if project_type != ProjectType::ALL && has_language {
      return Ok(
        templates
          .into_iter()
          .filter(|template| {
            project_type.eq(&template.project_type)
              && template.language == self.options.language.clone().unwrap()
          })
          .collect(),
      );
    } else if project_type != ProjectType::ALL {
      return Ok(
        templates
          .into_iter()
          .filter(|template| project_type.eq(&template.project_type))
          .collect(),
      );
    } else if has_language {
      return Ok(
        templates
          .into_iter()
          .filter(|template| template.language == self.options.language.clone().unwrap())
          .collect(),
      );
    } else {
      return Ok(templates);
    }
  }
}
