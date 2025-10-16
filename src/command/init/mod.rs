#[cfg(feature = "composition-js")]
mod authentication;
#[cfg(feature = "composition-js")]
mod config;
#[cfg(feature = "composition-js")]
mod graph_id;
#[cfg(feature = "composition-js")]
mod helpers;
#[cfg(feature = "composition-js")]
mod mcp;
#[cfg(feature = "composition-js")]
mod operations;
#[cfg(feature = "composition-js")]
pub mod options;
#[cfg(feature = "composition-js")]
pub mod states;
#[cfg(feature = "composition-js")]
mod template_fetcher;
#[cfg(feature = "composition-js")]
pub mod template_operations;
#[cfg(all(test, feature = "composition-js"))]
pub mod tests;
#[cfg(feature = "composition-js")]
pub mod transitions;
#[cfg(feature = "composition-js")]
use crate::RoverError;
#[cfg(feature = "composition-js")]
use crate::command::init::helpers::update_template_files_with_real_values;
#[cfg(feature = "composition-js")]
use crate::command::init::options::ProjectTemplateOpt;
#[cfg(feature = "composition-js")]
use crate::command::init::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectType, ProjectTypeOpt,
    ProjectUseCase, ProjectUseCaseOpt,
};
#[cfg(feature = "composition-js")]
use crate::command::init::transitions::DEFAULT_VARIANT;
#[cfg(feature = "composition-js")]
use crate::error::RoverErrorSuggestion;
#[cfg(feature = "composition-js")]
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::{RoverOutput, RoverResult};
use clap::Parser;
#[cfg(feature = "composition-js")]
use rover_client::RoverClientError;
#[cfg(feature = "composition-js")]
use rover_std::hyperlink;
use serde::Serialize;
use std::path::PathBuf;

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
enum MCPSetupType {
    ExistingGraph,
    NewProject,
}

#[cfg(feature = "composition-js")]
impl std::fmt::Display for MCPSetupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPSetupType::ExistingGraph => {
                write!(
                    f,
                    "Create MCP tools from an existing Apollo GraphOS project"
                )
            }
            MCPSetupType::NewProject => {
                write!(f, "Create MCP tools from a new Apollo GraphOS project")
            }
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
enum MCPProjectType {
    Rest,
    GraphQL,
}

#[cfg(feature = "composition-js")]
impl std::fmt::Display for MCPProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPProjectType::Rest => write!(
                f,
                "MCP server for REST APIs (make existing REST services AI-accessible)"
            ),
            MCPProjectType::GraphQL => write!(
                f,
                "MCP server for GraphQL APIs (make GraphQL services AI-accessible)"
            ),
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
enum MCPDataSourceType {
    ExternalAPIs, // REST, webhooks, SaaS
    GraphQLAPI,   // Existing GraphQL endpoints
}

#[cfg(feature = "composition-js")]
impl std::fmt::Display for MCPDataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPDataSourceType::ExternalAPIs => {
                write!(f, "Apollo graph with Connectors (connect to REST services)")
            }
            MCPDataSourceType::GraphQLAPI => write!(
                f,
                "Apollo graph with GraphQL endpoints (connect to existing GraphQL endpoints)"
            ),
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
struct GraphVariantOption {
    organization_name: String,
    graph_id: String,
    graph_name: String,
    variant_name: String,
    display_name: String,
}

#[cfg(feature = "composition-js")]
pub use template_fetcher::InitTemplateFetcher;

#[cfg(feature = "composition-js")]
use transitions::{CreateProjectResult, RestartReason};

#[derive(Debug, Parser, Clone, Serialize)]
#[clap(about = "Initialize a new graph")]
pub struct Init {
    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_template: ProjectTemplateOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_type: ProjectTypeOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    organization: ProjectOrganizationOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_use_case: ProjectUseCaseOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    project_name: ProjectNameOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    graph_id: GraphIdOpt,

    #[clap(flatten)]
    #[cfg(feature = "composition-js")]
    profile: ProfileOpt,

    #[clap(long, hide(true))]
    path: Option<PathBuf>,
}

impl Init {
    #[cfg(feature = "composition-js")]
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        use crate::command::init::states::{ProjectTypeSelected, UserAuthenticated};
        use camino::Utf8PathBuf;
        use helpers::display_use_template_message;
        use std::env;

        let welcome = UserAuthenticated::new()
            .check_authentication(&client_config, &self.profile)
            .await?;

        // Branch to MCP flow BEFORE directory validation
        if self.project_template.mcp {
            // Create ProjectTypeSelected state for MCP flow (bypasses directory check)
            let project_type = self
                .project_type
                .get_project_type()
                .unwrap_or(ProjectType::CreateNew); // Default to CreateNew for MCP

            let current_dir = env::current_dir()?;
            let output_path = Utf8PathBuf::from_path_buf(self.path.clone().unwrap_or(current_dir))
                .map_err(|_| anyhow::anyhow!("Failed to parse directory"))?;

            let project_type_selected = ProjectTypeSelected {
                project_type,
                output_path,
            };

            return self
                .handle_mcp_flow(project_type_selected, &client_config)
                .await;
        }

        let project_type_selected =
            welcome.select_project_type(&self.project_type, &self.path, &self.project_template)?;

        // Early return for AddSubgraph case
        if project_type_selected.project_type == ProjectType::AddSubgraph {
            display_use_template_message();
            return Ok(RoverOutput::EmptySuccess);
        }

        // Handle new project creation flow
        let use_case_selected = match project_type_selected
            .select_organization(&self.organization, &self.profile, &client_config)
            .await?
            .select_use_case(&self.project_use_case)?
        {
            Some(use_case) => use_case,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        let creation_confirmed = match use_case_selected
            .select_template(&self.project_template)
            .await?
            .enter_project_name(&self.project_name)?
            .confirm_graph_id(&self.graph_id)?
            .preview_and_confirm_creation()
            .await?
        {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        let project_created = creation_confirmed
            .create_project(&client_config, &self.profile)
            .await?;

        // Handle project creation result
        if let CreateProjectResult::Created(project) = project_created {
            update_template_files_with_real_values(&project)?;
            return Ok(project.complete().success());
        }

        // Handle restart loop
        if let CreateProjectResult::Restart {
            state: mut current_project,
            reason: _,
        } = project_created
        {
            const MAX_RETRIES: u8 = 3;

            for attempt in 0..MAX_RETRIES {
                if attempt >= MAX_RETRIES {
                    let suggestion = RoverErrorSuggestion::Adhoc(format!(
                        "If the issue persists, please contact support at {}.",
                        hyperlink("https://support.apollographql.com")
                    ));
                    let error = RoverError::from(RoverClientError::MaxRetriesExceeded {
                        max_retries: MAX_RETRIES,
                    })
                    .with_suggestion(suggestion);
                    return Err(error);
                }

                let graph_id_confirmed = current_project.confirm_graph_id(&self.graph_id)?;
                let creation_confirmed =
                    match graph_id_confirmed.preview_and_confirm_creation().await? {
                        Some(confirmed) => confirmed,
                        None => return Ok(RoverOutput::EmptySuccess),
                    };

                match creation_confirmed
                    .create_project(&client_config, &self.profile)
                    .await?
                {
                    CreateProjectResult::Created(project) => {
                        update_template_files_with_real_values(&project)?;
                        return Ok(project.complete().success());
                    }
                    CreateProjectResult::Restart {
                        state: project_named,
                        reason,
                    } => match reason {
                        RestartReason::FullRestart => {
                            let welcome = UserAuthenticated::new()
                                .check_authentication(&client_config, &self.profile)
                                .await?;
                            welcome.select_project_type(
                                &self.project_type,
                                &self.path,
                                &self.project_template,
                            )?;
                            return Ok(RoverOutput::EmptySuccess);
                        }
                        _ => {
                            current_project = project_named;
                            continue;
                        }
                    },
                }
            }
        }

        Ok(RoverOutput::EmptySuccess)
    }

    /// Handle MCP flow using dedicated state transitions
    #[cfg(feature = "composition-js")]
    async fn handle_mcp_flow(
        &self,
        project_type_selected: crate::command::init::states::ProjectTypeSelected,
        client_config: &StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        use crate::command::init::states::*;

        // Initialize MCP augmentation (bypasses directory check)
        let mcp_init = project_type_selected.initialize_mcp_augmentation(&self.project_template)?;

        // Step 1: MCP Setup Type Selection (New Project vs Existing Graph)
        let mcp_setup_selected = mcp_init.select_setup_type(&self.project_type)?;

        match mcp_setup_selected.setup_type {
            MCPSetupType::NewProject => {
                // Step 2: Data Source Selection for New Project (REST vs GraphQL)
                let mcp_data_source_selected =
                    mcp_setup_selected.select_data_source(&self.project_use_case)?;

                // Step 3: Continue with organization selection
                let mcp_org_selected = mcp_data_source_selected
                    .select_organization(&self.organization, &self.profile, client_config)
                    .await?;

                // Step 4: Template composition based on data source
                let mcp_template_composed = mcp_org_selected.compose_mcp_template().await?;

                // Step 5: Project naming
                let mcp_project_named =
                    mcp_template_composed.enter_project_name(&self.project_name)?;

                // Step 6: Graph ID confirmation
                let mcp_graph_confirmed = mcp_project_named.confirm_graph_id(&self.graph_id)?;

                // Step 7: MCP-specific preview and confirmation
                let mcp_creation_previewed =
                    match mcp_graph_confirmed.preview_mcp_creation().await? {
                        Some(previewed) => previewed,
                        None => return Ok(RoverOutput::EmptySuccess), // User cancelled
                    };

                // Step 8: Convert to MCPCreationConfirmed for type-safe MCP project creation
                let mcp_creation_confirmed =
                    mcp_creation_previewed.into_mcp_creation_confirmed()?;

                // Step 9: Follow the MCP-specific project creation flow
                let project_created = mcp_creation_confirmed
                    .create_project(client_config, &self.profile)
                    .await?;

                // Handle project creation result
                if let CreateProjectResult::Created(project) = project_created {
                    update_template_files_with_real_values(&project)?;
                    return Ok(project.complete().success());
                }

                // Handle restart logic if needed
                Ok(RoverOutput::EmptySuccess)
            }
            MCPSetupType::ExistingGraph => {
                // Handle existing graph MCP flow directly
                let client = client_config.get_authenticated_client(&self.profile)?;
                self.handle_existing_graph_mcp(&client, client_config).await
            }
        }
    }

    /// DEPRECATED: Handle MCP augmentation directly without going through project creation flow
    /// This method is being replaced by handle_mcp_flow using dedicated state transitions
    ///
    /// Template Variable Convention:
    /// - YAML files use ${VARIABLE} format to avoid YAML linting errors
    /// - Other templates use {{VARIABLE}} format for standard template syntax
    /// - Both formats are supported for compatibility during transition
    #[cfg(feature = "composition-js")]
    #[allow(dead_code)]
    async fn _deprecated_handle_mcp_augmentation(
        &self,
        client_config: &StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        use crate::command::init::authentication::{
            AuthenticationError, auth_error_to_rover_error,
        };
        use anyhow::anyhow;
        use rover_std::Style;
        use std::env;

        // Validate that directory is empty
        let current_dir = env::current_dir()?;
        let output_path = match &self.path {
            Some(path) => {
                camino::Utf8PathBuf::try_from(path.clone()).map_err(|_| anyhow!("Invalid path"))?
            }
            None => camino::Utf8PathBuf::from_path_buf(current_dir)
                .map_err(|_| anyhow!("Failed to parse directory"))?,
        };

        // Check if directory is empty
        if let Ok(mut dir) = std::fs::read_dir(&output_path)
            && dir.next().is_some()
        {
            return Err(RoverError::new(anyhow!(
                "Cannot initialize MCP server because the current directory is not empty"
            ))
            .with_suggestion(RoverErrorSuggestion::Adhoc(
                format!(
                    "Please run `{}` in an empty directory or use the `--path` flag to specify a different directory.\n\nIf you are wanting to use MCP with an existing project, you can run `{}` to add the all-in-one container to your project. See our docs for more information on how to configure it: {}",
                    Style::Command.paint("rover init --mcp"),
                    Style::Command.paint("rover dev --mcp"),
                   Style::Command.paint("rover docs open mcp-config"),
                ),
            )));
        }

        // Authenticate first
        let client = match client_config.get_authenticated_client(&self.profile) {
            Ok(client) => client,
            Err(_) => {
                return Err(auth_error_to_rover_error(
                    AuthenticationError::NoCredentialsFound,
                ));
            }
        };

        // Determine MCP setup type from project_type argument or prompt
        let setup_type = self.get_or_prompt_mcp_setup_type()?;

        match setup_type {
            MCPSetupType::ExistingGraph => {
                self.handle_existing_graph_mcp(&client, client_config).await
            }
            MCPSetupType::NewProject => self.handle_new_project_mcp(&client, client_config).await,
        }
    }

    /// Get MCP setup type from project_type argument or prompt user
    #[cfg(feature = "composition-js")]
    fn get_or_prompt_mcp_setup_type(&self) -> RoverResult<MCPSetupType> {
        // Check if project_type was provided via command line
        if let Some(project_type) = &self.project_type.project_type {
            let setup_type = match project_type {
                ProjectType::CreateNew => MCPSetupType::NewProject,
                ProjectType::AddSubgraph => MCPSetupType::ExistingGraph,
            };
            return Ok(setup_type);
        }

        // If no argument provided, prompt the user
        Self::prompt_mcp_setup_type()
    }

    /// Prompt user to choose MCP setup type
    #[cfg(feature = "composition-js")]
    fn prompt_mcp_setup_type() -> RoverResult<MCPSetupType> {
        use anyhow::anyhow;
        use dialoguer::Select;
        use dialoguer::console::Term;
        use rover_std::Style;

        println!();
        println!(
            "Welcome! This command helps you initialize a federated graph with MCP server capabilities."
        );
        println!();
        println!(
            "To learn more about init, run `{}` or visit {}",
            Style::Command.paint("rover init --mcp -h"),
            hyperlink("https://www.apollographql.com/docs/rover/commands/init")
        );
        println!();

        let setup_types = vec![MCPSetupType::NewProject, MCPSetupType::ExistingGraph];

        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select option"))
            .items(&setup_types)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => {
                let selected = setup_types[index].clone();
                Ok(selected)
            }
            _ => Err(RoverError::new(anyhow!("Selection cancelled"))),
        }
    }

    /// Preview files for new project creation
    #[cfg(feature = "composition-js")]
    async fn preview_mcp_new_project_files(
        &self,
        _project_name: &str,
        template_files: &std::collections::HashMap<camino::Utf8PathBuf, Vec<u8>>,
    ) -> RoverResult<bool> {
        use crate::command::init::helpers::print_mcp_file_categories;
        use dialoguer::Confirm;
        use dialoguer::console::Term;
        use rover_std::Style;

        println!();
        println!("=> You're about to add the following files to your local directory:");

        let file_paths: Vec<camino::Utf8PathBuf> = template_files.keys().cloned().collect();
        print_mcp_file_categories(file_paths);

        println!();
        println!("{}", Style::File.paint("What this template gives you"));
        println!("- Example GraphQL schema and REST connectors");
        println!("- Pre-configured MCP server with Docker setup");
        println!("- Sample tools showing how to make APIs AI-callable");
        println!();

        let confirmed = Confirm::new()
            .with_prompt("Create this template?")
            .default(true)
            .interact_on_opt(&Term::stderr())?;

        Ok(confirmed.unwrap_or(false))
    }

    /// Preview files to be created before generation with confirm dialog
    #[cfg(feature = "composition-js")]
    async fn preview_mcp_files(
        &self,
        _selected_graph: &GraphVariantOption,
        files: &std::collections::HashMap<camino::Utf8PathBuf, String>,
    ) -> RoverResult<bool> {
        use crate::command::init::helpers::print_mcp_file_categories;
        use dialoguer::Confirm;
        use dialoguer::console::Term;

        println!();
        println!("=> You're about to add the following files to your local directory:");
        let file_paths: Vec<camino::Utf8PathBuf> = files.keys().cloned().collect();
        print_mcp_file_categories(file_paths);
        println!();

        let confirmed = Confirm::new()
            .with_prompt("Create this template?")
            .default(true)
            .interact_on_opt(&Term::stderr())?;

        Ok(confirmed.unwrap_or(false))
    }

    /// Prompt user to select a graph variant
    #[cfg(feature = "composition-js")]
    fn prompt_graph_selection(
        graph_options: Vec<GraphVariantOption>,
    ) -> RoverResult<GraphVariantOption> {
        use anyhow::anyhow;
        use dialoguer::FuzzySelect;
        use dialoguer::console::Term;
        use rover_std::Style;

        let display_names = graph_options
            .iter()
            .map(|option| option.display_name.clone())
            .collect::<Vec<_>>();

        let selection = FuzzySelect::new()
            .with_prompt(
                Style::Prompt
                    .paint("? Select existing graph variant to work with (or type graph id):"),
            )
            .highlight_matches(true)
            .items(&display_names)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => {
                let selected = &graph_options[index];
                Ok(selected.clone())
            }
            None => Err(RoverError::new(anyhow!("Graph selection cancelled"))),
        }
    }

    /// Handle MCP setup for existing graph
    #[cfg(feature = "composition-js")]
    async fn handle_existing_graph_mcp(
        &self,
        client: &rover_client::blocking::StudioClient,
        client_config: &StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        use crate::command::init::authentication::{
            AuthenticationError, auth_error_to_rover_error,
        };
        use anyhow::anyhow;
        use rover_std::{Style, hyperlink};

        // Query GraphOS for user's organizations and their graphs
        use rover_client::operations::init::{list_graphs, memberships};
        use rover_client::operations::subgraph::list::{self as list_subgraphs, SubgraphListInput};
        use rover_client::operations::supergraph::fetch::{
            self as fetch_supergraph, SupergraphFetchInput,
        };
        use rover_client::shared::GraphRef;

        let memberships_response = memberships::run(client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(msg))
            }
            e => e.into(),
        })?;

        if memberships_response.memberships.is_empty() {
            println!(
                "{}",
                Style::WarningHeading.paint("▲ No organizations found")
            );
            println!(
                "You need to create an organization first. Visit {} to create your first organization.",
                hyperlink("https://studio.apollographql.com")
            );
            return Ok(RoverOutput::EmptySuccess);
        }

        // Collect all graphs from all organizations
        let mut all_graph_options = Vec::new();

        for org in &memberships_response.memberships {
            let list_graphs_response = list_graphs::run(
                list_graphs::ListGraphsInput {
                    organization_id: org.id.clone(),
                },
                client,
            )
            .await
            .map_err(|e| {
                RoverError::new(anyhow!(
                    "Failed to fetch graphs for organization {}: {}",
                    org.name,
                    e
                ))
            })?;

            for graph in list_graphs_response.organization.graphs {
                for variant in graph.variants {
                    all_graph_options.push(GraphVariantOption {
                        organization_name: org.name.clone(),
                        graph_id: graph.id.clone(),
                        graph_name: graph.name.clone(),
                        variant_name: variant.name.clone(),
                        display_name: if memberships_response.memberships.len() > 1 {
                            format!("{}/{} ({})", graph.name, variant.name, org.name)
                        } else {
                            format!("{} ({})", graph.name, variant.name)
                        },
                    });
                }
            }
        }

        if all_graph_options.is_empty() {
            println!("{}", Style::WarningHeading.paint("▲ No graphs found"));
            println!(
                "You must have a graph registered in GraphOS. Visit {} to connect an existing graph or run `{}` to create a new graph pre-configured to work with the Apollo MCP Server.",
                hyperlink("https://studio.apollographql.com"),
                Style::Command.paint("rover init --mcp")
            );
            return Ok(RoverOutput::EmptySuccess);
        }

        // Check if graph_id was provided via command line
        let selected_graph = if let Some(ref graph_id) = self.graph_id.graph_id {
            // Try to parse the graph_id (format: graph-id@variant or just graph-id)
            let (graph_part, variant_part) = if graph_id.contains('@') {
                let parts: Vec<&str> = graph_id.splitn(2, '@').collect();
                (parts[0].to_string(), Some(parts[1].to_string()))
            } else {
                (graph_id.clone(), None)
            };

            // Find matching graph in the list
            let matching_graph = all_graph_options.iter().find(|option| {
                option.graph_id == graph_part
                    && (variant_part.is_none()
                        || variant_part.as_deref() == Some(&option.variant_name))
            });

            match matching_graph {
                Some(graph) => graph.clone(),
                None => {
                    // If no exact match found, fall back to prompting
                    eprintln!(
                        "Warning: Specified graph '{}' not found in available graphs",
                        graph_id
                    );
                    Self::prompt_graph_selection(all_graph_options)?
                }
            }
        } else {
            // No graph_id provided, prompt for selection
            Self::prompt_graph_selection(all_graph_options)?
        };

        // Display project context and requirements
        println!();
        println!(
            "️{}",
            Style::File.paint("▲ Add AI capabilities to existing graph (~5 minute setup time)")
        );
        println!("Enhance an Apollo GraphOS graph with MCP server capabilities.");
        println!();
        println!(
            "{} Your data source (API endpoint, database, or service)",
            Style::Heading.paint("Requirements:")
        );
        println!();

        // For existing graphs, we work with GraphQL schemas (no template selection needed)

        // Fetch graph schemas from GraphOS
        println!(
            "{}",
            Style::Heading.paint("Pulling graph schemas from GraphOS")
        );

        let graph_ref = GraphRef::new(
            selected_graph.graph_id.clone(),
            Some(selected_graph.variant_name.clone()),
        )?;

        // Fetch supergraph schema
        let supergraph_sdl = match fetch_supergraph::run(
            SupergraphFetchInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await
        {
            Ok(response) => response.sdl.contents,
            Err(e) => {
                eprintln!(
                    "{}",
                    Style::WarningHeading
                        .paint(format!("▲ Could not fetch supergraph schema: {}", e))
                );
                // Continue without the schema - MCP can still work with just the graph reference
                String::new()
            }
        };

        // Fetch subgraph information
        let subgraph_info = match list_subgraphs::run(
            SubgraphListInput {
                graph_ref: graph_ref.clone(),
            },
            client,
        )
        .await
        {
            Ok(response) => {
                let subgraph_names: Vec<String> =
                    response.subgraphs.iter().map(|s| s.name.clone()).collect();
                if !subgraph_names.is_empty() {
                    format!("Subgraphs: {}", subgraph_names.join(", "))
                } else {
                    String::new()
                }
            }
            Err(_) => String::new(),
        };

        // Step 5: Use selected graph info for template replacement
        let project_name = selected_graph.graph_id.clone();
        // Docker requires lowercase image names without spaces or special characters
        let docker_tag = helpers::normalize_docker_tag(&selected_graph.graph_id);
        let graph_ref_str = format!(
            "{}@{}",
            selected_graph.graph_id, selected_graph.variant_name
        );
        let graph_endpoint = format!(
            "https://studio.apollographql.com/graph/{}/explorer",
            selected_graph.graph_id
        );

        // Get current directory
        let current_dir = match &self.path {
            Some(path) => camino::Utf8PathBuf::try_from(path.clone())
                .map_err(|_| RoverError::new(anyhow!("Invalid path")))?,
            None => camino::Utf8PathBuf::from("."),
        };

        // Fetch raw files from the add-mcp directory
        let branch_ref = "release/v3";
        let mut template_fetcher = InitTemplateFetcher::new();
        let template_options = template_fetcher.call(branch_ref).await?;

        // Extract files directly from the add-mcp directory (no examples to remove)
        let mut files = template_options.extract_directory_files("add-mcp")?;

        // If we have a supergraph schema, save it for reference
        if !supergraph_sdl.is_empty() {
            files.insert("supergraph.graphql".into(), supergraph_sdl.clone());
        }

        // Remove supergraph.yaml for existing graphs since we use --graph-ref instead of --supergraph-config
        files.remove(&camino::Utf8PathBuf::from("supergraph.yaml"));

        // Add a basic README with graph info
        let readme_content = format!(
            r#"# {} MCP Server

This MCP server provides AI-accessible tools for your Apollo graph.

## Graph Information
- **Graph ID**: {}
- **Variant**: {}
- **Organization**: {}
- **Graph Reference**: {}
{}

## Quick Start

1. Build the MCP server:
   ```bash
   docker build -f mcp.Dockerfile -t {}-mcp .
   ```

2. Run the MCP server:
   ```bash
   docker run --env-file .env -p5050:5050 {}-mcp
   # Linux users may need: docker run --network=host --env-file .env {}-mcp
   ```

3. Test with MCP Inspector:
   ```bash
   npx @mcp/inspector
   ```

4. Configure Claude Desktop to use this server.

## View in Apollo Studio
{}"#,
            selected_graph.graph_name,
            selected_graph.graph_id,
            selected_graph.variant_name,
            selected_graph.organization_name,
            graph_ref_str,
            if !subgraph_info.is_empty() {
                format!("\n- **{}**", subgraph_info)
            } else {
                String::new()
            },
            docker_tag,
            docker_tag,
            docker_tag,
            graph_endpoint
        );
        files.insert("README.md".into(), readme_content);

        // Get or create Apollo service key
        // First check if APOLLO_KEY is already set in environment
        let apollo_key = if let Ok(key) = std::env::var("APOLLO_KEY") {
            if key.starts_with("service:") {
                println!(
                    "{}",
                    Style::Success.paint("✓ Using existing APOLLO_KEY from environment")
                );
                key
            } else {
                // Need to create a new service key for this graph
                println!("{}", Style::Heading.paint("Creating service API key"));

                // Use the operations module to create API key
                use crate::command::init::operations::create_api_key;
                create_api_key(
                    client_config,
                    &self.profile,
                    selected_graph.graph_id.clone(),
                    format!("{}-mcp-server", selected_graph.graph_name),
                )
                .await?
            }
        } else {
            // Create new service key
            println!("{}", Style::Heading.paint("Creating service API key"));

            use crate::command::init::operations::create_api_key;
            create_api_key(
                client_config,
                &self.profile,
                selected_graph.graph_id.clone(),
                format!("{}-mcp-server", selected_graph.graph_name),
            )
            .await?
        };

        // Preview files and get user confirmation before creating them
        let confirmed = self.preview_mcp_files(&selected_graph, &files).await?;
        if !confirmed {
            println!("Setup cancelled.");
            return Ok(RoverOutput::EmptySuccess);
        }

        // Get the user's home directory for MCP server binary path
        let home_dir = if cfg!(windows) {
            std::env::var("USERPROFILE")
                .or_else(|_| {
                    std::env::var("HOMEDRIVE").and_then(|drive| {
                        std::env::var("HOMEPATH").map(|path| format!("{}{}", drive, path))
                    })
                })
                .map_err(|_| anyhow!("Could not determine home directory on Windows"))
        } else {
            std::env::var("HOME").map_err(|_| {
                anyhow!("Could not determine home directory from HOME environment variable")
            })
        }?;
        let home_dir = camino::Utf8PathBuf::from(home_dir);

        // Generate the MCP server binary path
        let mcp_server_binary = home_dir.join(".rover/bin/apollo-mcp-server-v0.8.0");

        // Generate the MCP config path (relative to project)
        let mcp_config_path = current_dir.join(".apollo/mcp.claude.yaml");

        // Generate the absolute tools path for MCP server
        let tools_absolute_path = current_dir.join("tools");
        let tools_path_str = tools_absolute_path.as_str().replace("/./", "/");

        // Write files to current directory with template replacement
        for (file_path, content) in &files {
            let final_file_path = camino::Utf8PathBuf::from(file_path);

            let target_path = current_dir.join(&final_file_path);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Replace template placeholders with selected graph information
            // Use ${} format for YAML files (avoids linting issues)
            // Use {{}} format for other templates and conditionals
            let mut processed_content = content
                // ${} format - primarily for YAML files
                .replace("${PROJECT_NAME}", &project_name)
                .replace("${GRAPH_REF}", &graph_ref_str)
                .replace("${GRAPH_ID}", &selected_graph.graph_id)
                .replace("${GRAPH_NAME}", &selected_graph.graph_name)
                .replace("${VARIANT_NAME}", &selected_graph.variant_name)
                .replace("${ORGANIZATION_NAME}", &selected_graph.organization_name)
                .replace("${APOLLO_API_KEY}", &apollo_key)
                .replace("${APOLLO_KEY}", &apollo_key)
                .replace("${APOLLO_GRAPH_REF}", &graph_ref_str)
                .replace("${GRAPHQL_ENDPOINT}", "http://localhost:4000")
                .replace("${STAGING_GRAPHQL_ENDPOINT}", "http://localhost:4000") // For staging YAML
                .replace("${GRAPH_STUDIO_URL}", &graph_endpoint)
                .replace("${PROJECT_VERSION}", "1.0.0")
                .replace(
                    "${PROJECT_REPOSITORY_URL}",
                    &format!("https://github.com/user/{}", project_name),
                )
                // {{}} format - for non-YAML templates and backwards compatibility
                .replace("{{PROJECT_NAME}}", &project_name)
                .replace("{{GRAPH_REF}}", &graph_ref_str)
                .replace("{{GRAPH_ID}}", &selected_graph.graph_id)
                .replace("{{GRAPH_NAME}}", &selected_graph.graph_name)
                .replace("{{VARIANT_NAME}}", &selected_graph.variant_name)
                .replace("{{ORGANIZATION_NAME}}", &selected_graph.organization_name)
                .replace("{{APOLLO_API_KEY}}", &apollo_key)
                .replace("{{APOLLO_KEY}}", &apollo_key)
                .replace("{{APOLLO_GRAPH_REF}}", &graph_ref_str)
                .replace("{{MCP_SERVER_BINARY}}", mcp_server_binary.as_str())
                .replace("{{MCP_CONFIG_PATH}}", mcp_config_path.as_str())
                .replace("{{GRAPHQL_ENDPOINT}}", "http://localhost:4000")
                .replace("{{GRAPH_STUDIO_URL}}", &graph_endpoint)
                .replace("{{PROJECT_VERSION}}", "1.0.0")
                .replace(
                    "{{PROJECT_REPOSITORY_URL}}",
                    &format!("https://github.com/user/{}", project_name),
                )
                // Other replacements
                .replace("- /tools", &format!("- {}", tools_path_str))
                .replace(
                    "endpoint: http://host.docker.internal:4000",
                    "endpoint: http://localhost:4000",
                );

            // Handle the specific case where .env.template needs dynamic values populated
            if file_path == ".env.template" {
                // Only replace the specific placeholders mentioned by user, leave others untouched
                processed_content = processed_content
                    .replace("\"{{PROJECT_NAME}}\"", &format!("\"{}\"", project_name))
                    .replace(
                        "\"{{GRAPHQL_ENDPOINT}}\"",
                        "\"http://host.docker.internal:4000/graphql\"",
                    )
                    .replace("\"{{APOLLO_KEY}}\"", &format!("\"{}\"", apollo_key))
                    .replace(
                        "\"{{APOLLO_GRAPH_REF}}\"",
                        &format!("\"{}\"", graph_ref_str),
                    );

                // Write processed template as .env file instead of .env.template
                let env_path = current_dir.join(".env");
                std::fs::write(&env_path, processed_content)?;
                continue; // Skip writing .env.template
            }

            std::fs::write(&target_path, processed_content)?;
        }

        println!();
        println!(
            "{}",
            Style::Success.paint("MCP server added to your project!")
        );

        println!("\n{}", Style::Heading.paint("Generated files:"));
        let tool_files: Vec<String> = files
            .keys()
            .filter(|k| k.starts_with("tools/") && k.ends_with(".graphql"))
            .map(|k| format!("   - {}", k.strip_prefix("tools/").unwrap_or(k)))
            .collect();
        for tool in tool_files {
            println!("{}", tool);
        }

        println!("\n{}", Style::Heading.paint("Selected Graph:"));
        println!(
            "  • Graph: {} ({})",
            selected_graph.graph_name, selected_graph.variant_name
        );
        println!("  • Organization: {}", selected_graph.organization_name);
        println!("  • Graph Reference: {}", graph_ref_str);
        if !supergraph_sdl.is_empty() {
            println!("  • Supergraph schema: Downloaded");
        }
        println!("  • Service API key: {}", apollo_key);
        println!("  • ✓ Credentials saved to .env file");

        println!("\n{}", Style::Heading.paint("Next steps:"));
        println!("   1. Start the MCP server:");
        println!();
        println!("      {}: {}", Style::Heading.paint("Linux/macOS"), Style::Command.paint(format!("set -a && source .env && set +a && rover dev --graph-ref {} --mcp .apollo/mcp.local.yaml", graph_ref_str)));
        println!();
        println!("      {}: ", Style::Heading.paint("Windows PowerShell"));
        println!("      {}", Style::Command.paint("Get-Content .env | ForEach-Object { $name, $value = $_.split('=',2); [System.Environment]::SetEnvironmentVariable($name, $value) }"));
        println!(
            "      {}",
            Style::Command.paint(format!(
                "rover dev --graph-ref {} --mcp .apollo/mcp.local.yaml",
                graph_ref_str
            ))
        );
        println!();
        println!(
            "      → API: {} | MCP: {}",
            Style::Link.paint("http://localhost:4000"),
            Style::Link.paint("http://localhost:5050")
        );
        println!();
        println!("   2. For containerized deployment:");
        println!(
            "      {}",
            Style::Command.paint(format!(
                "docker build -f mcp.Dockerfile -t {}-mcp .",
                docker_tag
            ))
        );
        println!(
            "      {}",
            Style::Command.paint(format!(
                "docker run --env-file .env -p5050:5050 {}-mcp",
                docker_tag
            ))
        );

        Ok(RoverOutput::EmptySuccess)
    }

    /// Get MCP data source type from project_use_case argument or prompt user
    #[cfg(feature = "composition-js")]
    fn get_or_prompt_mcp_data_source(&self) -> RoverResult<MCPDataSourceType> {
        // Check if project_use_case was provided via command line
        if let Some(use_case) = &self.project_use_case.project_use_case {
            let data_source = match use_case {
                ProjectUseCase::Connectors => MCPDataSourceType::ExternalAPIs,
                ProjectUseCase::GraphQLTemplate => MCPDataSourceType::GraphQLAPI,
            };
            return Ok(data_source);
        }

        // If no argument provided, prompt the user
        Self::prompt_mcp_data_source()
    }

    /// Prompt user to select MCP data source type
    #[cfg(feature = "composition-js")]
    fn prompt_mcp_data_source() -> RoverResult<MCPDataSourceType> {
        use anyhow::anyhow;
        use dialoguer::Select;
        use dialoguer::console::Term;
        use rover_std::Style;

        // Display project type and description
        println!();
        println!(
            "️{}",
            Style::File.paint("▲ AI-powered Apollo graph with MCP server ~10 minute setup time")
        );
        println!(
            "Build an Apollo GraphOS graph with MCP server capabilities. Start with a working template and connect your own APIs and data sources."
        );
        println!();
        println!(
            "{} Your data source (API endpoint, database, or service)",
            Style::Heading.paint("Requirements:")
        );
        println!();

        let options = vec![
            MCPDataSourceType::ExternalAPIs,
            MCPDataSourceType::GraphQLAPI,
        ];

        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? Select a starting template"))
            .items(&options)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(options[index].clone()),
            None => Err(RoverError::new(anyhow!("Selection cancelled"))),
        }
    }

    /// Display MCP-specific success message
    #[cfg(feature = "composition-js")]
    fn display_mcp_project_success(
        completed_project: &states::ProjectCreated,
        _mcp_project_type: &MCPProjectType,
        mcp_result: &crate::command::init::mcp::mcp_operations::MCPSetupResult,
    ) {
        use rover_std::Style;

        println!("{}", Style::Success.paint("✓ MCP server generated"));
        println!(
            "{}",
            Style::Success.paint("✓ Credentials saved to .env file")
        );
        if let Some(claude_config_path) = &mcp_result.claude_config {
            println!(
                "{}",
                Style::Success.paint(format!(
                    "✓ Claude Desktop config generated: {}",
                    claude_config_path
                ))
            );
        }

        // Project Details section
        println!();
        println!("{}", Style::File.paint("Project details"));
        println!(
            "   • MCP Server Name: mcp-{}",
            completed_project.config.project_name
        );
        println!(
            "   • GraphOS Organization: {}",
            completed_project.config.organization
        );
        println!();

        println!(
            "{}",
            Style::File.paint("GraphOS credentials for your graph")
        );
        println!(
            "   • {}: {}",
            Style::GraphRef.paint("APOLLO_GRAPH_REF"),
            completed_project.graph_ref
        );
        println!(
            "   • {}: {} (This is your graph's API key)", // gitleaks:allow
            Style::Command.paint("APOLLO_KEY"),
            completed_project.api_key
        );
        println!();

        // Next Steps section
        println!();
        println!("{}", Style::File.paint("Next steps ↴"));

        println!();
        println!("1. Configure Claude Desktop by copying claude_desktop_config.json to:");
        println!(
            "   • macOS: {}",
            Style::Path.paint("~/Library/Application Support/Claude/claude_desktop_config.json")
        );
        println!(
            "   • Windows: {}",
            Style::Path.paint("%APPDATA%\\Claude\\claude_desktop_config.json")
        );
        println!(
            "   • Linux: {}",
            Style::Path.paint("~/.config/Claude/claude_desktop_config.json")
        );
        println!();
        println!("   Then restart Claude Desktop.");

        println!();
        println!("2. Start MCP server (after graph creation):");
        println!();
        println!("   {}: {}", Style::Heading.paint("Linux/macOS"), Style::Command.paint("set -a && source .env && set +a && rover dev --supergraph-config supergraph.yaml --mcp .apollo/mcp.local.yaml"));
        println!();
        println!("   {}: ", Style::Heading.paint("Windows PowerShell"));
        println!("   {}", Style::Command.paint("Get-Content .env | ForEach-Object { $name, $value = $_.split('=',2); [System.Environment]::SetEnvironmentVariable($name, $value) }"));
        println!(
            "   {}",
            Style::Command.paint(
                "rover dev --supergraph-config supergraph.yaml --mcp .apollo/mcp.local.yaml"
            )
        );
        println!();
        println!(
            "   → API: {} | MCP: {}",
            Style::Link.paint("http://localhost:4000"),
            Style::Link.paint("http://localhost:5050")
        );

        println!();
        println!("3. Try it out in Claude:");
        println!(
            "   Ask \"What tools do I have available?\" or \"Can you get me some product information?\""
        );

        println!();
        println!("Next steps:");
        println!("- Customize endpoints → See the schema.graphql file");
        println!(
            "- Create tools → Studio's Sandbox Explorer: {}",
            Style::Link.paint("http://localhost:4000")
        );
        println!(
            "- Deploy → {}",
            Style::Command.paint("rover docs open mcp-deploy")
        );
        println!(
            "- Learn more → {}",
            Style::Command.paint("rover docs open mcp-qs")
        );
    }

    /// Handle MCP setup for new project creation
    #[cfg(feature = "composition-js")]
    async fn handle_new_project_mcp(
        &self,
        _client: &rover_client::blocking::StudioClient,
        client_config: &StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        use crate::command::init::options::{ProjectType, ProjectUseCase};
        use crate::command::init::states::{
            ProjectTypeSelected, TemplateSelected, UseCaseSelected, UserAuthenticated,
        };
        use crate::command::init::transitions::CreateProjectResult;
        use anyhow::anyhow;
        use rover_client::shared::GraphRef;

        // Determine data source type from project_use_case argument or prompt
        let data_source_type = self.get_or_prompt_mcp_data_source()?;

        // Authenticate
        let _welcome = UserAuthenticated::new()
            .check_authentication(client_config, &self.profile)
            .await?;

        // Skip project type selection since we know this is a new project
        let project_type_selected = ProjectTypeSelected {
            output_path: match &self.path {
                Some(path) => camino::Utf8PathBuf::try_from(path.clone())
                    .map_err(|_| RoverError::new(anyhow!("Invalid path")))?,
                None => camino::Utf8PathBuf::from("."),
            },
            project_type: ProjectType::CreateNew,
        };

        // Go through organization selection
        let organization_selected = project_type_selected
            .select_organization(&self.organization, &self.profile, client_config)
            .await?;

        // Create use case selected state based on data source type
        let (use_case, base_template_id, mcp_project_type) = match data_source_type {
            MCPDataSourceType::ExternalAPIs => {
                // For External APIs, use start-with-rest + add-mcp
                (
                    ProjectUseCase::Connectors,
                    "connectors",
                    MCPProjectType::Rest,
                )
            }
            MCPDataSourceType::GraphQLAPI => {
                // For GraphQL APIs, use start-with-typescript + add-mcp
                (
                    ProjectUseCase::GraphQLTemplate,
                    "typescript",
                    MCPProjectType::GraphQL,
                )
            }
        };

        let use_case_selected = UseCaseSelected {
            output_path: organization_selected.output_path,
            project_type: organization_selected.project_type,
            organization: organization_selected.organization,
            use_case,
        };

        // Fetch base template + add-mcp using the existing fetch_mcp_template method
        let branch_ref = "release/v3";
        let mut template_fetcher = InitTemplateFetcher::new();
        let mut selected_template = template_fetcher
            .fetch_mcp_template(base_template_id, branch_ref)
            .await?;

        // Filter files based on data source selection BEFORE preview
        let mut string_files: std::collections::HashMap<camino::Utf8PathBuf, String> =
            selected_template
                .files
                .iter()
                .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).to_string()))
                .collect();

        // Remove tools and examples since all MCP configs use operation collections
        let files_to_remove: Vec<_> = string_files
            .keys()
            .filter(|path| path.starts_with("examples/") || path.starts_with("tools/"))
            .cloned()
            .collect();

        for path in files_to_remove {
            string_files.remove(&path);
        }

        // Convert back to bytes (template replacement will happen later after project creation)
        selected_template.files = string_files
            .into_iter()
            .map(|(path, content)| (path, content.into_bytes()))
            .collect();

        let template_selected = TemplateSelected {
            output_path: use_case_selected.output_path,
            project_type: use_case_selected.project_type,
            organization: use_case_selected.organization,
            use_case: use_case_selected.use_case,
            selected_template,
        };

        // Continue with the normal naming flow, but skip the generic preview for MCP (we have our own)
        let graph_id_entered = template_selected
            .enter_project_name(&self.project_name)?
            .confirm_graph_id(&self.graph_id)?;

        // NOW apply template placeholder replacement after we have the project name
        // Convert files back to string format for processing
        let mut string_files: std::collections::HashMap<camino::Utf8PathBuf, String> =
            graph_id_entered
                .selected_template
                .files
                .iter()
                .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).to_string()))
                .collect();

        // Get the actual project name that will be used
        let project_name = &graph_id_entered.project_name.to_string();
        // Docker requires lowercase image names without spaces or special characters
        let docker_tag = helpers::normalize_docker_tag(project_name);

        // Get the user's home directory for MCP server binary path
        let home_dir = if cfg!(windows) {
            std::env::var("USERPROFILE")
                .or_else(|_| {
                    std::env::var("HOMEDRIVE").and_then(|drive| {
                        std::env::var("HOMEPATH").map(|path| format!("{}{}", drive, path))
                    })
                })
                .map_err(|_| anyhow!("Could not determine home directory on Windows"))
        } else {
            std::env::var("HOME").map_err(|_| {
                anyhow!("Could not determine home directory from HOME environment variable")
            })
        }?;
        let home_dir = camino::Utf8PathBuf::from(home_dir);

        // Generate the MCP server binary path
        let mcp_server_binary = home_dir.join(".rover/bin/apollo-mcp-server-v0.8.0");

        // Get the output path for the project
        let output_path = graph_id_entered.output_path.clone();
        let absolute_output_path = if output_path.is_relative() {
            std::env::current_dir()
                .ok()
                .and_then(|cwd| camino::Utf8PathBuf::try_from(cwd).ok())
                .map(|cwd| cwd.join(&output_path))
                .unwrap_or_else(|| output_path.clone())
        } else {
            output_path.clone()
        };

        // Generate the MCP config path (relative to project)
        let mcp_config_path = absolute_output_path.join(".apollo/mcp.claude.yaml");

        // Generate the absolute tools path for MCP server
        let tools_absolute_path = absolute_output_path.join("tools");
        let tools_path_str = tools_absolute_path.as_str().replace("/./", "/");

        // Apply template placeholder replacement (was missing in new project flow!)
        // Use placeholder values for new projects since we don't have a real graph yet
        let graph_ref = GraphRef::new(
            graph_id_entered.graph_id.clone().to_string(),
            Some(DEFAULT_VARIANT.to_string()),
        )?;

        for (_file_path, content) in string_files.iter_mut() {
            // Replace template placeholders - use both formats for compatibility
            // ${} format for YAML files (avoids linting issues)
            // {{}} format for other templates and conditionals
            *content = content
                // ${} format - primarily for YAML files
                .replace("${PROJECT_NAME}", project_name)
                .replace("${DOCKER_TAG}", &docker_tag)
                .replace("${GRAPH_REF}", &graph_ref.to_string())
                .replace("${GRAPH_ID}", &graph_ref.name)
                .replace("${GRAPH_NAME}", project_name)
                .replace("${VARIANT_NAME}", &graph_ref.variant)
                .replace("${ORGANIZATION_NAME}", "YOUR_ORGANIZATION")
                .replace("${APOLLO_GRAPH_REF}", &graph_ref.to_string())
                .replace("${GRAPHQL_ENDPOINT}", "http://localhost:4000")
                .replace("${STAGING_GRAPHQL_ENDPOINT}", "http://localhost:4000") // For staging YAML
                .replace(
                    "${GRAPH_STUDIO_URL}",
                    &format!(
                        "https://studio.apollographql.com/graph/{}/explorer",
                        project_name
                    ),
                )
                .replace("${PROJECT_VERSION}", "1.0.0")
                .replace(
                    "${PROJECT_REPOSITORY_URL}",
                    &format!("https://github.com/user/{}", project_name),
                )
                // {{}} format - for non-YAML templates and backwards compatibility
                .replace("{{PROJECT_NAME}}", project_name)
                .replace("{{DOCKER_TAG}}", &docker_tag)
                .replace("{{GRAPH_REF}}", &graph_ref.to_string())
                .replace("{{GRAPH_ID}}", &graph_ref.name)
                .replace("{{GRAPH_NAME}}", project_name)
                .replace("{{VARIANT_NAME}}", &graph_ref.variant)
                .replace("{{ORGANIZATION_NAME}}", "YOUR_ORGANIZATION") // Placeholder since org structure is complex
                .replace("{{APOLLO_GRAPH_REF}}", &graph_ref.to_string())
                .replace("{{MCP_SERVER_BINARY}}", mcp_server_binary.as_str())
                .replace("{{MCP_CONFIG_PATH}}", mcp_config_path.as_str())
                .replace("{{GRAPHQL_ENDPOINT}}", "http://localhost:4000")
                .replace(
                    "{{GRAPH_STUDIO_URL}}",
                    &format!(
                        "https://studio.apollographql.com/graph/{}/explorer",
                        project_name
                    ),
                )
                .replace("{{PROJECT_VERSION}}", "1.0.0")
                .replace(
                    "{{PROJECT_REPOSITORY_URL}}",
                    &format!("https://github.com/user/{}", project_name),
                )
                // Other replacements
                .replace("- /tools", &format!("- {}", tools_path_str))
                .replace(
                    "endpoint: http://host.docker.internal:4000",
                    "endpoint: http://localhost:4000",
                );

            // Handle REST_CONNECTORS placeholder based on data source type (simple approach)
            let rest_connectors_value =
                if matches!(data_source_type, MCPDataSourceType::ExternalAPIs) {
                    "true"
                } else {
                    "false"
                };
            *content = content.replace("{{REST_CONNECTORS}}", rest_connectors_value);

            // Simple handling of conditional blocks - just remove the template syntax
            *content = content
                .replace("{{#if REST_CONNECTORS}}", "")
                .replace("{{else}}", "")
                .replace("{{/if}}", "");

            // Fix Docker image tags to be lowercase and handle spaces
            // Replace any docker build/run commands that use project_name with lowercase version
            *content = content
                .replace(
                    &format!("-t {}", project_name),
                    &format!("-t {}", docker_tag),
                )
                .replace(
                    &format!("-t {}-mcp", project_name),
                    &format!("-t {}-mcp", docker_tag),
                )
                .replace(
                    &format!("p5050:5050 {}", project_name),
                    &format!("p5050:5050 {}", docker_tag),
                )
                .replace(
                    &format!("p5050:5050 {}-mcp", project_name),
                    &format!("p5050:5050 {}-mcp", docker_tag),
                )
                .replace(
                    &format!("--env-file .env {}", project_name),
                    &format!("--env-file .env {}", docker_tag),
                )
                .replace(
                    &format!("--network=host --env-file .env {}", project_name),
                    &format!("--network=host --env-file .env {}", docker_tag),
                )
                .replace(
                    &format!("--network=host --env-file .env {}-mcp", project_name),
                    &format!("--network=host --env-file .env {}-mcp", docker_tag),
                );
        }

        // Convert back to bytes and update the template, excluding .env.template
        let mut updated_graph_id_entered = graph_id_entered;
        updated_graph_id_entered.selected_template.files = string_files
            .into_iter()
            .filter(|(path, _)| path != ".env.template") // Skip .env.template files
            .map(|(path, content)| (path, content.into_bytes()))
            .collect();

        let graph_id_entered = updated_graph_id_entered;

        // Use regular preview for the deprecated MCP method
        let creation_confirmed = match graph_id_entered.preview_and_confirm_creation().await? {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        // Add MCP-specific preview before project creation
        let mcp_confirmed = self
            .preview_mcp_new_project_files(
                &creation_confirmed.config.project_name.to_string(),
                &creation_confirmed.selected_template.files,
            )
            .await?;

        if !mcp_confirmed {
            println!("Setup cancelled.");
            return Ok(RoverOutput::EmptySuccess);
        }

        let project_created = creation_confirmed
            .create_project(client_config, &self.profile)
            .await?;

        // Handle the project creation result
        let completed_project = match project_created {
            CreateProjectResult::Created(project) => project,
            CreateProjectResult::Restart {
                reason: _,
                state: _,
            } => {
                // For simplicity in v1, don't handle restarts for MCP flow
                return Err(RoverError::new(anyhow!(
                    "Project creation requires restart - please try again"
                )));
            }
        };

        let output_path = match &self.path {
            Some(path) => camino::Utf8PathBuf::try_from(path.clone())
                .map_err(|_| RoverError::new(anyhow!("Invalid path")))?,
            None => camino::Utf8PathBuf::from("."),
        };

        // Create .env file with actual credentials
        let env_content = format!(
            "APOLLO_KEY={}\nAPOLLO_GRAPH_REF={}\n",
            completed_project.api_key, completed_project.graph_ref
        );
        let env_path = output_path.join(".env");
        std::fs::write(&env_path, env_content)?;

        // Generate Claude Desktop config with real API key and graph ref
        use crate::command::init::mcp::mcp_operations::MCPOperations;
        let mcp_result = MCPOperations::setup_mcp_project_with_name(
            &output_path,
            &completed_project.api_key,
            &completed_project.graph_ref.to_string(),
            Some(&completed_project.config.project_name.to_string()),
        )?;

        update_template_files_with_real_values(&completed_project)?;

        // Display MCP-specific success message instead of standard completion
        Self::display_mcp_project_success(&completed_project, &mcp_project_type, &mcp_result);

        Ok(RoverOutput::EmptySuccess)
    }

    #[cfg(not(feature = "composition-js"))]
    pub async fn run(&self, _client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        use crate::RoverError;
        use crate::RoverErrorSuggestion;
        use anyhow::anyhow;
        use rover_std::hyperlink;

        let mut err = RoverError::new(anyhow!(
            "This version of Rover does not support this command."
        ));
        if cfg!(target_env = "musl") {
            err.set_suggestion(RoverErrorSuggestion::Adhoc(format!("Unfortunately, Deno does not currently support musl architectures. You can follow along with this issue for updates on musl support: {}, for now you will need to switch to a Linux distribution (like Ubuntu or CentOS) that can run Rover's prebuilt binaries.", hyperlink("https://github.com/denoland/deno/issues/3711"))));
        }

        Err(err)
    }
}
