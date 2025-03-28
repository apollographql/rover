use crate::options::{ProjectUseCase, ProjectUseCaseOpt, TemplateFetcher};
use crate::{RoverOutput, RoverResult};
use camino::Utf8PathBuf;
use clap::Parser;
use itertools::Itertools;
use rover_http::ReqwestService;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use serde::Serialize;
use std::collections::HashSet;
use std::env;

#[derive(Debug, Serialize, Parser)]
pub struct Init {
    #[clap(flatten)]
    use_case_options: ProjectUseCaseOpt,
}

impl Init {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        println!("\nWelcome! This command helps you initialize a new GraphQL API project using Apollo Federation with Apollo Router.\n");

        let request_service = ReqwestService::builder().build()?;

        let use_case = self.use_case_options.get_or_prompt_use_case()?;
        match use_case {
            ProjectUseCase::Connectors => {
                let repo_url = "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz";
                self.init_project(repo_url, request_service).await?;
            }
            ProjectUseCase::GraphQLTemplate => println!("\nComing soon!\n"),
        }

        Ok(RoverOutput::EmptySuccess)
    }

    fn prompt_creation(&self, artifacts: Vec<String>) -> std::io::Result<bool> {
        infoln!("The following files will be created:");
        let mut top_level_artifacts = HashSet::new();
        artifacts
            .iter()
            .filter(|path| path.matches('/').count() == 1 || path.matches('/').count() == 0)
            .sorted()
            .for_each(|path| {
                top_level_artifacts.insert(path.clone());
            });

        for artifact in top_level_artifacts {
            infoln!("{}", artifact);
        }
        println!();

        prompt_confirm_default_yes("Proceed with creation?")
    }

    async fn init_project(&self, repo_url: &str, http_service: ReqwestService) -> RoverResult<()> {
        let fetcher = TemplateFetcher::new(repo_url.parse()?, http_service).await?;

        let current_dir = env::current_dir()?;
        let current_dir = Utf8PathBuf::from_path_buf(current_dir)
            .map_err(|_| anyhow::anyhow!("Failed to parse current directory"))?;

        let output_path = match env::var("INIT_OUTPUT_DIR") {
            Ok(value) => { Utf8PathBuf::from(value)}
            Err(_) => {current_dir}
        };

        //at this point, we have the compressed bytes in the fetcher
        // we can do here other prep work below

        // once all the work is ready, confirm with user:
        match self.prompt_creation(fetcher.list_files()?) {
            Ok(result) => {
                if result {
                    fetcher.write_template(&output_path)?;
                } else {
                    println!("Project creation canceled. You can run this command again anytime.");
                }
            }
            Err(_) => Err(anyhow::anyhow!("Failed to prompt user for confirmation"))?,
        }

        Ok(())
    }
}
