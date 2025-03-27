use std::fmt::{self, Display};
use std::fs;
use std::io::{Cursor, Read, Write};

use anyhow::{anyhow};
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use flate2::read::GzDecoder;
use http::Uri;
use http_body_util::Full;
use rover_http::ReqwestService;
use serde::{Deserialize, Serialize};
use tar::Archive;
use tower::{Service, ServiceExt};

use crate::command::template::queries::{get_templates_for_language, list_templates_for_language};
use crate::{RoverError, RoverResult};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// Filter templates by the available language
    #[arg(long = "language", value_enum)]
    pub language: Option<ProjectLanguage>,
}

impl TemplateOpt {
    pub fn get_or_prompt_language(&self) -> RoverResult<ProjectLanguage> {
        if let Some(language) = &self.language {
            Ok(language.clone())
        } else {
            let languages = <ProjectLanguage as ValueEnum>::value_variants();

            let selection = Select::new()
                .with_prompt("What language are you planning on using for the project?")
                .items(languages)
                .default(0)
                .interact_on_opt(&Term::stderr())?;

            match selection {
                Some(index) => Ok(languages[index].clone()),
                None => Err(RoverError::new(anyhow!("No language selected"))),
            }
        }
    }
}

#[derive(Debug)]
struct ArchiveEntry {
    path: String,
    contents: Vec<u8>,
}

#[derive(Debug)]
pub struct TemplateFetcher {
    artifacts: Vec<ArchiveEntry>,
}

impl TemplateFetcher {

    // This fn attempts to pull a template from github based on the URL
    // If successful, upstream code can then call list_files to display all the files that this template has
    //or write to actually write the files to fs
    pub async fn new(download_url: Uri, mut request_service: ReqwestService) -> RoverResult<Self> {
        eprintln!("Downloading from {}", &download_url);

        let req = http::Request::builder()
            .method(http::Method::GET)
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .header(reqwest::header::USER_AGENT, "rover-client")
            .uri(download_url)
            .body(Full::default())?;

        let service = request_service.ready().await?;
        let res = service.call(req).await?;
        let res = res.body().to_vec();

        let cursor = Cursor::new(res);
        let tar = GzDecoder::new(cursor);
        let mut archive: Archive<GzDecoder<Cursor<Vec<u8>>>> = Archive::new(tar);
        let mut root_name = None;

        let mut artifacts = Vec::new();

        // Tars store all artifacts in a "root" directory named after itself.
        // below we handle that by stripping the "root" prefix and storing a flatter
        // version of it in the artifacts vec
        for entry in archive.entries()? {
            let mut entry = entry?;
            let path = entry.path()?.to_owned(); 

            if path.starts_with("pax_global_header") {
                continue;
            }

            if root_name.is_none() {
                root_name = Some(
                    path.components()
                        .next()
                        .expect("Unexpected empty archive entry")
                        .as_os_str()
                        .to_string_lossy()
                        .to_string(),
                );
            }

            if let Some(root) = &root_name {
                if let Ok(stripped_path) = path.strip_prefix(root) {
                    let stripped_path = stripped_path.to_string_lossy().to_string();

                    let mut contents = Vec::new();
                    entry
                        .read_to_end(&mut contents)?;

                    artifacts.push(ArchiveEntry {
                        path: stripped_path, 
                        contents,
                    });
                }
            }
        }
        Ok(Self { artifacts })
    }


    pub fn write_template(&self, template_path: Utf8PathBuf) -> RoverResult<()> {
        fs::create_dir_all(&template_path)?;
        
        for artifact in &self.artifacts {
            let full_path = template_path.join(&artifact.path);

            if artifact.path.ends_with('/') || artifact.contents.is_empty() {
                fs::create_dir_all(&full_path)?;
            } else {
                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                let mut output_file = fs::File::create(&full_path)?;

                output_file
                    .write_all(&artifact.contents)?;
            }
        }

        Ok(())
    }

    // this will also have a list_files to print the list of files to write
    // in a list_files fn
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectLanguage {
    CSharp,
    Go,
    Java,
    Javascript,
    Kotlin,
    Python,
    Rust,
    Typescript,
    #[clap(skip)]
    Other(String),
}

impl Display for ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use ProjectLanguage::*;
        let readable = match self {
            CSharp => "C#",
            Go => "Go",
            Java => "Java",
            Javascript => "JavaScript",
            Kotlin => "Kotlin",
            Python => "Python",
            Rust => "Rust",
            Typescript => "TypeScript",
            Other(other) => other,
        };
        write!(f, "{}", readable)
    }
}

impl From<ProjectLanguage> for get_templates_for_language::Language {
    fn from(language: ProjectLanguage) -> get_templates_for_language::Language {
        match language {
            ProjectLanguage::CSharp => get_templates_for_language::Language::C_SHARP,
            ProjectLanguage::Go => get_templates_for_language::Language::GO,
            ProjectLanguage::Java => get_templates_for_language::Language::JAVA,
            ProjectLanguage::Javascript => get_templates_for_language::Language::JAVASCRIPT,
            ProjectLanguage::Kotlin => get_templates_for_language::Language::KOTLIN,
            ProjectLanguage::Python => get_templates_for_language::Language::PYTHON,
            ProjectLanguage::Rust => get_templates_for_language::Language::RUST,
            ProjectLanguage::Typescript => get_templates_for_language::Language::TYPESCRIPT,
            ProjectLanguage::Other(other) => get_templates_for_language::Language::Other(other),
        }
    }
}

impl From<ProjectLanguage> for list_templates_for_language::Language {
    fn from(language: ProjectLanguage) -> list_templates_for_language::Language {
        match language {
            ProjectLanguage::CSharp => list_templates_for_language::Language::C_SHARP,
            ProjectLanguage::Go => list_templates_for_language::Language::GO,
            ProjectLanguage::Java => list_templates_for_language::Language::JAVA,
            ProjectLanguage::Javascript => list_templates_for_language::Language::JAVASCRIPT,
            ProjectLanguage::Kotlin => list_templates_for_language::Language::KOTLIN,
            ProjectLanguage::Python => list_templates_for_language::Language::PYTHON,
            ProjectLanguage::Rust => list_templates_for_language::Language::RUST,
            ProjectLanguage::Typescript => list_templates_for_language::Language::TYPESCRIPT,
            ProjectLanguage::Other(other) => list_templates_for_language::Language::Other(other),
        }
    }
}

impl From<list_templates_for_language::Language> for ProjectLanguage {
    fn from(language: list_templates_for_language::Language) -> Self {
        match language {
            list_templates_for_language::Language::C_SHARP => ProjectLanguage::CSharp,
            list_templates_for_language::Language::GO => ProjectLanguage::Go,
            list_templates_for_language::Language::JAVA => ProjectLanguage::Java,
            list_templates_for_language::Language::JAVASCRIPT => ProjectLanguage::Javascript,
            list_templates_for_language::Language::KOTLIN => ProjectLanguage::Kotlin,
            list_templates_for_language::Language::PYTHON => ProjectLanguage::Python,
            list_templates_for_language::Language::RUST => ProjectLanguage::Rust,
            list_templates_for_language::Language::TYPESCRIPT => ProjectLanguage::Typescript,
            list_templates_for_language::Language::Other(other) => ProjectLanguage::Other(other),
        }
    }
}
