use rover_http::ReqwestService;

use crate::command::init::config::ProjectConfig;
use crate::command::init::states::*;
use crate::command::init::prompts::*;
use crate::options::GraphIdOpt;
use crate::options::ProjectNameOpt;
use crate::options::{ProjectTypeOpt,  ProjectUseCaseOpt, ProjectOrganizationOpt};
use crate::{RoverOutput, RoverResult};

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
/// => You’re about to create a local directory with the following files:
///
/// .vscode/extensions.json
/// .idea/externalDependencies.xml
/// getting-started.md
/// router.yaml
/// supergraph.yaml
/// schema.graphql
///
/// ? Proceed with creation? (y/n): 

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
    
    pub fn preview_and_confirm_creation(self) -> RoverResult<Option<CreationConfirmed>> {
        // Create the configuration
        let config = self.create_config();
        
        // Get list of files that will be created
        // TODO: REPLACE DUMMY VALUES WITH ACTUAL VALUES
        let artifacts = vec![
            ".vscode/extensions.json".to_string(), 
            ".idea/externalDependencies.xml".to_string(), 
            "getting-started.md".to_string(), 
            "router.yaml".to_string(), 
            "supergraph.yaml".to_string(), 
            "schema.graphql".to_string()
        ];
        
        // Ask for confirmation
        let proceed = prompt_confirm_project_creation(&config, Some(&artifacts))?;
        
        if proceed {
            Ok(Some(CreationConfirmed {
                config,
                artifacts,
            }))
        } else {
            println!("Project creation canceled. You can run this command again anytime.");
            Ok(None)
        }
    }
}

/// PROMPT UX:
/// =========
/// 
/// ⣾ Creating files and generating GraphOS credentials..

// TODO: Replace with Daniel's implementation
impl CreationConfirmed {
  pub async fn create_project(self, _http_service: ReqwestService) -> RoverResult<ProjectCreated> {
      
      println!("TODO: Implement project creation");
      println!("⣾ Creating files and generating GraphOS credentials... (simulated)");
      
      Ok(ProjectCreated {
          config: self.config,
          artifacts: self.artifacts,
          api_key: "api-key-placeholder-12345".to_string(),
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