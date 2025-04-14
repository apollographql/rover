use std::env;
use std::fs::read_dir;
use std::path::PathBuf;

use camino::Utf8PathBuf;
use rover_client::operations::init::create_graph;
use rover_client::operations::init::memberships::{self};
use rover_client::operations::subgraph::publish;
use rover_client::shared::GitContext;
use rover_client::shared::GraphRef;
use rover_http::ReqwestService;

use crate::command::init::config::ProjectConfig;
use crate::command::init::graph_id::GraphId;
use crate::command::init::helpers::*;
use crate::command::init::operations::create_api_key;
use crate::command::init::states::*;
use crate::command::init::template_operations::{SupergraphBuilder, TemplateOperations};
use crate::options::GraphIdOpt;
use crate::options::Organization;
use crate::options::ProjectAuthenticationOpt;
use crate::options::ProjectNameOpt;
use crate::options::ProjectUseCase;
use crate::options::TemplateFetcher;
use crate::options::{ProfileOpt, ProjectOrganizationOpt, ProjectTypeOpt, ProjectUseCaseOpt};
use crate::utils::client::StudioClientConfig;
use crate::RoverError;
use crate::RoverErrorSuggestion;
use crate::{RoverOutput, RoverResult};
use anyhow::anyhow;
use rover_client::operations::init::create_graph::*;
use rover_client::operations::subgraph::publish::*;

/// PROMPT UX:
/// =========
///
/// No credentials found. Please go to http://studio.apollographql.com/user-settings/api-keys and create a new Personal API key.
///
/// Copy the key and paste it into the prompt below.
/// ?
impl UserAuthenticated {
    pub fn new() -> Self {
        UserAuthenticated {}
    }

    pub async fn check_authentication(
        self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<Welcome> {
        match client_config.get_authenticated_client(profile) {
            Ok(_) => Ok(Welcome::new()),
            Err(_) => {
                // Use the new prompt_for_api_key function to handle authentication
                match ProjectAuthenticationOpt::default()
                    .prompt_for_api_key(&client_config, profile)
                {
                    Ok(_) => {
                        // Try to authenticate again with the new credentials
                        match client_config.get_authenticated_client(profile) {
                            Ok(_) => Ok(Welcome::new()),
                            Err(_) => Err(anyhow!("Failed to get authenticated client").into()),
                        }
                    }
                    Err(e) => Err(anyhow!("Failed to set API key: {}", e).into()),
                }
            }
        }
    }
}

/// PROMPT UX:
/// ==========
///
/// Welcome! This command helps you initialize a federated Graph in your current directory.
/// To learn more about init, run `rover init -h` or visit https://www.apollographql.com/docs/rover/commands/init
///
/// ? Select option:
/// > Create a new Graph
/// > Add a subgraph to an existing Graph
impl Welcome {
    pub fn new() -> Self {
        Welcome {}
    }

    pub fn select_project_type(
        self,
        options: &ProjectTypeOpt,
        override_install_path: &Option<PathBuf>,
    ) -> RoverResult<ProjectTypeSelected> {
        display_welcome_message();

        // Check if directory is empty before proceeding
        let current_dir = env::current_dir()?;
        let output_path =
            Utf8PathBuf::from_path_buf(override_install_path.clone().unwrap_or(current_dir))
                .map_err(|_| anyhow::anyhow!("Failed to parse directory"))?;
        match read_dir(&output_path) {
            Ok(mut dir) => {
                if dir.next().is_some() {
                    return Err(RoverError::new(anyhow!(
                        "Cannot initialize the project because the current directory is not empty."
                    ))
                    .with_suggestion(RoverErrorSuggestion::Adhoc(
                        "Please run `init` on an empty directory".to_string(),
                    )));
                }
            }
            _ => {} // Directory doesn't exist or can't be read
        }

        let project_type = match options.get_project_type() {
            Some(ptype) => ptype,
            None => options.prompt_project_type()?,
        };

        Ok(ProjectTypeSelected {
            project_type,
            output_path,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// ? Select an organization:
/// > Org1
/// > Org2
/// > Org3
impl ProjectTypeSelected {
    pub async fn select_organization(
        self,
        options: &ProjectOrganizationOpt,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
    ) -> RoverResult<OrganizationSelected> {
        let client = client_config.get_authenticated_client(profile)?;
        let memberships_response = memberships::run(&client).await?;
        let organizations = memberships_response
            .memberships
            .iter()
            .map(|m| Organization::new(m.name.clone(), m.id.clone()))
            .collect::<Vec<_>>();
        let organization = options.get_or_prompt_organization(&organizations)?;
        Ok(OrganizationSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            organization,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// ? Select use case:
/// > Start a GraphQL API with one or more REST APIs
/// > Start a GraphQL API with recommended libraries
impl OrganizationSelected {
    pub fn select_use_case(self, options: &ProjectUseCaseOpt) -> RoverResult<UseCaseSelected> {
        let use_case = options.get_or_prompt_use_case()?;

        Ok(UseCaseSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// ? Name your Graph:
impl UseCaseSelected {
    pub fn enter_project_name(self, options: &ProjectNameOpt) -> RoverResult<ProjectNamed> {
        let project_name = options.get_or_prompt_project_name()?;

        Ok(ProjectNamed {
            output_path: self.output_path,
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
        let graph_id = options.get_or_prompt_graph_id(&self.project_name.to_string())?;

        Ok(GraphIdConfirmed {
            output_path: self.output_path,
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

    pub async fn preview_and_confirm_creation(
        self,
        http_service: ReqwestService,
    ) -> RoverResult<Option<CreationConfirmed>> {
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

        match TemplateOperations::prompt_creation(artifacts.clone()) {
            Ok(true) => {
                // User confirmed, proceed to create files
                Ok(Some(CreationConfirmed {
                    config,
                    template: template_fetcher,
                    output_path: self.output_path,
                }))
            }
            Ok(false) => {
                // User canceled
                println!("Project creation canceled. You can run this command again anytime.");
                Ok(None)
            }
            Err(e) => Err(anyhow!("Failed to prompt user for confirmation: {}", e).into()),
        }
    }
}

/// PROMPT UX:
/// =========
///
/// ⣾ Creating files and generating GraphOS credentials..
impl CreationConfirmed {
    pub async fn create_project(
        self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<ProjectCreated> {
        println!("⣾ Creating files and generating GraphOS credentials...");
        let client = client_config.get_authenticated_client(profile)?;

        // Write the template files without asking for confirmation again
        // (confirmation was done in the previous state)
        self.template.write_template(&self.output_path)?;
        
        let supergraph = SupergraphBuilder::new(self.output_path, 5);
        supergraph.build_and_write()?;
        
        let artifacts = self.template.list_files()?;

        let create_graph_response = create_graph::run(
            CreateGraphInput {
                hidden_from_uninvited_non_admin: false,
                create_graph_id: self.config.graph_id.to_string(),
                title: self.config.project_name.to_string(),
                organization_id: self.config.organization.to_string(),
            },
            &client,
        )
        .await?;
    
        let subgraphs = supergraph.generate_subgraphs()?;
        for (subgraph_name, subgraph_config) in subgraphs.iter() {
            println!("Publishing subgraph: {}", subgraph_name);
            publish::run(
                SubgraphPublishInput {
                    graph_ref: GraphRef {
                        name: create_graph_response.id.clone(),
                        variant: "current".to_string(),
                    },
                    subgraph: subgraph_name.to_string(),
                    url: subgraph_config.routing_url.clone(),
                    schema: "type Query { id: ID! }".to_string(), // TODO: Get the SDL from the subgraph config
                    git_context: GitContext {
                        branch: None,
                        commit: None,
                        author: None,
                        remote_url: None,
                    },
                    convert_to_federated_graph: false,
                },
                &client,
            )
            .await?;
        }

        // Create a new API key for the project first
        let api_key = create_api_key(
            client_config,
            profile,
            self.config.graph_id.to_string(),
            self.config.project_name.to_string(),
        )
        .await?;
    
        Ok(ProjectCreated {
            config: self.config,
            artifacts,
            api_key,
            graph_id: create_graph_response.id.parse::<GraphId>().unwrap(),
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
        display_project_created_message(
            &self.config.project_name.to_string(),
            &self.artifacts,
            &self.config.graph_id,
            &self.api_key,
        );

        Completed
    }
}

// Completed state transition
impl Completed {
    pub fn success(self) -> RoverOutput {
        RoverOutput::EmptySuccess
    }
}
