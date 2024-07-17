use std::fmt::{self, Display};
use std::io::Write;

use anyhow::{anyhow, Context};
use camino::Utf8PathBuf;
use clap::{Parser, ValueEnum};
use console::Term;
use dialoguer::Select;
use serde::{Deserialize, Serialize};
use url::Url;

use rover_std::Fs;

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

pub(crate) async fn extract_tarball(
    download_url: Url,
    template_path: &Utf8PathBuf,
    client: &reqwest::Client,
) -> RoverResult<()> {
    let download_dir = tempfile::Builder::new()
        .prefix("rover-template")
        .tempdir()?;
    let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
    let file_name = format!("{}.tar.gz", template_path);
    let tarball_path = download_dir_path.join(file_name);
    let mut f = std::fs::File::create(&tarball_path)?;
    eprintln!("Downloading from {}", &download_url);
    let response_bytes = client
        .get(download_url)
        .header(reqwest::header::USER_AGENT, "rover-client")
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;
    f.write_all(&response_bytes[..])?;
    f.sync_all()?;
    let f = std::fs::File::open(&tarball_path)?;
    let tar = flate2::read::GzDecoder::new(f);
    let mut archive = tar::Archive::new(tar);
    archive
        .unpack(template_path)
        .with_context(|| format!("could not unpack tarball to '{}'", &template_path))?;

    // The unpacked tar will be nested in another folder
    let extra_dir_name = Fs::get_dir_entries(template_path)?.find(|_| true);
    if let Some(Ok(extra_dir_name)) = extra_dir_name {
        // For this reason, we must copy the contents of the folder, then delete it
        Fs::copy_dir_all(extra_dir_name.path(), template_path)?;

        // Delete old unpacked zip
        Fs::remove_dir_all(extra_dir_name.path())?;
    }

    Ok(())
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
