use std::{collections::HashMap, env, fs::read_dir, path::PathBuf};

use anyhow::anyhow;
use camino::Utf8PathBuf;
use houston::Profile;
use rover_client::RoverClientError;
use rover_client::operations::init::create_graph::*;
use rover_client::operations::init::memberships;
use rover_client::shared::GraphRef;
use rover_std::{Spinner, Style, errln};

use crate::command::init::authentication::{AuthenticationError, auth_error_to_rover_error};
use crate::command::init::config::ProjectConfig;
use crate::command::init::helpers::*;
use crate::command::init::operations::create_api_key;
use crate::command::init::operations::publish_subgraphs;
use crate::command::init::operations::update_variant_federation_version;
use crate::command::init::options::ProjectUseCaseOpt;
use crate::command::init::options::*;
use crate::command::init::states::*;
use crate::command::init::template_fetcher::TemplateId;
use crate::command::init::template_operations::{SupergraphBuilder, TemplateOperations};

use crate::command::init::InitTemplateFetcher;

use crate::options::{TemplateListFiles, TemplateWrite};

use crate::RoverError;
use crate::RoverErrorSuggestion;
use crate::RoverOutput;
use crate::RoverResult;
use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;

#[derive(Debug)]
pub enum RestartReason {
    GraphIdExists,
    FullRestart,
}

#[derive(Debug)]
pub enum CreateProjectResult {
    Created(ProjectCreated),
    Restart {
        state: ProjectNamed,
        reason: RestartReason,
    },
}

pub(crate) const DEFAULT_VARIANT: &str = "current";

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
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<Welcome> {
        match client_config.get_authenticated_client(profile) {
            Ok(_) => {
                let is_user_api_key = self.check_is_user_api_key(client_config, profile)?;
                if !is_user_api_key {
                    return Err(auth_error_to_rover_error(AuthenticationError::NotUserKey));
                };

                Ok(Welcome::new())
            }
            Err(_) => {
                match ProjectAuthenticationOpt::default().prompt_for_api_key(client_config, profile)
                {
                    Ok(_) => {
                        // Try to authenticate again with the new credentials
                        match client_config.get_authenticated_client(profile) {
                            Ok(_) => Ok(Welcome::new()),
                            Err(_) => Err(auth_error_to_rover_error(
                                AuthenticationError::SecondChanceAuthFailure,
                            )),
                        }
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    pub fn check_is_user_api_key(
        self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<bool> {
        let credential = Profile::get_credential(&profile.profile_name, &client_config.config)?;
        if credential.api_key.starts_with("user:") {
            return Ok(true);
        }

        Ok(false)
    }
}

/// PROMPT UX:
/// ==========
///
/// Welcome! This command helps you initialize a federated graph in your current directory.
/// To learn more about init, run `rover init -h` or visit https://www.apollographql.com/docs/rover/commands/init
///
/// ? Select option:
/// > Create a new graph
/// > Add a subgraph to an existing graph
impl Welcome {
    pub fn new() -> Self {
        Welcome {}
    }

    pub fn select_project_type(
        self,
        options: &ProjectTypeOpt,
        override_install_path: &Option<PathBuf>,
        _template_options: &ProjectTemplateOpt,
    ) -> RoverResult<ProjectTypeSelected> {
        display_welcome_message();

        // Check if directory is empty before proceeding (skip check for MCP augmentation)
        let current_dir = env::current_dir()?;
        let output_path =
            Utf8PathBuf::from_path_buf(override_install_path.clone().unwrap_or(current_dir))
                .map_err(|_| anyhow::anyhow!("Failed to parse directory"))?;

        // Check if directory is empty - normal init requires empty directory
        if let Ok(mut dir) = read_dir(&output_path)
            && dir.next().is_some()
        {
            return Err(RoverError::new(anyhow!(
                        "Cannot initialize the graph because the current directory is not empty"
                    ))
                    .with_suggestion(RoverErrorSuggestion::Adhoc(
                        format!(
                            "Please run `{}` in an empty directory or use the `--path` flag to specify a different directory.\n",
                            Style::Command.paint("rover init"),
                        )
                        .to_string(),
                    )));
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
        let client = match client_config.get_authenticated_client(profile) {
            Ok(client) => client,
            Err(_) => {
                return Err(auth_error_to_rover_error(
                    AuthenticationError::NoCredentialsFound,
                ));
            }
        };

        // Try to get memberships
        let memberships_response = memberships::run(&client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(msg))
            }
            e => e.into(),
        })?;

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
/// > Start a graph with one or more REST APIs
/// > Start a graph with recommended libraries
impl OrganizationSelected {
    pub fn select_use_case(
        self,
        options: &ProjectUseCaseOpt,
    ) -> RoverResult<Option<UseCaseSelected>> {
        let use_case = options.get_or_prompt_use_case()?;

        Ok(Some(UseCaseSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case,
        }))
    }
}

/// PROMPT UX:
/// =========
///
/// ? Select a language and server library:
/// > Template A
/// > Template B
/// > Template C
impl UseCaseSelected {
    pub async fn select_template(
        self,
        options: &ProjectTemplateOpt,
    ) -> RoverResult<TemplateSelected> {
        // Fetch the template to get the list of files
        let repo_ref = "release/v3";
        let mut template_fetcher = InitTemplateFetcher::new();
        let template_options = template_fetcher.call(repo_ref).await?;

        // MCP flow is handled in separate state machine, should not reach here with --mcp flag
        if options.mcp {
            unreachable!(
                "MCP flow should use dedicated state transitions, not reach template selection"
            );
        }

        // Determine the list of templates based on the use case
        let selected_template: SelectedTemplateState = match self.use_case {
            // Select the `connectors` template if using use_case is Connectors
            ProjectUseCase::Connectors => {
                template_options.select_template(&TemplateId("connectors".to_string()))?
            }
            // Otherwise, automatically select the first available template (basic flow)
            ProjectUseCase::GraphQLTemplate => {
                let templates = template_options
                    .list_templates()
                    .iter()
                    .filter(|&t| t.id != TemplateId("connectors".to_string()))
                    .cloned()
                    .collect::<Vec<_>>();

                let template_id = if let Some(template) = options.get_template() {
                    // Use explicitly provided template
                    template
                } else {
                    // Auto-select first available template for basic flow
                    templates
                        .first()
                        .ok_or_else(|| RoverError::new(anyhow!("No templates available")))?
                        .id
                        .clone()
                };

                // Regular template selection for non-MCP flow
                template_options.select_template(&template_id)?
            }
        };

        Ok(TemplateSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            selected_template,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// ? Name your Graph:
impl TemplateSelected {
    pub fn enter_project_name(self, options: &ProjectNameOpt) -> RoverResult<ProjectNamed> {
        let project_name = options.get_or_prompt_project_name()?;

        Ok(ProjectNamed {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            selected_template: self.selected_template,
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
            selected_template: self.selected_template,
            project_name: self.project_name,
            graph_id,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// => You're about to add the following files to your local directory:
///
/// .vscode/extensions.json
/// .idea/externalDependencies.xml
/// GETTING_STARTED.md
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

    pub async fn preview_and_confirm_creation(self) -> RoverResult<Option<CreationConfirmed>> {
        // Create the configuration
        let config = self.create_config();

        match TemplateOperations::prompt_creation(
            self.selected_template.list_files()?,
            self.selected_template.template.print_depth,
        ) {
            Ok(true) => {
                // User confirmed, proceed to create files
                Ok(Some(CreationConfirmed {
                    config,
                    selected_template: self.selected_template,
                    output_path: self.output_path,
                }))
            }
            Ok(false) => {
                // User canceled
                println!("Graph creation canceled. You can run this command again anytime.");
                Ok(None)
            }
            Err(e) => Err(anyhow!("Failed to prompt user for confirmation: {}", e).into()),
        }
    }
}

impl CreationConfirmed {
    pub async fn create_project(
        self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<CreateProjectResult> {
        println!();
        let spinner = Spinner::new("Creating files and generating GraphOS credentials...");
        let client = match client_config.get_authenticated_client(profile) {
            Ok(client) => client,
            Err(_) => {
                println!();
                errln!("Invalid API key. Please authenticate again.");
                return Ok(CreateProjectResult::Restart {
                    state: ProjectNamed {
                        output_path: self.output_path,
                        project_type: self.config.project_type,
                        organization: self.config.organization,
                        use_case: self.config.use_case,
                        selected_template: self.selected_template,
                        project_name: self.config.project_name,
                    },
                    reason: RestartReason::FullRestart,
                });
            }
        };

        let create_graph_response = match run(
            CreateGraphInput {
                hidden_from_uninvited_non_admin: false,
                create_graph_id: self.config.graph_id.to_string(),
                title: self.config.project_name.to_string(),
                organization_id: self.config.organization.to_string(),
            },
            &client,
        )
        .await
        {
            Ok(response) => response,
            Err(RoverClientError::GraphCreationError { msg })
                if msg.contains("Service already exists") =>
            {
                println!();
                println!();
                errln!("Graph ID is already in use. Please try again with a different graph ID.");
                return Ok(CreateProjectResult::Restart {
                    state: ProjectNamed {
                        output_path: self.output_path,
                        project_type: self.config.project_type,
                        organization: self.config.organization,
                        use_case: self.config.use_case,
                        selected_template: self.selected_template,
                        project_name: self.config.project_name,
                    },
                    reason: RestartReason::GraphIdExists,
                });
            }
            Err(e) => {
                tracing::error!("Failed to create graph: {:?}", e);
                if e.to_string().contains("Cannot create") {
                    let error =
                        RoverError::from(RoverClientError::PermissionError { msg: e.to_string() })
                            .with_suggestion(RoverErrorSuggestion::ContactApolloAccountManager);
                    return Err(error);
                }
                let error =
                    RoverError::from(e).with_suggestion(RoverErrorSuggestion::ContactApolloSupport);
                return Err(error);
            }
        };

        // Write the template files without asking for confirmation again
        // (confirmation was done in the previous state)
        self.selected_template.write_template(&self.output_path)?;

        let routing_url = self.selected_template.template.routing_url.clone();
        let federation_version = self.selected_template.template.federation_version.clone();

        let supergraph = SupergraphBuilder::new(
            self.output_path.clone(),
            5,
            routing_url,
            &federation_version,
        );
        supergraph.build_and_write()?;

        let artifacts = self.selected_template.list_files()?;

        let subgraphs = supergraph.generate_subgraphs()?;
        let graph_ref = GraphRef {
            name: create_graph_response.id.clone(),
            variant: DEFAULT_VARIANT.to_string(),
        };

        // Publish subgraphs to Studio (including connector schemas for MCP projects)
        publish_subgraphs(&client, &self.output_path, &graph_ref, subgraphs).await?;

        update_variant_federation_version(&client, &graph_ref, Some(federation_version)).await?;

        // Create a new API key for the graph first
        let api_key = create_api_key(
            client_config,
            profile,
            self.config.graph_id.to_string(),
            self.config.project_name.to_string(),
        )
        .await?;

        // Note: MCP projects use MCPCreationConfirmed.create_project() instead

        spinner.success("Successfully created project files and credentials");

        Ok(CreateProjectResult::Created(ProjectCreated {
            output_path: self.output_path,
            config: self.config,
            artifacts,
            api_key,
            graph_ref,
            template: self.selected_template.template,
        }))
    }
}

impl ProjectCreated {
    fn display_mcp_success(&self) {
        use rover_std::Style;

        println!("{}", Style::Success.paint("✓ MCP server generated"));
        println!(
            "{}",
            Style::Success.paint("✓ Credentials saved to .env file")
        );

        // Check if claude_desktop_config.json was created
        if self
            .artifacts
            .iter()
            .any(|p| p.as_str().contains("claude_desktop_config.json"))
        {
            println!(
                "{}",
                Style::Success
                    .paint("✓ Claude Desktop config generated: ./claude_desktop_config.json")
            );
        }

        // Project Details section
        println!();
        println!("{}", Style::Heading.paint("Project details"));
        println!("   • MCP Server Name: mcp-{}", self.config.project_name);
        println!("   • GraphOS Organization: {}", self.config.organization);
        println!();

        println!(
            "{}",
            Style::Heading.paint("GraphOS credentials for your graph")
        );
        println!(
            "   • {}: {}",
            Style::GraphRef.paint("APOLLO_GRAPH_REF"),
            self.graph_ref
        );
        println!(
            "   • {}: {} (This is your graph's API key)",
            Style::Command.paint("APOLLO_KEY"),
            self.api_key
        );
        println!();

        // Next Steps section
        println!();
        println!("{}", Style::Heading.paint("Next steps ↴"));
        println!();

        // Step 1: AI client setup
        println!("1. Connect an AI client to your MCP server:");
        println!();
        println!(
            "   Your MCP server name: {}",
            Style::Command.paint(format!("mcp-{}", self.config.project_name))
        );
        println!(
            "   MCP endpoint: {}",
            Style::Link.paint("http://127.0.0.1:5050/mcp")
        );
        println!();
        println!(
            "   For Claude Desktop setup: {}",
            Style::Command.paint("rover docs open mcp-claude")
        );
        println!();

        // Step 2: Base template commands (npm install, npm start, etc.) if they exist
        let has_commands = if let Some(commands) = &self.template.commands {
            let valid_commands: Vec<&str> = commands
                .iter()
                .filter(|cmd| !cmd.trim().is_empty())
                .map(|cmd| cmd.trim())
                .collect();

            if !valid_commands.is_empty() {
                if valid_commands.len() == 1 {
                    println!("2. Start the subgraph server by running the following command:");
                    println!();
                    println!("   {}", Style::Command.paint(valid_commands[0]));
                } else {
                    println!(
                        "2. Start the subgraph server by running the following commands in order:"
                    );
                    println!();
                    for cmd in valid_commands {
                        println!("   {}", Style::Command.paint(cmd));
                    }
                }
                println!();
                true
            } else {
                false
            }
        } else {
            false
        };

        // Determine the final step number based on whether commands were shown
        let final_step_num = if has_commands { "3" } else { "2" };

        println!(
            "{}. Start MCP server (after completing the above steps):",
            final_step_num
        );
        println!();
        println!("   Linux/macOS: {}", Style::Command.paint("set -a && source .env && set +a && rover dev --supergraph-config supergraph.yaml --mcp .apollo/mcp.local.yaml"));
        println!();
        println!("   Windows PowerShell:");
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
        println!();
    }

    pub fn complete(self) -> Completed {
        // Check if this is an MCP project by looking for MCP-specific files
        let is_mcp_project = self.artifacts.iter().any(|path| {
            let path_str = path.as_str();
            path_str.contains("mcp.local.yaml")
                || path_str.contains("mcp.staging.yaml")
                || path_str.contains("claude_desktop_config.json")
                || path_str.contains("mcp.Dockerfile")
        });

        if is_mcp_project {
            // Display MCP-specific success message
            self.display_mcp_success();
        } else {
            // Display regular rover init message
            display_project_created_message(
                self.config.project_name.to_string(),
                &self.artifacts,
                &self.graph_ref,
                self.api_key.to_string(),
                self.template.commands,
                self.template.start_point_file,
                self.template.print_depth,
            );
        }

        Completed
    }
}

// Completed state transition
impl Completed {
    pub fn success(self) -> RoverOutput {
        RoverOutput::EmptySuccess
    }
}

// MCP-specific state transitions
// These handle the MCP augmentation flow which branches from the main init flow

impl ProjectTypeSelected {
    /// Branch to MCP flow when --mcp flag is used
    pub fn initialize_mcp_augmentation(
        self,
        template_options: &ProjectTemplateOpt,
    ) -> RoverResult<MCPInitialization> {
        if !template_options.mcp {
            return Err(RoverError::new(anyhow!(
                "MCP initialization called without --mcp flag"
            )));
        }

        // MCP allows non-empty directories (augmenting existing projects)
        // So we skip the directory check that normal init performs

        Ok(MCPInitialization {
            output_path: self.output_path,
            project_type: self.project_type,
        })
    }
}

impl MCPInitialization {
    pub fn select_setup_type(self, options: &ProjectTypeOpt) -> RoverResult<MCPSetupTypeSelected> {
        use dialoguer::Select;
        use rover_std::Style;

        // Check if project_type was provided via command line
        let setup_type = if let Some(project_type) = &options.project_type {
            match project_type {
                ProjectType::CreateNew => MCPSetupType::NewProject,
                ProjectType::AddSubgraph => MCPSetupType::ExistingGraph,
            }
        } else {
            // Display MCP welcome message and prompt
            println!();
            println!(
                "Welcome! This command helps you initialize a federated graph with MCP server capabilities."
            );
            println!();
            println!(
                "To learn more about init, run `{}` or visit https://www.apollographql.com/docs/rover/commands/init",
                Style::Command.paint("rover init --mcp -h")
            );
            println!();

            let setup_types = vec![MCPSetupType::NewProject, MCPSetupType::ExistingGraph];

            let selection = Select::new()
                .with_prompt(Style::Prompt.paint("? Select option"))
                .items(&setup_types)
                .default(0)
                .interact()
                .map_err(|e| RoverError::new(anyhow!("Failed to get user selection: {}", e)))?;

            setup_types[selection].clone()
        };

        Ok(MCPSetupTypeSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            setup_type,
        })
    }
}

impl MCPSetupTypeSelected {
    pub fn select_data_source(
        self,
        options: &ProjectUseCaseOpt,
    ) -> RoverResult<MCPDataSourceSelected> {
        use dialoguer::Select;
        use rover_std::Style;

        // Only NewProject flow needs data source selection
        // ExistingGraph goes to different flow (handled in handle_mcp_flow)
        if !matches!(self.setup_type, MCPSetupType::NewProject) {
            return Err(RoverError::new(anyhow!(
                "Data source selection only available for new project flow"
            )));
        }

        // Check if project_use_case was provided via command line
        let data_source_type = if let Some(use_case) = &options.project_use_case {
            match use_case {
                ProjectUseCase::Connectors => MCPDataSourceType::ExternalAPIs,
                ProjectUseCase::GraphQLTemplate => MCPDataSourceType::GraphQLAPI,
            }
        } else {
            // Display data source selection prompt
            println!();
            println!(
                "️{}",
                Style::File
                    .paint("▲ AI-powered Apollo graph with MCP server ~10 minute setup time")
            );
            println!(
                "Build an Apollo GraphOS graph with MCP server capabilities. Start with a working template and connect your own APIs and data sources."
            );
            println!();
            println!(
                "{}",
                Style::Heading
                    .paint("Requirements: Your data source (API endpoint, database, or service)")
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
                .interact()
                .map_err(|e| RoverError::new(anyhow!("Failed to get user selection: {}", e)))?;

            options[selection].clone()
        };

        Ok(MCPDataSourceSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            setup_type: self.setup_type,
            data_source_type,
        })
    }
}

impl MCPDataSourceSelected {
    pub async fn select_organization(
        self,
        options: &ProjectOrganizationOpt,
        profile: &ProfileOpt,
        client_config: &StudioClientConfig,
    ) -> RoverResult<MCPOrganizationSelected> {
        let client = match client_config.get_authenticated_client(profile) {
            Ok(client) => client,
            Err(_) => {
                return Err(auth_error_to_rover_error(
                    AuthenticationError::NoCredentialsFound,
                ));
            }
        };

        // Reuse the same memberships logic as the main flow
        let memberships_response = memberships::run(&client).await.map_err(|e| match e {
            RoverClientError::GraphQl { msg } if msg.contains("Unauthorized") => {
                auth_error_to_rover_error(AuthenticationError::AuthenticationFailed(msg))
            }
            e => e.into(),
        })?;

        let organizations = memberships_response
            .memberships
            .iter()
            .map(|m| Organization::new(m.name.clone(), m.id.clone()))
            .collect::<Vec<_>>();

        let organization = options.get_or_prompt_organization(&organizations)?;

        Ok(MCPOrganizationSelected {
            output_path: self.output_path,
            project_type: self.project_type,
            organization,
            setup_type: self.setup_type,
            data_source_type: self.data_source_type,
        })
    }
}

impl MCPOrganizationSelected {
    pub async fn compose_mcp_template(self) -> RoverResult<MCPTemplateComposed> {
        // Determine use case and template based on data source type
        let use_case = match self.data_source_type {
            MCPDataSourceType::ExternalAPIs => ProjectUseCase::Connectors,
            MCPDataSourceType::GraphQLAPI => ProjectUseCase::GraphQLTemplate,
        };

        let repo_ref = "release/v3";
        let mut template_fetcher = InitTemplateFetcher::new();

        // Select template based on data source type
        let template_id = match self.data_source_type {
            MCPDataSourceType::ExternalAPIs => "connectors",
            MCPDataSourceType::GraphQLAPI => "typescript",
        };

        // Use the MCP-specific template fetcher that merges base template + add-mcp
        let selected_template = template_fetcher
            .fetch_mcp_template(template_id, repo_ref)
            .await?;

        // Create MCPComposedTemplate with AGENTS.md merge applied for new projects
        // The selected_template.files already contains base template + MCP additions
        let composed_template = MCPComposedTemplate::new_with_agents_merge(
            selected_template.template,
            selected_template.files,
            self.project_type.clone(),
        );

        Ok(MCPTemplateComposed {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case,
            composed_template,
            setup_type: self.setup_type,
            data_source_type: self.data_source_type,
        })
    }
}

impl MCPTemplateComposed {
    pub fn enter_project_name(self, options: &ProjectNameOpt) -> RoverResult<MCPProjectNamed> {
        let project_name = options.get_or_prompt_project_name()?;

        Ok(MCPProjectNamed {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            composed_template: self.composed_template,
            project_name,
            setup_type: self.setup_type,
            data_source_type: self.data_source_type,
        })
    }
}

impl MCPProjectNamed {
    pub fn confirm_graph_id(self, options: &GraphIdOpt) -> RoverResult<MCPGraphIdConfirmed> {
        let graph_id = options.get_or_prompt_graph_id(&self.project_name.to_string())?;

        Ok(MCPGraphIdConfirmed {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            project_name: self.project_name,
            graph_id,
            composed_template: self.composed_template,
            setup_type: self.setup_type,
            data_source_type: self.data_source_type,
        })
    }
}

impl MCPGraphIdConfirmed {
    /// Show MCP-specific preview and get user confirmation
    pub async fn preview_mcp_creation(self) -> RoverResult<Option<MCPCreationPreviewed>> {
        use dialoguer::Confirm;
        use rover_std::Style;

        println!();
        println!("=> You're about to add the following files to your local directory:");
        println!();

        // Get the files from the composed template
        let files = self.composed_template.list_files();
        print_mcp_file_categories(files);

        println!();
        println!("{}", Style::File.paint("What this template gives you"));

        // Customize message based on data source type
        match self.data_source_type {
            MCPDataSourceType::ExternalAPIs => {
                println!("- Example GraphQL schema and REST connectors");
                println!("- Pre-configured MCP server with Docker setup");
                println!("- Sample tools showing how to make APIs AI-callable");
            }
            MCPDataSourceType::GraphQLAPI => {
                println!("- Example GraphQL schema and GraphQL connectors");
                println!("- Pre-configured MCP server with Docker setup");
                println!("- Sample tools showing how to make GraphQL APIs AI-callable");
            }
        }
        println!();

        let confirmed = Confirm::new()
            .with_prompt("Create this template?")
            .default(true)
            .interact()
            .map_err(|e| RoverError::new(anyhow!("Failed to get user confirmation: {}", e)))?;

        if confirmed {
            let config = ProjectConfig {
                organization: self.organization,
                use_case: self.use_case,
                project_name: self.project_name,
                graph_id: self.graph_id,
                project_type: self.project_type,
            };

            Ok(Some(MCPCreationPreviewed {
                output_path: self.output_path,
                config,
                composed_template: self.composed_template,
                setup_type: self.setup_type,
                data_source_type: self.data_source_type,
            }))
        } else {
            println!("Template creation cancelled. You can run this command again anytime.");
            Ok(None)
        }
    }
}

impl MCPCreationPreviewed {
    /// Convert to MCPCreationConfirmed for type-safe MCP project creation
    pub fn into_mcp_creation_confirmed(self) -> RoverResult<MCPCreationConfirmed> {
        Ok(MCPCreationConfirmed {
            config: self.config,
            composed_template: self.composed_template,
            output_path: self.output_path,
        })
    }
}

impl MCPCreationConfirmed {
    // Removed unused process_mcp_env_files method - functionality moved to unified helper

    /// Process template placeholders in any file content (for MCP projects)
    /// Handles both ${} and {{}} placeholder formats for comprehensive compatibility
    pub fn process_template_placeholders(
        &self,
        content: &str,
        api_key: &str,
        graph_ref: &GraphRef,
    ) -> String {
        use crate::command::init::helpers::{
            MCPTemplateContext, process_mcp_template_placeholders,
        };

        let project_name = self.config.project_name.to_string();
        let graph_id = self.config.graph_id.to_string();
        let organization = self.config.organization.to_string();

        // Use unified template processing for consistency
        let ctx = MCPTemplateContext {
            project_name: &project_name,
            graph_id: &graph_id,
            graph_name: &project_name, // graph_name same as project_name for new projects
            variant_name: "current",
            organization_name: &organization,
            api_key,
            graph_ref,
            mcp_server_binary: None,
            mcp_config_path: None,
            tools_path: None,
        };

        process_mcp_template_placeholders(content, &ctx)
    }

    pub async fn create_project(
        self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<CreateProjectResult> {
        println!();
        let spinner = Spinner::new("Creating files and generating GraphOS credentials...");
        let client = match client_config.get_authenticated_client(profile) {
            Ok(client) => client,
            Err(_) => {
                println!();
                errln!("Invalid API key. Please authenticate again.");
                return Ok(CreateProjectResult::Restart {
                    state: ProjectNamed {
                        output_path: self.output_path,
                        project_type: self.config.project_type,
                        organization: self.config.organization,
                        use_case: self.config.use_case,
                        project_name: self.config.project_name,
                        selected_template: SelectedTemplateState {
                            template: self.composed_template.base_template.clone(),
                            files: self.composed_template.merged_files.clone(),
                        },
                    },
                    reason: RestartReason::FullRestart,
                });
            }
        };

        // Create the graph in GraphOS first (following rover init pattern)
        let create_graph_response = match run(
            CreateGraphInput {
                hidden_from_uninvited_non_admin: false,
                create_graph_id: self.config.graph_id.to_string(),
                title: self.config.project_name.to_string(),
                organization_id: self.config.organization.to_string(),
            },
            &client,
        )
        .await
        {
            Ok(response) => response,
            Err(RoverClientError::GraphCreationError { msg })
                if msg.contains("Service already exists") =>
            {
                println!();
                println!();
                errln!("Graph ID is already in use. Please try again with a different graph ID.");
                return Ok(CreateProjectResult::Restart {
                    state: ProjectNamed {
                        output_path: self.output_path,
                        project_type: self.config.project_type,
                        organization: self.config.organization,
                        use_case: self.config.use_case,
                        selected_template: SelectedTemplateState {
                            template: self.composed_template.base_template.clone(),
                            files: self.composed_template.merged_files.clone(),
                        },
                        project_name: self.config.project_name,
                    },
                    reason: RestartReason::GraphIdExists,
                });
            }
            Err(e) => {
                tracing::error!("Failed to create graph: {:?}", e);
                if e.to_string().contains("Cannot create") {
                    let error =
                        RoverError::from(RoverClientError::PermissionError { msg: e.to_string() })
                            .with_suggestion(RoverErrorSuggestion::ContactApolloAccountManager);
                    return Err(error);
                }
                let error =
                    RoverError::from(e).with_suggestion(RoverErrorSuggestion::ContactApolloSupport);
                return Err(error);
            }
        };

        // Create API key first so we can use it for placeholder processing
        let api_key = create_api_key(
            client_config,
            profile,
            self.config.graph_id.to_string(),
            self.config.project_name.to_string(),
        )
        .await?;

        let graph_ref = GraphRef {
            name: create_graph_response.id.clone(),
            variant: DEFAULT_VARIANT.to_string(),
        };

        // Process ALL template placeholders in merged files (including add-MCP files)
        let mut processed_files = HashMap::new();
        for (file_path, content) in &self.composed_template.merged_files {
            let content_str = String::from_utf8_lossy(content);
            let processed_content =
                self.process_template_placeholders(&content_str, &api_key, &graph_ref);

            // Handle .env.template → .env renaming for MCP projects
            let final_path = if file_path.as_str().ends_with(".env.template") {
                Utf8PathBuf::from(file_path.as_str().replace(".env.template", ".env"))
            } else {
                file_path.clone()
            };

            processed_files.insert(final_path, processed_content.into_bytes());
        }

        // Create a temporary SelectedTemplateState with processed files
        let selected_template = SelectedTemplateState {
            template: self.composed_template.base_template.clone(),
            files: processed_files,
        };

        // Write processed template files to filesystem
        selected_template.write_template(&self.output_path)?;

        // Build supergraph configuration (required for MCP projects)
        let routing_url = selected_template.template.routing_url.clone();
        let federation_version = selected_template.template.federation_version.clone();
        let supergraph = SupergraphBuilder::new(
            self.output_path.clone(),
            5,
            routing_url,
            &federation_version,
        );
        supergraph.build_and_write()?;

        let artifacts = selected_template.list_files()?;
        let subgraphs = supergraph.generate_subgraphs()?;

        // Publish subgraphs to Studio (including connector schemas for MCP projects)
        publish_subgraphs(&client, &self.output_path, &graph_ref, subgraphs).await?;
        update_variant_federation_version(&client, &graph_ref, Some(federation_version)).await?;

        spinner.success("Successfully created project files and credentials");

        Ok(CreateProjectResult::Created(ProjectCreated {
            output_path: self.output_path,
            config: self.config,
            artifacts,
            api_key,
            graph_ref,
            template: self.composed_template.base_template,
        }))
    }
}
