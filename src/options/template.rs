use std::fmt::{self, Display};
use std::io::Write;

use console::Term;
use dialoguer::Select;
use saucer::{anyhow, clap, Parser, Utf8PathBuf, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::{error::RoverError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct TemplateOpt {
    /// Filter templates by the available language
    #[clap(long = "language", value_enum)]
    pub language: Option<ProjectLanguage>,
}

impl TemplateOpt {
    pub fn get_or_prompt_language(&self) -> Result<ProjectLanguage> {
        if let Some(language) = self.language {
            Ok(language)
        } else {
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
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GithubTemplate {
    pub id: &'static str,
    pub git_url: &'static str,
    pub display: &'static str,
    pub language: ProjectLanguage,
}

impl GithubTemplate {
    pub(crate) fn repo_slug(&self) -> Result<&'static str> {
        self.git_url
            .split('/')
            .last()
            .ok_or_else(|| anyhow!("Could not determine tarball path.").into())
    }

    pub(crate) fn extract_github_tarball(
        &self,
        template_path: &Utf8PathBuf,
        client: &reqwest::blocking::Client,
    ) -> Result<()> {
        let download_dir = tempdir::TempDir::new(self.id)?;
        let download_dir_path = Utf8PathBuf::try_from(download_dir.into_path())?;
        let git_repo_slug = self.repo_slug()?;
        let tarball_path = download_dir_path.join(format!("{}.tar.gz", git_repo_slug));
        let tarball_url = format!("{}/archive/refs/heads/main.tar.gz", &self.git_url);
        let mut f = std::fs::File::create(&tarball_path)?;
        eprintln!("Downloading {}", &self.git_url);
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
        // Delete old unpacked zip
        saucer::Fs::remove_dir_all(
            Utf8PathBuf::try_from(template_folder_path.join(format!("{}-main", git_repo_slug)))?,
            "",
        )?;

        Ok(())
    }
}

impl Display for GithubTemplate {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.display)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, clap::ValueEnum)]
pub enum ProjectLanguage {
    Java,
    Javascript,
    Python,
    Rust,
    Typescript,
}

impl ProjectLanguage {
    pub(crate) fn filter(&self, templates: Vec<GithubTemplate>) -> Vec<GithubTemplate> {
        templates
            .into_iter()
            .filter_map(|template| {
                if self == &template.language {
                    Some(template)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn descriptor(&self) -> &'static str {
        use ProjectLanguage::*;
        match self {
            Java => "java",
            Javascript => "javascript",
            Python => "python",
            Rust => "rust",
            Typescript => "typescript",
        }
    }
}

impl Display for ProjectLanguage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.descriptor())
    }
}
