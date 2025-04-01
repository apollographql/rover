use crate::options::{ProjectUseCase, ProjectUseCaseOpt, TemplateFetcher};
use crate::{RoverError, RoverErrorSuggestion, RoverOutput, RoverResult};
use anyhow::anyhow;
use camino::Utf8PathBuf;
use clap::Parser;
use itertools::Itertools;
use rover_http::ReqwestService;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use serde::Serialize;
use std::fs::read_dir;
use std::{env, io};

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    #[clap(flatten)]
    use_case_options: ProjectUseCaseOpt,

    #[clap(long, hide(true))]
    path: Option<Utf8PathBuf>,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        let request_service = ReqwestService::builder().build()?;

        let use_case = self.use_case_options.get_or_prompt_use_case()?;
        match use_case {
            ProjectUseCase::Connectors => {
                let repo_url = "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz";
                self.init_project(repo_url, request_service, self.path.clone())
                    .await?;
            }
            ProjectUseCase::GraphQLTemplate => println!("\nComing soon!\n"),
        }

        Ok(RoverOutput::EmptySuccess)
    }

    fn prompt_creation(&self, artifacts: Vec<Utf8PathBuf>) -> io::Result<bool> {
        println!("The following files will be created:");
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        self.print_grouped_files(artifacts_sorted);

        println!();
        prompt_confirm_default_yes("Proceed with creation?")
    }

    fn print_grouped_files(&self, artifacts: Vec<Utf8PathBuf>) {
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

    async fn init_project(
        &self,
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
            _ => {} // we could handle not found here but for init is unlikely. Also, this block will be removed once we start using the template code
        }

        let template = TemplateFetcher::new(http_service)
            .call(repo_url.parse()?)
            .await?;

        // once all the work is ready, confirm with user:
        match self.prompt_creation(template.list_files()?) {
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
