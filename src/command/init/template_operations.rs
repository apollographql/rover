use crate::options::TemplateFetcher;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};
use anyhow::anyhow;
use camino::Utf8PathBuf;
use itertools::Itertools;
use rover_http::ReqwestService;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use std::env;
use std::fs::read_dir;
use std::io;

pub struct TemplateOperations;

impl TemplateOperations {
    pub fn prompt_creation(artifacts: Vec<Utf8PathBuf>) -> io::Result<bool> {
        println!("The following files will be created:");
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        Self::print_grouped_files(artifacts_sorted);

        println!();
        prompt_confirm_default_yes("Proceed with creation?")
    }

    pub fn print_grouped_files(artifacts: Vec<Utf8PathBuf>) {
        for (_, files) in &artifacts
            .into_iter()
            .chunk_by(|artifact| artifact.parent().map(|p| p.to_owned()))
        {
            for file in files {
                if file.file_name().is_some() {
                    infoln!("{}", file);
                }
            }
        }
    }

    pub async fn _init_project(
        repo_url: &str,
        http_service: ReqwestService,
        output_path: Option<Utf8PathBuf>,
    ) -> RoverResult<()> {
        let current_dir = env::current_dir()?;
        let current_dir = Utf8PathBuf::from_path_buf(current_dir)
            .map_err(|_| anyhow::anyhow!("Failed to parse current directory"))?;

        let output_path = output_path.unwrap_or(current_dir);

        //TODO: move this in favor of the template command handling
        match read_dir(&output_path) {
            Ok(mut dir) => {
                if dir.next().is_some() {
                    return Err(RoverError::new(anyhow!(
                        "Cannot initialize the project because the '{}' directory is not empty.",
                        &output_path
                    ))
                    .with_suggestion(RoverErrorSuggestion::Adhoc(
                        "Please run Init on an empty directory".to_string(),
                    )));
                }
            }
            _ => {} // we could handle not found here but for init is unlikely
        }

        let template = TemplateFetcher::new(http_service)
            .call(repo_url.parse()?)
            .await?;

        // once all the work is ready, confirm with user:
        match Self::prompt_creation(template.list_files()?) {
            Ok(result) => {
                if result {
                    template.write_template(&output_path)?;
                } else {
                    println!("Project creation canceled. You can run this command again anytime.");
                }
            }
            Err(_) => Err(anyhow::anyhow!("Failed to prompt user for confirmation"))?,
        }

        Ok(())
    }
}