use std::{env, fs::read_dir, path::PathBuf};

use anyhow::anyhow;
use camino::Utf8PathBuf;
use houston::Profile;
use rover_client::operations::init::create_graph::*;
use rover_client::operations::init::memberships;
use rover_client::shared::GraphRef;
use rover_client::RoverClientError;
use rover_std::{errln, Spinner, Style};

use crate::command::init::authentication::{auth_error_to_rover_error, AuthenticationError};
use crate::command::init::config::ProjectConfig;
use crate::command::init::helpers::*;
use crate::command::init::operations::create_api_key;
use crate::command::init::operations::publish_subgraphs;
use crate::command::init::operations::update_variant_federation_version;
use crate::command::init::options::*;
use crate::command::init::states::*;
use crate::command::init::template_fetcher::TemplateId;
use crate::command::init::template_operations::{SupergraphBuilder, TemplateOperations};


use crate::command::init::InitTemplateFetcher;

use crate::options::{TemplateListFiles, TemplateWrite};

use crate::options::ProfileOpt;
use crate::utils::client::StudioClientConfig;
use crate::RoverError;
use crate::RoverErrorSuggestion;
use crate::RoverOutput;
use crate::RoverResult;

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

const DEFAULT_VARIANT: &str = "current";

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
    ) -> RoverResult<ProjectTypeSelected> {
        display_welcome_message();

        // Check if directory is empty before proceeding
        let current_dir = env::current_dir()?;
        let output_path =
            Utf8PathBuf::from_path_buf(override_install_path.clone().unwrap_or(current_dir))
                .map_err(|_| anyhow::anyhow!("Failed to parse directory"))?;
        if let Ok(mut dir) = read_dir(&output_path) {
            if dir.next().is_some() {
                return Err(RoverError::new(anyhow!(
                    "Cannot initialize the graph because the current directory is not empty"
                ))
                .with_suggestion(RoverErrorSuggestion::Adhoc(
                    format!(
                        "Please run `{}` in an empty directory and make sure to check for hidden files.",
                        Style::Command.paint("init")
                    )
                    .to_string(),
                )));
            }
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
        let repo_ref = env::var("ROVER_INIT_TEMPLATE_REF").unwrap_or_else(|_| "release/v2".to_string());
        let template_fetcher = InitTemplateFetcher::new().call(&repo_ref).await?;

        // Determine the list of templates based on the use case
        let selected_template: SelectedTemplateState = match self.use_case {
            // Select the `connectors` template if using use_case is Connectors
            ProjectUseCase::Connectors => {
                template_fetcher.select_template(&TemplateId("connectors".to_string()))?
            }
            // Otherwise, filter out the `connectors` template & show list of all others
            ProjectUseCase::GraphQLTemplate => {
                let templates = template_fetcher
                    .list_templates()
                    .iter()
                    .filter(|&t| t.id != TemplateId("connectors".to_string()))
                    .cloned()
                    .collect::<Vec<_>>();
                let template_id = options.get_or_prompt_template(&templates)?;
                template_fetcher.select_template(&template_id)?
            }
            // Use template for React template (defer version fetching until creation)
            #[cfg(feature = "react-template")]
            ProjectUseCase::ReactTemplate => {
                let template_id = TemplateId("react-typescript-apollo".to_string());
                template_fetcher.select_template(&template_id)?
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
/// ? Would you like AI-powered mock data for faster development?
/// ? Describe your app's domain or focus (optional):
#[cfg(feature = "react-template")]
impl TemplateSelected {
    pub fn configure_mocking(
        self,
        mocking_setup_options: &ProjectMockingSetupOpt,
        mocking_context_options: &ProjectMockingContextOpt,
    ) -> RoverResult<MockingConfigured> {
        // Only show mocking prompts for React templates
        if self.use_case != ProjectUseCase::ReactTemplate {
            return Err(RoverError::new(anyhow::anyhow!("Mocking configuration is only available for React templates")));
        }

        let mocking_setup = mocking_setup_options.get_or_prompt_mocking_setup()?;
        let mocking_context = if mocking_setup == MockingSetup::Yes {
            Some(mocking_context_options.get_or_prompt_mocking_context()?)
        } else {
            None
        };

        Ok(MockingConfigured {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            selected_template: self.selected_template,
            mocking_setup,
            mocking_context,
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
            #[cfg(feature = "react-template")]
            mocking_setup: None,
            #[cfg(feature = "react-template")]
            mocking_context: None,
        })
    }
}

/// PROMPT UX:
/// =========
///
/// ? Name your Graph:
#[cfg(feature = "react-template")]
impl MockingConfigured {
    pub fn enter_project_name(self, options: &ProjectNameOpt) -> RoverResult<ProjectNamed> {
        let project_name = options.get_or_prompt_project_name()?;

        Ok(ProjectNamed {
            output_path: self.output_path,
            project_type: self.project_type,
            organization: self.organization,
            use_case: self.use_case,
            selected_template: self.selected_template,
            project_name,
            mocking_setup: Some(self.mocking_setup),
            mocking_context: self.mocking_context,
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
            #[cfg(feature = "react-template")]
            mocking_setup: self.mocking_setup,
            #[cfg(feature = "react-template")]
            mocking_context: self.mocking_context,
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
/// getting-started.md
/// router.yaml
/// supergraph.yaml
/// schema.graphql
///
/// ? Proceed with creation? (y/n):
impl GraphIdConfirmed {
    pub fn create_config(&self) -> ProjectConfig {
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
                #[cfg(feature = "react-template")]
                if config.use_case == ProjectUseCase::ReactTemplate {
                    // React apps skip graph creation entirely
                    return Ok(Some(CreationConfirmed {
                        config,
                        selected_template: self.selected_template,
                        output_path: self.output_path,
                        skip_graph_creation: true,
                        #[cfg(feature = "react-template")]
                        mocking_setup: self.mocking_setup,
                        #[cfg(feature = "react-template")]
                        mocking_context: self.mocking_context,
                    }));
                }
                
                // Standard federated graph creation flow
                Ok(Some(CreationConfirmed {
                    config,
                    selected_template: self.selected_template,
                    output_path: self.output_path,
                    #[cfg(feature = "react-template")]
                    skip_graph_creation: false,
                    #[cfg(feature = "react-template")]
                    mocking_setup: self.mocking_setup,
                    #[cfg(feature = "react-template")]
                    mocking_context: self.mocking_context,
                }))
            }
            Ok(false) => {
                // User canceled
                #[cfg(feature = "react-template")]
                let cancel_message = if config.use_case == ProjectUseCase::ReactTemplate {
                    "React app creation canceled. You can run this command again anytime."
                } else {
                    "Graph creation canceled. You can run this command again anytime."
                };
                #[cfg(not(feature = "react-template"))]
                let cancel_message = "Graph creation canceled. You can run this command again anytime.";
                
                println!("{}", cancel_message);
                Ok(None)
            }
            Err(e) => Err(anyhow!("Failed to prompt user for confirmation: {}", e).into()),
        }
    }
}

impl CreationConfirmed {
    #[cfg(feature = "react-template")]
    fn replace_project_name_in_template(&mut self) -> RoverResult<()> {
        // Replace project name placeholders in all template files
        for (_path, content) in self.selected_template.files.iter_mut() {
            let content_str = String::from_utf8_lossy(content);
            let updated_content = content_str
                .replace("{{PROJECT_NAME}}", &self.config.project_name.to_string());
            
            *content = updated_content.into_bytes();
        }
        
        Ok(())
    }

    #[cfg(feature = "react-template")]
    fn replace_system_prompt_in_template(&mut self) -> RoverResult<()> {
        // Replace system prompt placeholders in all template files
        let system_prompt = self.mocking_context.as_ref()
            .map(|context| context.as_str())
            .unwrap_or("You are a helpful assistant that generates realistic mock data for GraphQL APIs. Generate data that matches the schema structure and provides meaningful, varied examples.");
        
        for (_path, content) in self.selected_template.files.iter_mut() {
            let content_str = String::from_utf8_lossy(content);
            let updated_content = content_str
                .replace("{{SYSTEM_PROMPT}}", system_prompt);
            
            *content = updated_content.into_bytes();
        }
        
        Ok(())
    }

    #[cfg(feature = "react-template")]
    async fn update_template_versions(&mut self) -> RoverResult<()> {
        use crate::command::init::react_template::SafeNpmClient;
        
        // Fetch latest versions
        let npm_client = SafeNpmClient::new();
        let deps = npm_client.get_deps_with_fallback().await;
        
        // Update template files with latest versions
        for (_path, content) in self.selected_template.files.iter_mut() {
            let content_str = String::from_utf8_lossy(content);
            let updated_content = content_str
                .replace("{{REACT_VERSION}}", &deps.react)
                .replace("{{REACT_DOM_VERSION}}", &deps.react_dom)
                .replace("{{REACT_ROUTER_DOM_VERSION}}", &deps.react_router_dom)
                .replace("{{APOLLO_CLIENT_VERSION}}", &deps.apollo_client)
                .replace("{{GRAPHQL_VERSION}}", &deps.graphql)
                .replace("{{VITE_VERSION}}", &deps.vite)
                .replace("{{VITE_PLUGIN_REACT_VERSION}}", &deps.vite_plugin_react)
                .replace("{{TYPESCRIPT_VERSION}}", &deps.typescript)
                .replace("{{TYPES_REACT_VERSION}}", &deps.types_react)
                .replace("{{TYPES_REACT_DOM_VERSION}}", &deps.types_react_dom)
                .replace("{{TYPESCRIPT_ESLINT_PLUGIN_VERSION}}", &deps.typescript_eslint_plugin)
                .replace("{{TYPESCRIPT_ESLINT_PARSER_VERSION}}", &deps.typescript_eslint_parser)
                .replace("{{ESLINT_VERSION}}", &deps.eslint)
                .replace("{{ESLINT_PLUGIN_REACT_HOOKS_VERSION}}", &deps.eslint_plugin_react_hooks)
                .replace("{{ESLINT_PLUGIN_REACT_REFRESH_VERSION}}", &deps.eslint_plugin_react_refresh)
                .replace("{{GRAPHQL_CODEGEN_CLI_VERSION}}", &deps.graphql_codegen_cli)
                .replace("{{GRAPHQL_CODEGEN_CLIENT_PRESET_VERSION}}", &deps.graphql_codegen_client_preset);
            
            *content = updated_content.into_bytes();
        }
        
        Ok(())
    }

    pub async fn create_project(
        self,
        client_config: &StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<CreateProjectResult> {
        #[cfg(feature = "react-template")]
        if self.skip_graph_creation {
            return self.create_react_project_without_graph(client_config, profile).await;
        }
        
        // Existing federated graph creation logic
        self.create_federated_project_with_graph(client_config, profile).await
    }

    async fn create_federated_project_with_graph(
        mut self,
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
                        #[cfg(feature = "react-template")]
                        mocking_setup: None,
                        #[cfg(feature = "react-template")]
                        mocking_context: None,
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
                        #[cfg(feature = "react-template")]
                        mocking_setup: None,
                        #[cfg(feature = "react-template")]
                        mocking_context: None,
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
        
        // Replace project name placeholder for React templates
        #[cfg(feature = "react-template")]
        if self.config.use_case == ProjectUseCase::ReactTemplate {
            // Fetch latest npm versions and update template files
            self.update_template_versions().await?;
            self.replace_project_name_in_template()?;
        }
        
        self.selected_template.write_template(&self.output_path)?;

        let routing_url = self.selected_template.template.routing_url.clone();
        let federation_version = self.selected_template.template.federation_version.clone();

        // Skip supergraph generation for React templates (client-side only)
        #[cfg(feature = "react-template")]
        let skip_supergraph = self.config.use_case == ProjectUseCase::ReactTemplate;
        #[cfg(not(feature = "react-template"))]
        let skip_supergraph = false;

        let graph_ref = GraphRef {
            name: create_graph_response.id.clone(),
            variant: DEFAULT_VARIANT.to_string(),
        };

        if !skip_supergraph {
            let supergraph = SupergraphBuilder::new(
                self.output_path.clone(),
                5,
                routing_url,
                &federation_version,
            );
            supergraph.build_and_write()?;
            
            let subgraphs = supergraph.generate_subgraphs()?;
            publish_subgraphs(&client, &self.output_path, &graph_ref, subgraphs).await?;
        }

        let artifacts = self.selected_template.list_files()?;

        // Only update federation version for GraphQL templates that have variants
        if !skip_supergraph {
            update_variant_federation_version(&client, &graph_ref, Some(federation_version)).await?;
        }

        // Create a new API key for the graph first
        let api_key = create_api_key(
            client_config,
            profile,
            self.config.graph_id.to_string(),
            self.config.project_name.to_string(),
        )
        .await?;

        spinner.success("Successfully created files and generated GraphOS credentials.");

        Ok(CreateProjectResult::Created(ProjectCreated {
            config: self.config,
            artifacts,
            api_key,
            graph_ref,
            template: self.selected_template.template,
            #[cfg(feature = "react-template")]
            graph_created: true,
        }))
    }

    #[cfg(feature = "react-template")]
    async fn create_react_project_without_graph(
        mut self,
        _client_config: &StudioClientConfig,
        _profile: &ProfileOpt,
    ) -> RoverResult<CreateProjectResult> {
        println!();
        let spinner = Spinner::new("Creating React app files...");
        
        // CRITICAL: No client authentication needed since no graph creation
        // This is the key insight - we can create React apps without GraphOS interaction
        
        // Write template files
        // Fetch latest npm versions and update template files
        self.update_template_versions().await?;
        self.replace_project_name_in_template()?;
        self.replace_system_prompt_in_template()?;
        self.selected_template.write_template(&self.output_path)?;
        
        let artifacts = self.selected_template.list_files()?;
        
        // Create a mock GraphRef for UI consistency
        let graph_ref = GraphRef {
            name: self.config.graph_id.to_string(),
            variant: DEFAULT_VARIANT.to_string(),
        };
        
        // IMPORTANT: No real API key needed for React apps
        // They'll configure their own GraphOS connection later
        let api_key = "react-app-placeholder".to_string();
        
        spinner.success("Successfully created React app files.");
        
        Ok(CreateProjectResult::Created(ProjectCreated {
            config: self.config,
            artifacts,
            api_key,
            graph_ref,
            template: self.selected_template.template,
            graph_created: false,
        }))
    }
}

impl ProjectCreated {
    pub fn complete(self) -> Completed {
        #[cfg(feature = "react-template")]
        let is_react_template = self.config.use_case == ProjectUseCase::ReactTemplate;
        #[cfg(not(feature = "react-template"))]
        let is_react_template = false;

        #[cfg(feature = "react-template")]
        let graph_created = self.graph_created;
        #[cfg(not(feature = "react-template"))]
        let graph_created = true;

        #[cfg(feature = "react-template")]
        let organization_string = self.config.organization.to_string();
        #[cfg(feature = "react-template")]
        let organization_id = if is_react_template {
            Some(organization_string.as_str())
        } else {
            None
        };
        #[cfg(not(feature = "react-template"))]
        let organization_id = None;

        display_project_created_message(
            self.config.project_name.to_string(),
            &self.artifacts,
            &self.graph_ref,
            self.api_key.to_string(),
            self.template.commands,
            self.template.start_point_file,
            self.template.print_depth,
            is_react_template,
            organization_id,
            graph_created,
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
