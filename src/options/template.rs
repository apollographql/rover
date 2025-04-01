use std::fmt::{self, Display};

use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use http::Uri;
use http_body_util::Full;
use rover_http::ReqwestService;
use rover_std::Fs;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
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
pub struct TemplateFetcher {
    request_service: ReqwestService,
}

pub struct TemplateProject {
    contents: Vec<u8>,
}

impl TemplateFetcher {
    pub fn new(request_service: ReqwestService) -> Self {
        Self { request_service }
    }

    pub async fn call(&mut self, download_url: Uri) -> RoverResult<TemplateProject> {
        println!("Downloading from {}", &download_url);
        println!();
        let req = http::Request::builder()
            .method(http::Method::GET)
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .header(reqwest::header::USER_AGENT, "rover-client")
            .uri(download_url)
            .body(Full::default())?;

        let service = self.request_service.ready().await?;
        let res = service.call(req).await?;
        let res = res.body().to_vec();

        if res.is_empty() {
            return Err(RoverError::new(anyhow!("No template found")));
        }

        Ok(TemplateProject { contents: res })
    }
}

impl TemplateProject {
    pub fn write_template(&self, template_path: &Utf8PathBuf) -> RoverResult<()> {
        let cursor = Cursor::new(&self.contents);
        let tar = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(tar);

        archive.unpack(template_path)?;

        let extra_dir_name = Fs::get_dir_entries(template_path)?.find(|_| true);
        if let Some(Ok(extra_dir_name)) = extra_dir_name {
            Fs::copy_dir_all(extra_dir_name.path(), template_path)?;
            Fs::remove_dir_all(extra_dir_name.path())?;
        }

        Ok(())
    }

    #[cfg_attr(not(feature = "init"), allow(dead_code))]
    pub fn list_files(&self) -> RoverResult<Vec<Utf8PathBuf>> {
        let cursor = Cursor::new(&self.contents);
        let tar = flate2::read::GzDecoder::new(cursor);
        let mut archive = tar::Archive::new(tar);

        let mut files = Vec::new();
        for entry in archive.entries()? {
            let entry = entry?;
            let path = entry.path()?;
            let mut components = path.components();
            components.next();
            let path = components.as_path();

            if !(path.starts_with("pax_global_header") || path.starts_with("..")) {
                if let Ok(path_buf) = Utf8PathBuf::from_path_buf(path.to_path_buf()) {
                    //ignore top level directories
                    if !entry.header().entry_type().is_dir() {
                        files.push(path_buf);
                    }
                }
            }
        }

        Ok(files)
    }
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
