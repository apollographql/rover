#[cfg(feature = "composition-js")]
mod authentication;
#[cfg(feature = "composition-js")]
mod config;
#[cfg(feature = "composition-js")]
mod graph_id;
#[cfg(feature = "composition-js")]
mod helpers;
#[cfg(feature = "composition-js")]
mod mcp_operations;
#[cfg(feature = "composition-js")]
mod mcp_augmentation;
#[cfg(feature = "composition-js")]
mod operations;
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
use crate::command::init::options::ProjectTemplateOpt;
#[cfg(feature = "composition-js")]
use crate::command::init::options::{
    GraphIdOpt, ProjectNameOpt, ProjectOrganizationOpt, ProjectType, ProjectTypeOpt,
    ProjectUseCaseOpt,
};
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
            MCPSetupType::ExistingGraph => write!(f, "Add MCP capabilities to an existing Apollo project"),
            MCPSetupType::NewProject => write!(f, "Create a new MCP server project"),
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
enum MCPProjectType {
    REST,
    GraphQL,
}

#[cfg(feature = "composition-js")]
impl std::fmt::Display for MCPProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPProjectType::REST => write!(f, "MCP server for REST APIs (make existing REST services AI-accessible)"),
            MCPProjectType::GraphQL => write!(f, "MCP server for GraphQL APIs (make GraphQL services AI-accessible)"),
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
enum MCPDataSourceType {
    ExternalAPIs,    // REST, webhooks, SaaS
    AWSServices,     // Lambda, DynamoDB
    GraphQLAPI,      // Existing GraphQL endpoints
}

#[cfg(feature = "composition-js")]
impl std::fmt::Display for MCPDataSourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MCPDataSourceType::ExternalAPIs => write!(f, "External APIs (REST, webhooks, SaaS tools)"),
            MCPDataSourceType::AWSServices => write!(f, "AWS services (Lambda, DynamoDB, etc.)"),
            MCPDataSourceType::GraphQLAPI => write!(f, "Existing GraphQL API"),
        }
    }
}

#[cfg(feature = "composition-js")]
#[derive(Clone, Debug)]
struct GraphVariantOption {
    organization_name: String,
    organization_id: String,
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
        use crate::command::init::states::UserAuthenticated;
        use helpers::display_use_template_message;
        
        // Handle MCP augmentation as special case - skip all project creation flow
        if self.project_template.mcp {
            return self.handle_mcp_augmentation(&client_config).await;
        }
        
        let welcome = UserAuthenticated::new()
            .check_authentication(&client_config, &self.profile)
            .await?;

        let project_type_selected = welcome.select_project_type(&self.project_type, &self.path, &self.project_template)?;

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
                    let suggestion = RoverErrorSuggestion::Adhoc(
                        format!(
                            "If the issue persists, please contact support at {}.",
                            hyperlink("https://support.apollographql.com")
                        )
                        .to_string(),
                    );
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
                            welcome.select_project_type(&self.project_type, &self.path, &self.project_template)?;
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
    
    /// Handle MCP augmentation directly without going through project creation flow
    #[cfg(feature = "composition-js")]
    async fn handle_mcp_augmentation(&self, client_config: &StudioClientConfig) -> RoverResult<RoverOutput> {
        use rover_std::Style;
        use crate::command::init::authentication::{AuthenticationError, auth_error_to_rover_error};
        use std::env;
        use anyhow::anyhow;
        
        println!("{}", Style::Heading.paint("🚀 Adding MCP server capabilities..."));
        
        // Validate that directory is empty
        let current_dir = env::current_dir()?;
        let output_path = match &self.path {
            Some(path) => camino::Utf8PathBuf::try_from(path.clone())
                .map_err(|_| anyhow!("Invalid path"))?,
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
                        "Please run `{}` in an empty directory",
                        Style::Command.paint("rover init --mcp")
                    )
                    .to_string(),
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
        
        // Prompt for MCP setup type
        let setup_type = Self::prompt_mcp_setup_type()?;
        
        match setup_type {
            MCPSetupType::ExistingGraph => {
                self.handle_existing_graph_mcp(&client, client_config).await
            }
            MCPSetupType::NewProject => {
                self.handle_new_project_mcp(&client, client_config).await
            }
        }
    }
    
    /// Prompt user to choose MCP setup type
    #[cfg(feature = "composition-js")]
    fn prompt_mcp_setup_type() -> RoverResult<MCPSetupType> {
        use dialoguer::Select;
        use dialoguer::console::Term;
        use anyhow::anyhow;
        
        let options = vec![
            MCPSetupType::ExistingGraph,
            MCPSetupType::NewProject,
        ];
        
        let names = options
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>();
            
        let selection = Select::new()
            .with_prompt("How would you like to set up your MCP server?")
            .items(&names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(options[index].clone()),
            None => Err(RoverError::new(anyhow!("Selection cancelled"))),
        }
    }
    
    /// Prompt user to select a graph variant
    #[cfg(feature = "composition-js")]
    fn prompt_graph_selection(graph_options: Vec<GraphVariantOption>) -> RoverResult<GraphVariantOption> {
        use dialoguer::Select;
        use dialoguer::console::Term;
        use anyhow::anyhow;
        
        let display_names = graph_options
            .iter()
            .map(|option| option.display_name.clone())
            .collect::<Vec<_>>();
            
        let selection = Select::new()
            .with_prompt("Which graph would you like to add MCP server capabilities to?")
            .items(&display_names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(graph_options[index].clone()),
            None => Err(RoverError::new(anyhow!("Graph selection cancelled"))),
        }
    }
    
    /// Handle MCP setup for existing graph
    #[cfg(feature = "composition-js")]
    async fn handle_existing_graph_mcp(&self, client: &rover_client::blocking::StudioClient, client_config: &StudioClientConfig) -> RoverResult<RoverOutput> {
        use rover_std::Style;
        use crate::command::init::authentication::{AuthenticationError, auth_error_to_rover_error};
        use rover_client::operations::init::{memberships, list_graphs};
        use rover_client::operations::supergraph::fetch::{run as fetch_supergraph, SupergraphFetchInput};
        use rover_client::operations::subgraph::list::{run as list_subgraphs, SubgraphListInput};
        use rover_client::shared::GraphRef;
        use rover_client::RoverClientError;
        use anyhow::anyhow;
        
        println!("{}", Style::Heading.paint("🔍 Querying your Apollo graphs..."));
        
        // Step 2: Query GraphOS for user's organizations and their graphs
        let memberships_response = memberships::run(&client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(msg))
            }
            e => e.into(),
        })?;

        if memberships_response.memberships.is_empty() {
            println!("{}", Style::WarningHeading.paint("❌ No organizations found"));
            println!("You need to create a graph first. Visit https://studio.apollographql.com to create your first graph.");
            return Ok(RoverOutput::EmptySuccess);
        }

        // Collect all graphs from all organizations
        let mut all_graph_options = Vec::new();
        
        for org in &memberships_response.memberships {
            println!("{} Fetching graphs for organization: {}", Style::Heading.paint("ℹ"), org.name);
            
            let list_graphs_response = list_graphs::run(
                list_graphs::ListGraphsInput {
                    organization_id: org.id.clone(),
                },
                &client,
            ).await.map_err(|e| {
                RoverError::new(anyhow!("Failed to fetch graphs for organization {}: {}", org.name, e))
            })?;

            for graph in list_graphs_response.organization.graphs {
                for variant in graph.variants {
                    all_graph_options.push(GraphVariantOption {
                        organization_name: org.name.clone(),
                        organization_id: org.id.clone(),
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
            println!("{}", Style::WarningHeading.paint("❌ No graphs found"));
            println!("You need to create a graph first. Visit https://studio.apollographql.com to create your first graph.");
            return Ok(RoverOutput::EmptySuccess);
        }

        // Step 3: Present graph selection dropdown
        println!("{}", Style::Success.paint(&format!("✅ Found {} graph variants", all_graph_options.len())));
        
        let selected_graph = Self::prompt_graph_selection(all_graph_options)?;
        
        // Step 3.5: Prompt for data source type
        let data_source_type = Self::prompt_mcp_data_source()?;
        
        // Step 4: Fetch graph schemas from GraphOS
        println!("{}", Style::Heading.paint("📥 Pulling graph schemas from GraphOS..."));
        
        let graph_ref = GraphRef::new(selected_graph.graph_id.clone(), Some(selected_graph.variant_name.clone()))?;
        
        // Fetch supergraph schema
        let supergraph_sdl = match fetch_supergraph(
            SupergraphFetchInput { graph_ref: graph_ref.clone() },
            &client,
        ).await {
            Ok(response) => response.sdl.contents,
            Err(e) => {
                eprintln!("{}", Style::WarningHeading.paint(format!("⚠️  Could not fetch supergraph schema: {}", e)));
                // Continue without the schema - MCP can still work with just the graph reference
                String::new()
            }
        };
        
        // Fetch subgraph information
        let subgraph_info = match list_subgraphs(
            SubgraphListInput { graph_ref: graph_ref.clone() },
            &client,
        ).await {
            Ok(response) => {
                let subgraph_names: Vec<String> = response.subgraphs.iter()
                    .map(|s| s.name.clone())
                    .collect();
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
        let graph_ref_str = format!("{}@{}", selected_graph.graph_id, selected_graph.variant_name);
        let graph_endpoint = format!("https://studio.apollographql.com/graph/{}/explorer", selected_graph.graph_id);
        
        // Get current directory
        let current_dir = match &self.path {
            Some(path) => {
                camino::Utf8PathBuf::try_from(path.clone())
                    .map_err(|_| RoverError::new(anyhow!("Invalid path")))?
            }
            None => camino::Utf8PathBuf::from("."),
        };
        
        // Fetch raw files from the add-mcp directory
        let branch_ref = "camille/start-with-mcp-template";
        let mut template_fetcher = InitTemplateFetcher::new();
        let template_options = template_fetcher.call(branch_ref).await?;
        
        // Extract files directly from the add-mcp directory
        let mut files = template_options.extract_directory_files("add-mcp")?;
        
        // Filter tools based on data source selection
        let tools_to_include = match data_source_type {
            MCPDataSourceType::ExternalAPIs => vec!["examples/api"],
            MCPDataSourceType::AWSServices => vec!["examples/aws"],
            MCPDataSourceType::GraphQLAPI => vec!["examples/graphql"],
        };
        
        // Copy only selected examples to tools/
        let mut files_to_remove = Vec::new();
        let mut files_to_add = Vec::new();
        
        for (file_path, content) in &files {
            if file_path.starts_with("examples/") {
                // Check if this example should be included
                let should_include = tools_to_include.iter().any(|&prefix| file_path.starts_with(prefix));
                if should_include {
                    // Rename from examples/category/file.graphql to tools/file.graphql
                    let new_path = file_path.as_str()
                        .replace("examples/api/", "tools/")
                        .replace("examples/aws/", "tools/")
                        .replace("examples/graphql/", "tools/");
                    files_to_add.push((new_path, content.clone()));
                }
                // Mark original examples/ file for removal
                files_to_remove.push(file_path.clone());
            }
        }
        
        // Remove example files
        for path in files_to_remove {
            files.remove(&path);
        }
        
        // Add renamed tool files
        for (path, content) in files_to_add {
            files.insert(path.into(), content);
        }
        
        // If we have a supergraph schema, save it
        if !supergraph_sdl.is_empty() {
            files.insert("supergraph.graphql".into(), supergraph_sdl.clone());
        }
        
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
   docker run --env-file .env -p5000:5000 {}-mcp
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
            if !subgraph_info.is_empty() { format!("\n- **{}**", subgraph_info) } else { String::new() },
            project_name,
            project_name,
            graph_endpoint
        );
        files.insert("README.md".into(), readme_content);
        
        // Get or create Apollo service key
        // First check if APOLLO_KEY is already set in environment
        let apollo_key = if let Ok(key) = std::env::var("APOLLO_KEY") {
            if key.starts_with("service:") {
                println!("{}", Style::Success.paint("✓ Using existing APOLLO_KEY from environment"));
                key
            } else {
                // Need to create a new service key for this graph
                println!("{}", Style::Heading.paint("🔑 Creating service API key..."));
                
                // Use the operations module to create API key
                use crate::command::init::operations::create_api_key;
                create_api_key(
                    client_config,
                    &self.profile,
                    selected_graph.graph_id.clone(),
                    format!("{}-mcp-server", selected_graph.graph_name),
                ).await?
            }
        } else {
            // Create new service key
            println!("{}", Style::Heading.paint("🔑 Creating service API key..."));
            
            use crate::command::init::operations::create_api_key;
            create_api_key(
                client_config,
                &self.profile,
                selected_graph.graph_id.clone(),
                format!("{}-mcp-server", selected_graph.graph_name),
            ).await?
        };
        
        // Write files to current directory with template replacement
        for (file_path, content) in &files {
            let final_file_path = camino::Utf8PathBuf::from(file_path);
            
            let target_path = current_dir.join(&final_file_path);
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            // Replace template placeholders with selected graph information
            let mut processed_content = content
                .replace("{{PROJECT_NAME}}", &project_name)
                .replace("{{GRAPH_REF}}", &graph_ref_str)
                .replace("{{GRAPH_ID}}", &selected_graph.graph_id)
                .replace("{{GRAPH_NAME}}", &selected_graph.graph_name)
                .replace("{{VARIANT_NAME}}", &selected_graph.variant_name)
                .replace("{{ORGANIZATION_NAME}}", &selected_graph.organization_name)
                .replace("{{APOLLO_API_KEY}}", &apollo_key)
                .replace("{{APOLLO_KEY}}", &apollo_key)
                .replace("{{GRAPHQL_ENDPOINT}}", "http://localhost:4000/graphql")
                .replace("{{GRAPH_STUDIO_URL}}", &graph_endpoint)
                .replace("{{PROJECT_VERSION}}", "1.0.0")
                .replace("{{PROJECT_REPOSITORY_URL}}", &format!("https://github.com/user/{}", project_name));
            
            // Handle the specific case where .env has placeholder API keys
            // Replace "service:{{PROJECT_NAME}}:YOUR_API_KEY_HERE" with actual service key
            if file_path == ".env" {
                processed_content = processed_content
                    .replace(&format!("service:{}:YOUR_API_KEY_HERE", project_name), &apollo_key)
                    .replace("service:{{PROJECT_NAME}}:YOUR_API_KEY_HERE", &apollo_key)
                    .replace("YOUR_API_KEY_HERE", &apollo_key.split(':').last().unwrap_or(&apollo_key));
            }
            
            std::fs::write(&target_path, processed_content)?;
        }
        
        println!("\n{}", Style::Success.paint("✅ MCP server added to your project!"));
        
        println!("\n{}", Style::Heading.paint(format!(
            "📁 Generated tools based on \"{}\" selection:",
            match data_source_type {
                MCPDataSourceType::ExternalAPIs => "External APIs",
                MCPDataSourceType::AWSServices => "AWS services",
                MCPDataSourceType::GraphQLAPI => "GraphQL API",
            }
        )));
        
        // List the actual tools generated
        let tool_files: Vec<String> = files.keys()
            .filter(|k| k.starts_with("tools/") && k.ends_with(".graphql"))
            .map(|k| format!("   - {}", k.strip_prefix("tools/").unwrap_or(k)))
            .collect();
        for tool in tool_files {
            println!("{}", tool);
        }
        
        println!("\n{}", Style::Heading.paint("📋 Selected Graph:"));
        println!("  • Graph: {} ({})", selected_graph.graph_name, selected_graph.variant_name);
        println!("  • Organization: {}", selected_graph.organization_name);
        println!("  • Graph Reference: {}", graph_ref_str);
        if !supergraph_sdl.is_empty() {
            println!("  • Supergraph schema: ✓ Downloaded");
        }
        println!("  • Service API key: ✓ Generated and configured");
        
        println!("\n{}", Style::Heading.paint("🚀 Next steps:"));
        println!("   1. docker build -f mcp.Dockerfile -t {}-mcp .", project_name);
        println!("   2. docker run --env-file .env -p5000:5000 {}-mcp", project_name);
        println!("   3. npx @modelcontextprotocol/inspector");
        
        println!("\n{}", Style::Heading.paint("📚 Resources:"));
        println!("   - More API connectors: apollographql.com/docs/connectors");
        println!("   - Add database access: apollographql.com/docs/mcp-databases");
        println!("   - Full documentation: apollographql.com/docs/mcp");
        
        println!("\n💡 Each .graphql file in /tools becomes an MCP tool!");
        
        Ok(RoverOutput::EmptySuccess)
    }
    
    /// Generate .env file with Apollo credentials for MCP project
    #[cfg(feature = "composition-js")]
    fn generate_mcp_env_file(completed_project: &states::ProjectCreated, output_path: &camino::Utf8PathBuf) -> RoverResult<()> {
        use rover_std::Fs;
        
        let env_path = output_path.join(".env");
        let env_content = format!(
            "APOLLO_KEY={}\nAPOLLO_GRAPH_REF={}\n",
            completed_project.api_key,
            completed_project.graph_ref
        );
        
        Fs::write_file(&env_path, env_content)?;
        Ok(())
    }
    
    /// Prompt user to select MCP project type (REST or GraphQL)
    #[cfg(feature = "composition-js")]
    fn prompt_mcp_project_type() -> RoverResult<MCPProjectType> {
        use dialoguer::Select;
        use dialoguer::console::Term;
        use anyhow::anyhow;
        use rover_std::Style;
        
        let options = vec![
            MCPProjectType::REST,
            MCPProjectType::GraphQL,
        ];
        
        let names = options
            .iter()
            .map(|o| o.to_string())
            .collect::<Vec<_>>();
            
        let selection = Select::new()
            .with_prompt(Style::Prompt.paint("? What type of MCP server are you building?").to_string())
            .items(&names)
            .default(0)
            .interact_on_opt(&Term::stderr())?;

        match selection {
            Some(index) => Ok(options[index].clone()),
            None => Err(RoverError::new(anyhow!("Use case selection cancelled"))),
        }
    }
    
    /// Prompt user to select MCP data source type
    #[cfg(feature = "composition-js")]
    fn prompt_mcp_data_source() -> RoverResult<MCPDataSourceType> {
        use dialoguer::Select;
        use dialoguer::console::Term;
        use anyhow::anyhow;
        
        let options = vec![
            MCPDataSourceType::ExternalAPIs,
            MCPDataSourceType::AWSServices,
            MCPDataSourceType::GraphQLAPI,
        ];
        
        let names = options.iter().map(|o| o.to_string()).collect::<Vec<_>>();
        
        let selection = Select::new()
            .with_prompt("What will your MCP server primarily connect to?")
            .items(&names)
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
        mcp_project_type: &MCPProjectType,
        data_source_type: &MCPDataSourceType,
    ) {
        use rover_std::Style;

        println!("\n{}", Style::Success.paint("✅ MCP server project ready!"));
        
        // Display selected data source type
        println!("\n{}", Style::Heading.paint(format!(
            "📁 Generated tools based on \"{}\" selection:",
            match data_source_type {
                MCPDataSourceType::ExternalAPIs => "External APIs",
                MCPDataSourceType::AWSServices => "AWS services",
                MCPDataSourceType::GraphQLAPI => "GraphQL API",
            }
        )));
        
        // List the actual tools generated
        let tool_files: Vec<String> = completed_project.artifacts.iter()
            .filter(|path| path.starts_with("tools/") && path.ends_with(".graphql"))
            .map(|path| format!("   - {}", path.strip_prefix("tools/").unwrap_or(path)))
            .collect();
        for tool in tool_files {
            println!("{}", tool);
        }
        
        // Project Details section
        println!("\n{}", Style::Heading.paint("📋 Project Details:"));
        println!("  • Project: {}", completed_project.config.project_name);
        println!("  • Organization: {}", completed_project.config.organization);
        println!("  • Graph Reference: {}", completed_project.graph_ref);
        match mcp_project_type {
            MCPProjectType::REST => println!("  • Type: REST APIs → MCP Server"),
            MCPProjectType::GraphQL => println!("  • Type: GraphQL API → MCP Server"),
        }
        println!("  • Service API key: ✓ Generated and configured");
        
        // Next Steps section
        println!("\n{}", Style::Heading.paint("🚀 Next steps:"));
        match mcp_project_type {
            MCPProjectType::GraphQL => {
                println!("   1. Start the subgraph server:");
                println!("      npm ci && npm run dev");
                println!("   2. Start local development:");
                println!("      export $(cat .env | xargs) && rover dev --supergraph-config supergraph.yaml");
                println!("   3. docker build -f mcp.Dockerfile -t {}-mcp .", completed_project.config.graph_id);
                println!("   4. docker run --env-file .env -p5000:5000 {}-mcp", completed_project.config.graph_id);
                println!("   5. npx @modelcontextprotocol/inspector");
            }
            MCPProjectType::REST => {
                println!("   1. Configure your REST API credentials in .apollo/router.local.yaml");
                println!("   2. Start local development:");
                println!("      export $(cat .env | xargs) && rover dev --supergraph-config connectors/supergraph.yaml");
                println!("   3. docker build -f mcp.Dockerfile -t {}-mcp .", completed_project.config.graph_id);
                println!("   4. docker run --env-file .env -p5000:5000 {}-mcp", completed_project.config.graph_id);
                println!("   5. npx @modelcontextprotocol/inspector");
            }
        }
        
        println!("\n{}", Style::Heading.paint("📚 Resources:"));
        println!("   - More API connectors: apollographql.com/docs/connectors");
        println!("   - Add database access: apollographql.com/docs/mcp-databases");
        println!("   - Full documentation: apollographql.com/docs/mcp");
        
        println!("\n💡 Each .graphql file in /tools becomes an MCP tool!");
    }
    
    /// Handle MCP setup for new project creation
    #[cfg(feature = "composition-js")]
    async fn handle_new_project_mcp(&self, _client: &rover_client::blocking::StudioClient, client_config: &StudioClientConfig) -> RoverResult<RoverOutput> {
        use rover_std::Style;
        use crate::command::init::states::{UserAuthenticated, ProjectTypeSelected, UseCaseSelected, TemplateSelected};
        use crate::command::init::transitions::CreateProjectResult;
        use crate::command::init::options::{ProjectType, ProjectUseCase};
        use anyhow::anyhow;
        
        println!("{}", Style::Heading.paint("🚀 Creating new project with MCP server..."));
        
        // Prompt for data source type
        let data_source_type = Self::prompt_mcp_data_source()?;
        
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
            .select_organization(&self.organization, &self.profile, &client_config)
            .await?;

        // Create use case selected state based on data source type
        let (use_case, base_template_id, mcp_project_type) = match data_source_type {
            MCPDataSourceType::ExternalAPIs => {
                // For External APIs, use start-with-rest + add-mcp
                (ProjectUseCase::Connectors, "connectors", MCPProjectType::REST)
            }
            MCPDataSourceType::AWSServices => {
                // For AWS services, use start-with-rest + add-mcp
                (ProjectUseCase::Connectors, "connectors", MCPProjectType::REST)
            }
            MCPDataSourceType::GraphQLAPI => {
                // For GraphQL APIs, use start-with-typescript + add-mcp
                (ProjectUseCase::GraphQLTemplate, "typescript", MCPProjectType::GraphQL)
            }
        };
        
        let use_case_selected = UseCaseSelected {
            output_path: organization_selected.output_path,
            project_type: organization_selected.project_type,
            organization: organization_selected.organization,
            use_case,
        };

        // Fetch base template + add-mcp using the existing fetch_mcp_template method
        let branch_ref = "camille/start-with-mcp-template";
        let mut template_fetcher = InitTemplateFetcher::new();
        let mut selected_template = template_fetcher.fetch_mcp_template(base_template_id, branch_ref).await?;
        
        // Filter files based on data source selection BEFORE preview
        let mut string_files: std::collections::HashMap<camino::Utf8PathBuf, String> = selected_template.files
            .iter()
            .map(|(path, bytes)| (path.clone(), String::from_utf8_lossy(bytes).to_string()))
            .collect();
        
        Self::filter_template_files_by_data_source(&mut string_files, &data_source_type);
        
        // Convert back to bytes
        selected_template.files = string_files.into_iter()
            .map(|(path, content)| (path, content.into_bytes()))
            .collect();
        
        let template_selected = TemplateSelected {
            output_path: use_case_selected.output_path,
            project_type: use_case_selected.project_type,
            organization: use_case_selected.organization,
            use_case: use_case_selected.use_case,
            selected_template,
        };

        // Continue with the normal naming flow, but skip graph ID confirmation for MCP
        let creation_confirmed = match template_selected
            .enter_project_name(&self.project_name)?
            .auto_generate_graph_id()?
            .preview_and_confirm_creation()
            .await?
        {
            Some(confirmed) => confirmed,
            None => return Ok(RoverOutput::EmptySuccess),
        };

        let project_created = creation_confirmed
            .create_project(&client_config, &self.profile)
            .await?;

        // Handle the project creation result
        let completed_project = match project_created {
            CreateProjectResult::Created(project) => project,
            CreateProjectResult::Restart { reason: _, state: _ } => {
                // For simplicity in v1, don't handle restarts for MCP flow
                return Err(RoverError::new(anyhow!(
                    "Project creation requires restart - please try again"
                )));
            }
        };

        // Generate .env file with Apollo credentials
        let output_path = match &self.path {
            Some(path) => camino::Utf8PathBuf::try_from(path.clone())
                .map_err(|_| RoverError::new(anyhow!("Invalid path")))?,
            None => camino::Utf8PathBuf::from("."),
        };
        Self::generate_mcp_env_file(&completed_project, &output_path)?;

        // Display MCP-specific success message instead of standard completion
        Self::display_mcp_project_success(&completed_project, &mcp_project_type, &data_source_type);
        
        Ok(RoverOutput::EmptySuccess)
    }
    
    /// Filter template files based on data source selection (for preview and creation)
    #[cfg(feature = "composition-js")]
    fn filter_template_files_by_data_source(
        files: &mut std::collections::HashMap<camino::Utf8PathBuf, String>,
        data_source_type: &MCPDataSourceType,
    ) {
        // Determine which examples to include
        let tools_to_include = match data_source_type {
            MCPDataSourceType::ExternalAPIs => vec!["examples/api"],
            MCPDataSourceType::AWSServices => vec!["examples/aws"],
            MCPDataSourceType::GraphQLAPI => vec!["examples/graphql"],
        };
        
        // Copy only selected examples to tools/
        let mut files_to_remove = Vec::new();
        let mut files_to_add = Vec::new();
        
        for (file_path, content) in files.iter() {
            if file_path.starts_with("examples/") {
                // Check if this example should be included
                let should_include = tools_to_include.iter().any(|&prefix| file_path.starts_with(prefix));
                if should_include {
                    // Rename from examples/category/file.graphql to tools/file.graphql
                    let new_path = file_path.as_str()
                        .replace("examples/api/", "tools/")
                        .replace("examples/aws/", "tools/")
                        .replace("examples/graphql/", "tools/");
                    files_to_add.push((camino::Utf8PathBuf::from(new_path), content.clone()));
                }
                // Mark original examples/ file for removal
                files_to_remove.push(file_path.clone());
            }
        }
        
        // Remove example files
        for path in files_to_remove {
            files.remove(&path);
        }
        
        // Add renamed tool files
        for (path, content) in files_to_add {
            files.insert(path, content);
        }
    }
    
    /// Apply file filtering for new project based on data source selection (kept for compatibility)
    #[cfg(feature = "composition-js")]
    fn filter_mcp_examples_for_new_project(
        project_path: &camino::Utf8PathBuf,
        data_source_type: &MCPDataSourceType,
    ) -> RoverResult<()> {
        let examples_path = project_path.join("examples");
        let tools_path = project_path.join("tools");
        
        // Check if examples directory exists
        if !examples_path.exists() {
            return Ok(()); // Nothing to filter
        }
        
        // Ensure tools directory exists
        std::fs::create_dir_all(&tools_path)?;
        
        // Determine which examples to include
        let source_dirs = match data_source_type {
            MCPDataSourceType::ExternalAPIs => vec!["api"],
            MCPDataSourceType::AWSServices => vec!["aws"],
            MCPDataSourceType::GraphQLAPI => vec!["graphql"],
        };
        
        // Copy relevant example files to tools directory
        for source_dir in source_dirs {
            let source_path = examples_path.join(source_dir);
            if source_path.exists() {
                for entry in std::fs::read_dir(&source_path)? {
                    let entry = entry?;
                    let file_path = entry.path();
                    if file_path.extension().and_then(|s| s.to_str()) == Some("graphql") {
                        let file_name = file_path.file_name().unwrap().to_str().unwrap();
                        let target_path = tools_path.join(file_name);
                        std::fs::copy(&file_path, &target_path)?;
                    }
                }
            }
        }
        
        // Remove the examples directory
        if examples_path.exists() {
            std::fs::remove_dir_all(&examples_path)?;
        }
        
        Ok(())
    }
    
    /// Extract project name from existing project configuration
    #[cfg(feature = "composition-js")]
    fn get_project_name_from_config(project_dir: &camino::Utf8PathBuf) -> RoverResult<String> {
        // Try to read from .env file first
        let env_path = project_dir.join(".env");
        if env_path.exists() {
            let env_content = std::fs::read_to_string(&env_path)?;
            for line in env_content.lines() {
                if let Some(graph_ref) = line.strip_prefix("APOLLO_GRAPH_REF=") {
                    let graph_ref = graph_ref.trim_matches('"');
                    // APOLLO_GRAPH_REF format is typically "graph-id@variant"
                    if let Some(graph_id) = graph_ref.split('@').next() {
                        if !graph_id.is_empty() {
                            return Ok(graph_id.to_string());
                        }
                    }
                }
            }
        }
        
        // Fall back to directory name
        let dir_name = project_dir
            .file_name()
            .unwrap_or("my-project")
            .to_string();
        
        Ok(dir_name)
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
