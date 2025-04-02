use std::env;
use std::fs::read_dir;

use camino::Utf8PathBuf;
use rover_http::ReqwestService;

use crate::command::init::config::ProjectConfig;
use crate::command::init::states::*;
use crate::command::init::helpers::*;
use crate::command::init::template_operations::TemplateOperations;
use crate::options::GraphIdOpt;
use crate::options::ProjectNameOpt;
use crate::options::ProjectUseCase;
use crate::options::TemplateFetcher;
use crate::options::{ProjectTypeOpt,  ProjectUseCaseOpt, ProjectOrganizationOpt};
use crate::RoverError;
use crate::RoverErrorSuggestion;
use crate::{RoverOutput, RoverResult};
use anyhow::anyhow;

/// PROMPT UX:
/// ==========
/// 
/// Welcome! This command helps you initialize a federated GraphQL API in your current directory.
/// To learn more about init, run `rover init -h` or visit https://www.apollographql.com/docs/rover/commands/init
/// 
/// ? Select option:
/// > Create a new GraphQL API
///   Add a subgraph to an existing GraphQL API

impl Welcome {
    pub fn new() -> Self {
        Welcome
    }

    pub fn select_project_type(self, options: &ProjectTypeOpt) -> RoverResult<ProjectTypeSelected> {
      display_welcome_message();
      
      let project_type = match options.get_project_type() {
        Some(ptype) => ptype,
        None => options.prompt_project_type()?,
    };
      
      Ok(ProjectTypeSelected { project_type })
  }
}

/// PROMPT UX:
/// =========
/// 
/// ? Select an organization:
/// > Org1
///   Org2
///   Org3

impl ProjectTypeSelected {
    pub fn select_organization(self, options: &ProjectOrganizationOpt) -> RoverResult<OrganizationSelected> {
        // TODO: Get list of organizations from Studio Client
        let organizations: Vec<String> = vec!["default-organization".to_string()]; 
        
        let organization = options.get_or_prompt_organization(&organizations)?;
        
        Ok(OrganizationSelected {
            project_type: self.project_type,
            organization,
        })
    }
}

/// PROMPT UX:
/// =========
/// 
/// ? Select use case:
/// > Connect one or more REST APIs
///   Start a GraphQL API with recommended libraries

impl OrganizationSelected {
    pub fn select_use_case(self, options: &ProjectUseCaseOpt) -> RoverResult<UseCaseSelected> {
        let use_case = options.get_or_prompt_use_case()?;
        
        Ok(UseCaseSelected {
            project_type: self.project_type,
            organization: self.organization,
            use_case,
        })
    }
}

/// PROMPT UX:
/// =========
/// 
/// ? Name your GraphQL API: 

impl UseCaseSelected {
  pub fn enter_project_name(self, options: &ProjectNameOpt) -> RoverResult<ProjectNamed> {
      let project_name = options.get_or_prompt_project_name()?;
      
      Ok(ProjectNamed {
          project_type: self.project_type,
          organization: self.organization,
          use_case: self.use_case,
          project_name,
      })
  }
}

/// PROMPT UX:
/// =========
/// 
/// ? Confirm or modify graph ID (start with a letter and use only letters, numbers, and dashes): [ana-test-3-wuqfnu]

impl ProjectNamed {
  pub fn confirm_graph_id(self, options: &GraphIdOpt) -> RoverResult<GraphIdConfirmed> {
      let graph_id = options.get_or_prompt_graph_id(&self.project_name)?;
      
      Ok(GraphIdConfirmed {
          project_type: self.project_type,
          organization: self.organization,
          use_case: self.use_case,
          project_name: self.project_name,
          graph_id,
      })
  }
}

/// PROMPT UX:
/// =========
/// 
/// => You're about to create a local directory with the following files:
///
/// .vscode/extensions.json
/// .idea/externalDependencies.xml
/// getting-started.md
/// router.yaml
/// supergraph.yaml
/// schema.graphql
///
/// ? Proceed with creation? (y/n): 

/******************************************************************
 * We've split the functionality from init_project across two state 
 * transitions to fit the state machine pattern:
 * 
 * 1. GraphIdConfirmed.preview_and_confirm_creation:
 *    - Takes the template fetching part to get the list of files
 *    - Uses TemplateOperations.prompt_creation to display files and ask for confirmation
 *    - Stores the repo_url for use in the next state
 * 
 * 2. CreationConfirmed.create_project:
 *    - Takes the directory checking logic
 *    - Fetches the template again (maybe could be optimized)
 *    - Writes the template files
 *    - Gets final list of created files
 * 
 * This approach preserves the same functionality as init_project
 ******************************************************************/
 impl GraphIdConfirmed {
  fn create_config(&self) -> ProjectConfig {
      ProjectConfig {
          organization: self.organization.clone(),
          use_case: self.use_case.clone(),
          project_name: self.project_name.clone(),
          graph_id: self.graph_id.clone(),
          project_type: self.project_type.clone(),
      }
  }
  
  // This method handles the first part of what init_project does:
  // - Determine the repository URL
  // - Fetch the template 
  // - Display files and get confirmation
  pub async fn preview_and_confirm_creation(self, http_service: ReqwestService) -> RoverResult<Option<CreationConfirmed>> {
      // Create the configuration
      let config = self.create_config();
      
      // Determine the repository URL based on the use case
      let repo_url = match self.use_case {
          ProjectUseCase::Connectors => "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz",
          ProjectUseCase::GraphQLTemplate => {
              println!("\nGraphQL Template is coming soon!\n");
              return Ok(None); // Early return if template not available
          },
      };
      
      // Fetch the template to get the list of files
      let template_fetcher = TemplateFetcher::new(http_service)
          .call(repo_url.parse()?)
          .await?;
      
      // Get list of files that will be created
      let artifacts = template_fetcher.list_files()?;
      
      // This directly uses TemplateOperations.prompt_creation from the original implementation
      match TemplateOperations::prompt_creation(artifacts.clone()) {
          Ok(true) => {
              // User confirmed, proceed to create files
              Ok(Some(CreationConfirmed {
                  config,
                  repo_url: repo_url.to_string(),
                  output_path: None, // Default to current directory
              }))
          },
          Ok(false) => {
              // User canceled
              println!("Project creation canceled. You can run this command again anytime.");
              Ok(None)
          },
          Err(e) => Err(anyhow!("Failed to prompt user for confirmation: {}", e).into()),
      }
  }
}

/// PROMPT UX:
/// =========
/// 
/// ⣾ Creating files and generating GraphOS credentials..

// This method handles the second part of what init_project does:
// - Check if directory is empty
// - Fetch the template again
// - Write the template files
impl CreationConfirmed {
  pub async fn create_project(self, http_service: ReqwestService) -> RoverResult<ProjectCreated> {
      println!("⣾ Creating files and generating GraphOS credentials...");
      
      // This logic is taken directly from init_project's directory path handling
      let current_dir = env::current_dir()?;
      let current_dir = Utf8PathBuf::from_path_buf(current_dir)
          .map_err(|_| anyhow::anyhow!("Failed to parse current directory"))?;
      let output_path = self.output_path.unwrap_or(current_dir);
      
      // This directory checking logic is copied from init_project
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
          _ => {} // Directory doesn't exist or can't be read
      }
      
      // We re-fetch the template here - could potentially be optimized
      // to pass the template from the previous state
      let template_fetcher = TemplateFetcher::new(http_service.clone())
          .call(self.repo_url.parse()?)
          .await?;
      
      // Write the template files without asking for confirmation again
      // (confirmation was done in the previous state)
      template_fetcher.write_template(&output_path)?;
      
      // Get the list of created files
      let artifacts = template_fetcher.list_files()?;
      
      // API key creation would happen here in a real implementation
      let api_key = "api-key-placeholder-12345".to_string();
      
      Ok(ProjectCreated {
          config: self.config,
          artifacts,
          api_key,
      })
  }
}

/// PROMPT UX:
/// =========
/// 
/// => All set! Your project `ana-test` has been created. Please review details below to see what was generated.
/// 
/// Project directory, etc.


impl ProjectCreated {
    pub fn complete(self) -> Completed {
        println!("\n=> All set! Your project `{}` has been created. Please review details below to see what was generated.", self.config.project_name);
        
        // Display created files
        println!("\nProject directory");
        for artifact in &self.artifacts {
            println!("✓ {}", artifact);
        }
        
        // Display credentials
        println!("\nGraphOS credentials for your GraphQL API");
        println!("✓ APOLLO_GRAPH_REF={}@current (Formatted graph-id@variant, references a GraphQL API in the Apollo GraphOS platform)", self.config.graph_id);
        println!("✓ APOLLO_KEY={} (This is your project's API key, also known as a graph API key)", self.api_key);
        
        // Display next steps
        println!("\n️▲ Before you proceed:");
        println!("- Set your graph API key as an environment variable (learn more about env vars by running `rover docs open config`)");
        println!("- Save your graph ref (You can also get it from Studio by visiting your graph variant's home page)");
        
        println!("\nNext steps Run the following command to start a local development session:  $ rover dev --supergraph-config supergraph.yaml  For more information, check out `getting-started.md`.");
        
        Completed
    }
}

// Completed state transition
impl Completed {
    pub fn success(self) -> RoverOutput {
        RoverOutput::EmptySuccess
    }
}