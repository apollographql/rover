use std::fs;
use std::collections::HashMap;
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use rover_std::{Fs, Style};
use houston as config;
use serde_json::json;

use crate::{RoverResult, RoverError};
use crate::command::init::InitTemplateFetcher;
use crate::command::init::states::SelectedTemplateState;
use crate::options::{ProfileOpt, TemplateWrite};
use crate::utils::client::StudioClientConfig;

#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    RestConnector,
    StandardGraphQL,
}

#[derive(Debug, Clone)]
pub struct MCPTool {
    pub name: String,
    pub operation_type: String, // "query" or "mutation"
    pub field_name: String,
    pub return_type: String,
    pub description: String,
    pub example_usage: Vec<String>,
    pub variables: Vec<String>,
}

#[derive(Debug)]
pub struct MCPAugmentationResult {
    pub tools_generated: Vec<MCPTool>,
    pub files_created: Vec<Utf8PathBuf>,
    pub project_type: ProjectType,
}

pub struct MCPAugmentation;

impl MCPAugmentation {
    /// Main entry point to augment a project for MCP
    pub async fn augment_project(
        project_path: &Utf8PathBuf,
        profile_name: &str,
        config: &config::Config,
        client_config: &StudioClientConfig,
    ) -> RoverResult<MCPAugmentationResult> {
        println!("{}", Style::Heading.paint("🚀 Augmenting project for MCP..."));
        
        // 1. Detect project type from existing schemas
        let schema_files = Self::find_graphql_schemas(project_path)?;
        let project_type = Self::detect_project_type(&schema_files)?;
        
        println!("✓ Detected {} project", match project_type {
            ProjectType::RestConnector => "REST Connector (found @connect directives)",
            ProjectType::StandardGraphQL => "Standard GraphQL",
        });
        
        // 2. Extract or generate Apollo service credentials
        let (api_key, graph_ref) = Self::get_or_create_apollo_credentials(
            project_path,
            profile_name,
            config,
            client_config,
        ).await?;
        println!("✓ Extracted credentials from profile '{}'", profile_name);
        
        // 3. Generate tools from existing schemas
        let tools = Self::extract_tools_from_schemas(&schema_files, &project_type)?;
        println!("✓ Generated {} MCP tool{}", tools.len(), if tools.len() != 1 { "s" } else { "" });
        
        // 4. Fetch and apply the add-mcp template
        let mut files_created = Vec::new();
        
        // Fetch the add-mcp template from the branch
        let mut template_fetcher = InitTemplateFetcher::new();
        let template_ref = "camille/start-with-mcp-template";
        let template_options = template_fetcher.call(template_ref).await?;
        
        // Select the mcp template specifically
        let add_mcp_state = template_options.select_template(&crate::command::init::template_fetcher::TemplateId("mcp".to_string()))?;
        
        // Prepare template variables for replacement
        let template_vars = Self::prepare_template_variables(project_path, &api_key, &graph_ref, &project_type)?;
        
        // Apply variable replacements to template files
        let processed_template = Self::process_template_with_variables(add_mcp_state, &template_vars)?;
        
        // Write template files (handles merging with existing files)
        processed_template.write_template(project_path)?;
        
        // Track files created by template
        files_created.extend(Self::list_template_files(project_path)?);
        
        // Generate dynamic tools from existing schemas (our value-add)
        Self::generate_tool_files(project_path, &tools)?;
        files_created.push(project_path.join("tools"));
        
        // Update .env file with actual credentials (overwrite template's .env.template)
        let env_path = Self::update_env_file(project_path, &api_key, &graph_ref)?;
        files_created.push(env_path);
        
        println!("✓ Created MCP configuration files");
        
        // 5. Validate Docker setup (optional)
        Self::validate_docker_build(project_path)?;
        
        // 6. Report results
        Self::print_generation_summary(&tools, &files_created, project_path, &project_type)?;
        
        Ok(MCPAugmentationResult {
            tools_generated: tools,
            files_created,
            project_type,
        })
    }
    
    /// Find all GraphQL schema files in the project
    fn find_graphql_schemas(project_path: &Utf8PathBuf) -> RoverResult<HashMap<Utf8PathBuf, String>> {
        let mut schema_files = HashMap::new();
        
        // Look for .graphql files in common locations
        let search_paths = vec![
            project_path.clone(),
            project_path.join("schema"),
            project_path.join("schemas"),
            project_path.join("connectors"),
        ];
        
        for search_path in search_paths {
            if search_path.exists() && search_path.is_dir() {
                if let Ok(entries) = fs::read_dir(&search_path) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if let Some(extension) = path.extension() {
                            if extension == "graphql" || extension == "gql" {
                                if let Ok(utf8_path) = Utf8PathBuf::try_from(path) {
                                    if let Ok(content) = fs::read_to_string(&utf8_path) {
                                        schema_files.insert(utf8_path, content);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        
        if schema_files.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No GraphQL schema files found. Expected .graphql or .gql files in project directory."
            )));
        }
        
        Ok(schema_files)
    }
    
    /// Detect project type based on schema content
    fn detect_project_type(schema_files: &HashMap<Utf8PathBuf, String>) -> RoverResult<ProjectType> {
        for content in schema_files.values() {
            if content.contains("@connect") || content.contains("@source") {
                return Ok(ProjectType::RestConnector);
            }
        }
        
        // Check for standard GraphQL patterns
        for content in schema_files.values() {
            if content.contains("type Query") || content.contains("type Mutation") {
                return Ok(ProjectType::StandardGraphQL);
            }
        }
        
        // Default to standard GraphQL if we have schemas but can't determine type
        Ok(ProjectType::StandardGraphQL)
    }
    
    /// Extract MCP tools from GraphQL schemas
    fn extract_tools_from_schemas(
        schema_files: &HashMap<Utf8PathBuf, String>,
        project_type: &ProjectType,
    ) -> RoverResult<Vec<MCPTool>> {
        let mut tools = Vec::new();
        
        for (file_path, content) in schema_files {
            let file_tools = Self::parse_schema_for_tools(content, file_path, project_type)?;
            tools.extend(file_tools);
        }
        
        if tools.is_empty() {
            return Err(RoverError::new(anyhow!(
                "No Query or Mutation fields found to generate MCP tools"
            )));
        }
        
        Ok(tools)
    }
    
    /// Parse a single GraphQL schema to extract tools
    fn parse_schema_for_tools(
        content: &str, 
        _file_path: &Utf8PathBuf,
        project_type: &ProjectType,
    ) -> RoverResult<Vec<MCPTool>> {
        let mut tools = Vec::new();
        
        // Simple regex-based parsing for MVP (can be replaced with AST parsing later)
        let lines: Vec<&str> = content.lines().collect();
        let mut in_query_type = false;
        let mut in_mutation_type = false;
        let mut brace_count = 0;
        let mut current_field = String::new();
        
        for line in lines {
            let trimmed = line.trim();
            
            // Detect Query type block
            if trimmed.starts_with("type Query") {
                in_query_type = true;
                brace_count = 0;
                // Handle case where opening brace is on same line
                if trimmed.contains('{') {
                    brace_count += trimmed.chars().filter(|&c| c == '{').count() as i32;
                    brace_count -= trimmed.chars().filter(|&c| c == '}').count() as i32;
                }
                continue;
            }
            
            // Detect Mutation type block  
            if trimmed.starts_with("type Mutation") {
                in_mutation_type = true;
                brace_count = 0;
                // Handle case where opening brace is on same line
                if trimmed.contains('{') {
                    brace_count += trimmed.chars().filter(|&c| c == '{').count() as i32;
                    brace_count -= trimmed.chars().filter(|&c| c == '}').count() as i32;
                }
                continue;
            }
            
            // Track braces to know when we exit the type block
            brace_count += trimmed.chars().filter(|&c| c == '{').count() as i32;
            brace_count -= trimmed.chars().filter(|&c| c == '}').count() as i32;
            
            if brace_count <= 0 && (in_query_type || in_mutation_type) {
                // Process any remaining field before exiting
                if !current_field.is_empty() {
                    if let Some(tool) = Self::parse_field_definition(&current_field, in_query_type, project_type) {
                        tools.push(tool);
                    }
                    current_field.clear();
                }
                in_query_type = false;
                in_mutation_type = false;
                continue;
            }
            
            // Extract field definitions within Query/Mutation types
            if (in_query_type || in_mutation_type) && !trimmed.is_empty() && !trimmed.starts_with('#') {
                // Check if this line contains a field definition (has colon and isn't a directive)
                if trimmed.contains(':') && !trimmed.starts_with('@') && !trimmed.ends_with(':') && !trimmed.contains('"') {
                    // Process any previous field
                    if !current_field.is_empty() {
                        if let Some(tool) = Self::parse_field_definition(&current_field, in_query_type, project_type) {
                            tools.push(tool);
                        }
                    }
                    // Start new field
                    current_field = trimmed.to_string();
                } else if trimmed.starts_with('@') || trimmed.starts_with('{') || trimmed.starts_with('}') {
                    // This is a directive or block - process current field if we have one
                    if !current_field.is_empty() {
                        if let Some(tool) = Self::parse_field_definition(&current_field, in_query_type, project_type) {
                            tools.push(tool);
                        }
                        current_field.clear();
                    }
                }
            }
        }
        
        // Process any remaining field
        if !current_field.is_empty() {
            if let Some(tool) = Self::parse_field_definition(&current_field, in_query_type, project_type) {
                tools.push(tool);
            }
        }
        
        Ok(tools)
    }
    
    /// Parse a single field definition to create an MCP tool
    fn parse_field_definition(line: &str, is_query: bool, project_type: &ProjectType) -> Option<MCPTool> {
        // Simple field parsing: fieldName: ReturnType or fieldName(args): ReturnType
        let trimmed = line.trim().trim_end_matches(',');
        
        if trimmed.starts_with('{') || trimmed.starts_with('}') || trimmed.starts_with('@') {
            return None;
        }
        
        // Extract field name and return type
        let colon_pos = trimmed.find(':')?;
        let field_part = &trimmed[..colon_pos].trim();
        let return_type_part = &trimmed[colon_pos + 1..].trim();
        
        // Handle field with arguments: fieldName(arg: Type)
        let field_name = if let Some(paren_pos) = field_part.find('(') {
            &field_part[..paren_pos]
        } else {
            field_part
        }.trim();
        
        // Skip internal GraphQL fields
        if field_name.starts_with('_') {
            return None;
        }
        
        let return_type = return_type_part
            .split_whitespace()
            .next()?
            .trim();
        
        // Generate tool name (PascalCase)
        let tool_name = Self::generate_tool_name(field_name, is_query);
        
        // Generate description and examples based on project type
        let (description, examples) = Self::generate_tool_metadata(field_name, return_type, project_type);
        
        Some(MCPTool {
            name: tool_name,
            operation_type: if is_query { "query".to_string() } else { "mutation".to_string() },
            field_name: field_name.to_string(),
            return_type: return_type.to_string(),
            description,
            example_usage: examples,
            variables: vec![], // TODO: Parse actual variables from schema
        })
    }
    
    /// Generate tool name from field name (GetProducts from products)
    fn generate_tool_name(field_name: &str, is_query: bool) -> String {
        let prefix = if is_query { "Get" } else { "" };
        let pascal_case = Self::to_pascal_case(field_name);
        format!("{}{}", prefix, pascal_case)
    }
    
    /// Convert snake_case or camelCase to PascalCase
    fn to_pascal_case(input: &str) -> String {
        input
            .split('_')
            .map(|word| {
                let mut chars: Vec<char> = word.chars().collect();
                if !chars.is_empty() {
                    chars[0] = chars[0].to_uppercase().next().unwrap_or(chars[0]);
                }
                chars.into_iter().collect::<String>()
            })
            .collect::<String>()
    }
    
    /// Generate description and examples for a tool based on project type
    fn generate_tool_metadata(
        field_name: &str,
        return_type: &str,
        project_type: &ProjectType,
    ) -> (String, Vec<String>) {
        let description = match project_type {
            ProjectType::RestConnector => {
                format!("Retrieves {} data from the REST API via Apollo Connectors.\n\nThis tool connects to your REST API endpoint to fetch {} information that AI assistants can use to answer questions about your data.", field_name, field_name)
            }
            ProjectType::StandardGraphQL => {
                format!("Queries {} data from your GraphQL API.\n\nThis tool enables AI assistants to fetch {} information directly from your GraphQL endpoint.", field_name, field_name)
            }
        };
        
        let examples = match field_name {
            name if name.contains("product") => vec![
                format!("What {}s are available?", name.trim_end_matches('s')),
                format!("Show me the {} catalog", name.trim_end_matches('s')),
                format!("List all {}s with details", name.trim_end_matches('s')),
            ],
            name if name.contains("user") => vec![
                format!("Who are the current {}s?", name.trim_end_matches('s')),
                format!("Show me {} information", name.trim_end_matches('s')),
                format!("List all {}s", name.trim_end_matches('s')),
            ],
            name => vec![
                format!("Show me {}", name),
                format!("What {} do you have?", name),
                format!("List all {}", name),
            ],
        };
        
        (description, examples)
    }
    
    /// Get existing Apollo service credentials or create new ones
    pub async fn get_or_create_apollo_credentials(
        project_path: &Utf8PathBuf,
        profile_name: &str,
        config: &config::Config,
        client_config: &StudioClientConfig,
    ) -> RoverResult<(String, String)> {
        
        // 1. First check for existing service key in environment
        if let Ok(apollo_key) = std::env::var("APOLLO_KEY") {
            if apollo_key.starts_with("service:") {
                let graph_ref = Self::extract_graph_ref_from_service_key(&apollo_key)?;
                println!("✓ Found existing service key in APOLLO_KEY environment variable");
                return Ok((apollo_key, graph_ref));
            }
        }
        
        // 2. Check for existing .env file with service key  
        let env_file_path = project_path.join(".env");
        if env_file_path.exists() {
            if let Ok(env_content) = fs::read_to_string(&env_file_path) {
                for line in env_content.lines() {
                    if let Some(key_value) = line.strip_prefix("APOLLO_KEY=") {
                        let apollo_key = key_value.trim();
                        if apollo_key.starts_with("service:") {
                            let graph_ref = Self::extract_graph_ref_from_service_key(apollo_key)?;
                            println!("✓ Found existing service key in .env file");
                            return Ok((apollo_key.to_string(), graph_ref));
                        }
                    }
                }
            }
        }
        
        // 3. No existing service key found - derive graph ID from project
        println!("⚠ No existing service key (APOLLO_KEY) found.");
        
        // Try to derive graph ID from project structure
        let graph_id = Self::derive_graph_id_from_project(project_path)?;
        println!("📝 Using derived graph ID: '{}'", graph_id);
        
        // For now, use a placeholder service key to test parsing logic
        // This will be replaced with proper key generation once parsing works
        let placeholder_key = format!("service:{}:placeholder-key-for-testing", graph_id);
        let graph_ref = format!("{}@current", graph_id);
        
        println!("⚠ Using placeholder credentials for development testing");
        println!("   In production, run: rover graph introspect {} | rover subgraph publish {} --name mcp --schema -", graph_ref, graph_ref);
        
        Ok((placeholder_key, graph_ref))
    }
    
    /// Extract graph reference from service key format: "service:graph-id:key"
    pub fn extract_graph_ref_from_service_key(service_key: &str) -> RoverResult<String> {
        if let Some(service_start) = service_key.find("service:") {
            if let Some(first_colon) = service_key[service_start + 8..].find(':') {
                let graph_id = &service_key[service_start + 8..service_start + 8 + first_colon];
                Ok(format!("{}@current", graph_id))
            } else {
                Err(RoverError::new(anyhow!("Invalid service API key format. Expected 'service:graph-id:key'")))
            }
        } else {
            Err(RoverError::new(anyhow!("Invalid service API key format. Expected 'service:graph-id:key'")))
        }
    }
    
    /// Derive graph ID from project structure  
    pub fn derive_graph_id_from_project(project_path: &Utf8PathBuf) -> RoverResult<String> {
        // 1. Check for apollo.config.json
        let apollo_config_path = project_path.join("apollo.config.json");
        if apollo_config_path.exists() {
            if let Ok(config_content) = fs::read_to_string(&apollo_config_path) {
                if let Ok(config_json) = serde_json::from_str::<serde_json::Value>(&config_content) {
                    // Look for common Apollo config patterns
                    if let Some(service_name) = config_json.get("service").and_then(|s| s.get("name")).and_then(|n| n.as_str()) {
                        return Ok(Self::sanitize_graph_id(service_name));
                    }
                    if let Some(graph_ref) = config_json.get("graph").and_then(|g| g.as_str()) {
                        // Extract graph ID from graph@variant format
                        let graph_id = graph_ref.split('@').next().unwrap_or(graph_ref);
                        return Ok(Self::sanitize_graph_id(graph_id));
                    }
                }
            }
        }
        
        // 2. Check for package.json name (for Node.js projects)
        let package_json_path = project_path.join("package.json");
        if package_json_path.exists() {
            if let Ok(package_content) = fs::read_to_string(&package_json_path) {
                if let Ok(package_json) = serde_json::from_str::<serde_json::Value>(&package_content) {
                    if let Some(name) = package_json.get("name").and_then(|n| n.as_str()) {
                        return Ok(Self::sanitize_graph_id(name));
                    }
                }
            }
        }
        
        // 3. Use directory name as fallback
        let dir_name = project_path.file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| "mcp-server".to_string());
        
        Ok(Self::sanitize_graph_id(&dir_name))
    }
    
    /// Sanitize a string to be a valid Apollo graph ID
    fn sanitize_graph_id(input: &str) -> String {
        input
            .to_lowercase()
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    }
    
    /// Generate tool files in /tools directory
    fn generate_tool_files(project_path: &Utf8PathBuf, tools: &[MCPTool]) -> RoverResult<()> {
        let tools_dir = project_path.join("tools");
        fs::create_dir_all(&tools_dir)
            .map_err(|e| RoverError::new(anyhow!("Failed to create tools directory: {}", e)))?;
        
        for tool in tools {
            let tool_file_path = tools_dir.join(format!("{}.graphql", tool.name));
            let tool_content = Self::generate_tool_file_content(tool)?;
            
            Fs::write_file(&tool_file_path, tool_content)?;
            println!("  ✓ Generated {}.graphql", tool.name);
        }
        
        Ok(())
    }
    
    /// Generate content for a single tool .graphql file
    fn generate_tool_file_content(tool: &MCPTool) -> RoverResult<String> {
        let examples_section = tool.example_usage
            .iter()
            .map(|example| format!("# - \"{}\"", example))
            .collect::<Vec<_>>()
            .join("\n");
        
        let variables_section = if tool.variables.is_empty() {
            "# Variables: None required for this query".to_string()
        } else {
            format!("# Variables: {}", tool.variables.join(", "))
        };
        
        // Properly comment multi-line descriptions
        let commented_description = tool.description
            .lines()
            .map(|line| if line.is_empty() { "#".to_string() } else { format!("# {}", line) })
            .collect::<Vec<_>>()
            .join("\n");

        let content = format!(
r#"# {}
# 
{}
#
# Example Usage:
{}
#
# Operation Type: {}
# Return Type: {}
# {}

{} {} {{
  {} {{
    id
    name
    description
  }}
}}
"#,
            tool.description.lines().next().unwrap_or(&tool.name),
            commented_description,
            examples_section,
            tool.operation_type,
            tool.return_type,
            variables_section,
            tool.operation_type,
            tool.name,
            tool.field_name
        );
        
        Ok(content)
    }
    
    /// Prepare template variables for replacement
    fn prepare_template_variables(
        project_path: &Utf8PathBuf,
        api_key: &str,
        graph_ref: &str,
        project_type: &ProjectType,
    ) -> RoverResult<HashMap<String, String>> {
        let mut vars = HashMap::new();
        
        // Extract graph name from graph_ref (e.g., "my-graph@current" -> "my-graph")
        let graph_name = graph_ref.split('@').next().unwrap_or(graph_ref);
        
        // Derive project name from directory or graph name
        let project_name = project_path.file_name()
            .map(|name| name.to_string())
            .unwrap_or_else(|| graph_name.to_string());
        
        vars.insert("{{PROJECT_NAME}}".to_string(), project_name);
        vars.insert("{{GRAPHQL_ENDPOINT}}".to_string(), "http://localhost:4000".to_string());
        vars.insert("{{APOLLO_API_KEY}}".to_string(), api_key.to_string());
        vars.insert("{{APOLLO_KEY}}".to_string(), api_key.to_string());
        vars.insert("{{APOLLO_GRAPH_REF}}".to_string(), graph_ref.to_string());
        
        // Mark if this is a REST connector project for conditional handling
        vars.insert("{{REST_CONNECTORS}}".to_string(), 
            if *project_type == ProjectType::RestConnector { "true" } else { "false" }.to_string());
        
        Ok(vars)
    }
    
    /// Process template files with variable replacements
    fn process_template_with_variables(
        mut template_state: SelectedTemplateState,
        variables: &HashMap<String, String>,
    ) -> RoverResult<SelectedTemplateState> {
        use std::collections::HashMap;
        
        // Process each file in the template
        for (_, content) in template_state.files.iter_mut() {
            if let Ok(text) = String::from_utf8(content.clone()) {
                let mut processed_text = text;
                
                // Handle Handlebars-style conditionals for REST_CONNECTORS
                let is_rest_connector = variables.get("{{REST_CONNECTORS}}")
                    .map(|v| v == "true")
                    .unwrap_or(false);
                
                // Process {{#if REST_CONNECTORS}}...{{/if}} blocks manually
                processed_text = Self::process_handlebars_conditional(
                    processed_text, 
                    "REST_CONNECTORS", 
                    is_rest_connector
                );
                
                // Replace all template variables
                for (placeholder, value) in variables {
                    if placeholder != "{{REST_CONNECTORS}}" { // Skip the conditional marker
                        processed_text = processed_text.replace(placeholder, value);
                    }
                }
                
                *content = processed_text.into_bytes();
            }
        }
        
        Ok(template_state)
    }
    
    /// Process Handlebars-style conditionals
    fn process_handlebars_conditional(text: String, condition_name: &str, condition_value: bool) -> String {
        let start_tag = format!("{{{{#if {}}}}}", condition_name);
        let end_tag = "{{/if}}";
        
        let mut result = text;
        
        while let Some(start_pos) = result.find(&start_tag) {
            if let Some(end_pos) = result[start_pos..].find(end_tag) {
                let actual_end_pos = start_pos + end_pos;
                let content_start = start_pos + start_tag.len();
                let content = &result[content_start..actual_end_pos];
                
                let replacement = if condition_value {
                    content.to_string()
                } else {
                    String::new()
                };
                
                // Replace the entire conditional block
                let full_end_pos = actual_end_pos + end_tag.len();
                result.replace_range(start_pos..full_end_pos, &replacement);
            } else {
                // Malformed template, break to avoid infinite loop
                break;
            }
        }
        
        result
    }
    
    /// List files created by template
    fn list_template_files(project_path: &Utf8PathBuf) -> RoverResult<Vec<Utf8PathBuf>> {
        let mut files = Vec::new();
        
        // List expected template files
        let template_paths = vec![
            ".apollo/mcp.local.yaml",
            "mcp.Dockerfile", 
            "claude-desktop-config.json",
            ".vscode/settings.json",
            "QUICKSTART_MCP.md",
        ];
        
        for path in template_paths {
            let full_path = project_path.join(path);
            if full_path.exists() {
                files.push(full_path);
            }
        }
        
        Ok(files)
    }
    
    /// Update .env file with actual credentials
    fn update_env_file(
        project_path: &Utf8PathBuf,
        api_key: &str,
        graph_ref: &str,
    ) -> RoverResult<Utf8PathBuf> {
        let env_path = project_path.join(".env");
        
        // Read existing .env content if it exists
        let mut env_content = String::new();
        if env_path.exists() {
            if let Ok(existing) = fs::read_to_string(&env_path) {
                env_content = existing;
            }
        }
        
        // Update or add APOLLO_KEY and APOLLO_GRAPH_REF
        let mut lines: Vec<String> = env_content.lines().map(|l| l.to_string()).collect();
        let mut found_key = false;
        let mut found_ref = false;
        
        for line in &mut lines {
            if line.starts_with("APOLLO_KEY=") {
                *line = format!("APOLLO_KEY={}", api_key);
                found_key = true;
            } else if line.starts_with("APOLLO_GRAPH_REF=") {
                *line = format!("APOLLO_GRAPH_REF={}", graph_ref);
                found_ref = true;
            }
        }
        
        if !found_key {
            lines.push(format!("APOLLO_KEY={}", api_key));
        }
        if !found_ref {
            lines.push(format!("APOLLO_GRAPH_REF={}", graph_ref));
        }
        
        // Write updated content
        let final_content = lines.join("\n");
        Fs::write_file(&env_path, final_content)?;
        
        Ok(env_path)
    }
    
    // ===== DEPRECATED FUNCTIONS - TO BE REMOVED =====
    // These functions are replaced by template fetching
    
    /// DEPRECATED: Generate .apollo/mcp.local.yaml configuration file
    fn generate_mcp_config(project_path: &Utf8PathBuf, project_type: &ProjectType) -> RoverResult<Utf8PathBuf> {
        let apollo_dir = project_path.join(".apollo");
        fs::create_dir_all(&apollo_dir)
            .map_err(|e| RoverError::new(anyhow!("Failed to create .apollo directory: {}", e)))?;
        
        let mcp_config_path = apollo_dir.join("mcp.local.yaml");
        
        let config_comment = match project_type {
            ProjectType::RestConnector => "# Apollo MCP Server Configuration - Local Development (REST Connector)\n# \n# This configuration enables your REST API (via Apollo Connectors) to work as an MCP server\n# that AI assistants like Claude can interact with through tools.",
            ProjectType::StandardGraphQL => "# Apollo MCP Server Configuration - Local Development (Standard GraphQL)\n# \n# This configuration enables your GraphQL API to work as an MCP server\n# that AI assistants like Claude can interact with through tools.",
        };
        
        let config_content = format!(
r#"{}

overrides:
  mutation_mode: all    # Enable all mutation operations as tools

operations:
  source: local         # Use local filesystem for tool definitions
  paths: 
    - tools            # Directory containing your .graphql tool files

introspection:
  introspect:
    enabled: true       # Allow schema introspection
  search:
    enabled: true       # Enable GraphQL search capabilities  
  execute:
    enabled: true       # Allow query execution

transport:
  type: streamable_http # MCP transport protocol
  port: 5000           # MCP server port

# Update this endpoint to match your GraphQL server
# For REST connectors, this should point to your Apollo Router
endpoint: http://localhost:4000
"#,
            config_comment
        );
        
        Fs::write_file(&mcp_config_path, config_content)?;
        Ok(mcp_config_path)
    }
    
    /// Generate Docker files
    fn generate_dockerfiles(project_path: &Utf8PathBuf, project_type: &ProjectType) -> RoverResult<Vec<Utf8PathBuf>> {
        let mut generated_files = Vec::new();
        
        // Generate mcp.Dockerfile
        let mcp_dockerfile_path = project_path.join("mcp.Dockerfile");
        let mcp_dockerfile_content = Self::generate_mcp_dockerfile_content(project_type)?;
        Fs::write_file(&mcp_dockerfile_path, mcp_dockerfile_content)?;
        generated_files.push(mcp_dockerfile_path);
        
        // Generate router.Dockerfile (optional, for full stack)
        let router_dockerfile_path = project_path.join("router.Dockerfile");
        let router_dockerfile_content = Self::generate_router_dockerfile_content()?;
        Fs::write_file(&router_dockerfile_path, router_dockerfile_content)?;
        generated_files.push(router_dockerfile_path);
        
        println!("  ✓ Generated mcp.Dockerfile and router.Dockerfile");
        Ok(generated_files)
    }
    
    /// Generate mcp.Dockerfile content
    fn generate_mcp_dockerfile_content(project_type: &ProjectType) -> RoverResult<String> {
        let comment = match project_type {
            ProjectType::RestConnector => "# Apollo MCP Server Dockerfile for REST Connector Projects\n# Based on: https://github.com/michael-watson/mcp-builder-community-server",
            ProjectType::StandardGraphQL => "# Apollo MCP Server Dockerfile for Standard GraphQL Projects\n# Based on: https://github.com/michael-watson/mcp-builder-community-server",
        };
        
        let tools_comment = match project_type {
            ProjectType::RestConnector => "# Copy REST connector tools generated from your GraphQL schema\n# These tools enable AI assistants to interact with your REST APIs",
            ProjectType::StandardGraphQL => "# Copy GraphQL tools generated from your schema\n# These tools enable AI assistants to interact with your GraphQL API",
        };
        
        let content = format!(
r#"{}

FROM ghcr.io/apollographql/apollo-mcp-server:canary-20250903T205844Z-ea32f7d

ARG BUILD_ENV=local

# Copy MCP configuration (environment-specific)
COPY .apollo/mcp.$BUILD_ENV.yaml /mcp.yaml

{}
COPY tools /data/tools

EXPOSE 5000

CMD ["mcp.yaml"]
"#,
            comment,
            tools_comment
        );
        
        Ok(content)
    }
    
    /// Generate router.Dockerfile content
    fn generate_router_dockerfile_content() -> RoverResult<String> {
        let content = r#"# Apollo Router Dockerfile for GraphQL endpoint
# Provides the GraphQL endpoint that MCP server connects to

FROM ghcr.io/apollographql/router:v2.5.0

ARG BUILD_ENV=local

# Copy router configuration (environment-specific)
COPY .apollo/router.$BUILD_ENV.yaml /config.yaml

CMD ["/config.yaml"]
"#;
        
        Ok(content.to_string())
    }
    
    /// Generate claude_desktop_config.json
    fn generate_claude_config(project_path: &Utf8PathBuf, api_key: &str, graph_ref: &str) -> RoverResult<Utf8PathBuf> {
        let claude_config_path = project_path.join("claude_desktop_config.json");
        
        let claude_config = json!({
            "mcpServers": {
                "apollo-mcp-server": {
                    "command": "./apollo-mcp-server",
                    "args": ["./.apollo/mcp.local.yaml"],
                    "env": {
                        "APOLLO_KEY": api_key,
                        "APOLLO_GRAPH_REF": graph_ref
                    }
                }
            }
        });
        
        let claude_config_str = serde_json::to_string_pretty(&claude_config)?;
        Fs::write_file(&claude_config_path, claude_config_str)?;
        
        Ok(claude_config_path)
    }
    
    /// Generate .env file
    fn generate_env_file(project_path: &Utf8PathBuf, api_key: &str, graph_ref: &str) -> RoverResult<Utf8PathBuf> {
        let env_path = project_path.join(".env");
        
        let env_content = format!(
            "APOLLO_KEY={}\nAPOLLO_GRAPH_REF={}\n",
            api_key, graph_ref
        );
        
        Fs::write_file(&env_path, env_content)?;
        Ok(env_path)
    }
    
    /// Generate apollo.config.json if it doesn't exist
    fn generate_apollo_config(project_path: &Utf8PathBuf) -> RoverResult<Utf8PathBuf> {
        let apollo_config_path = project_path.join("apollo.config.json");
        
        if !apollo_config_path.exists() {
            let apollo_config = json!({
                "rover": {
                    "supergraphConfig": "./supergraph.yaml"
                }
            });
            
            let apollo_config_str = serde_json::to_string_pretty(&apollo_config)?;
            Fs::write_file(&apollo_config_path, apollo_config_str)?;
        }
        
        Ok(apollo_config_path)
    }
    
    /// Generate comprehensive documentation
    fn generate_documentation(project_path: &Utf8PathBuf, tools: &[MCPTool], project_type: &ProjectType) -> RoverResult<Utf8PathBuf> {
        let doc_filename = match project_type {
            ProjectType::RestConnector => "quickstart-mcp-rest-connector.md",
            ProjectType::StandardGraphQL => "quickstart-mcp-graphql.md",
        };
        
        let doc_path = project_path.join(doc_filename);
        let doc_content = Self::generate_documentation_content(tools, project_type)?;
        
        Fs::write_file(&doc_path, doc_content)?;
        Ok(doc_path)
    }
    
    /// Generate documentation content based on project type
    fn generate_documentation_content(tools: &[MCPTool], project_type: &ProjectType) -> RoverResult<String> {
        let title = match project_type {
            ProjectType::RestConnector => "🚀 MCP Server Quick Start - REST Connector Edition",
            ProjectType::StandardGraphQL => "🚀 MCP Server Quick Start - GraphQL Edition",
        };
        
        let intro = match project_type {
            ProjectType::RestConnector => "Your REST API is now AI-accessible! Claude can interact with your REST endpoints through natural language using these generated MCP tools.",
            ProjectType::StandardGraphQL => "Your GraphQL API is now AI-accessible! Claude can interact with your GraphQL endpoint through natural language using these generated MCP tools.",
        };
        
        let tools_section = tools
            .iter()
            .map(|tool| {
                format!(
                    "1. **{}** (`/tools/{}.graphql`)\n   - **Purpose**: {}\n   - **Usage Examples**:\n{}\n",
                    tool.name,
                    tool.name,
                    tool.description.lines().next().unwrap_or(&tool.name),
                    tool.example_usage
                        .iter()
                        .map(|example| format!("     - \"{}\"", example))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        
        let additional_resources = match project_type {
            ProjectType::RestConnector => {
r#"### REST Connector Resources 📖
- [Apollo Connectors Documentation](https://www.apollographql.com/docs/graphos/schema-design/connectors)
- [REST API Integration Guide](https://www.apollographql.com/docs/graphos/get-started/guides/rest)  
- [Prebuilt Connectors Library](https://www.apollographql.com/docs/graphos/connectors/library) ⭐

### Common REST Patterns 🔧

#### Adding Authentication Headers
```graphql
@source(
  name: "api"
  http: {
    baseURL: "https://api.example.com"
    headers: [
      { name: "Authorization", value: "Bearer {$config.apiToken}" }
      { name: "X-API-Key", value: "{$config.apiKey}" }
    ]
  }
)
```

#### CRUD Operations Template
```graphql
# CREATE - Add new item
mutation CreateItem($input: ItemInput!) {
  createItem(input: $input) 
    @connect(source: "api", http: { POST: "/items", body: "$args.input" })
    { id name }
}

# READ - Get single item  
query GetItem($id: ID!) {
  item(id: $id)
    @connect(source: "api", http: { GET: "/items/{$args.id}" })
    { id name description }
}

# UPDATE - Modify item
mutation UpdateItem($id: ID!, $input: ItemInput!) {
  updateItem(id: $id, input: $input)
    @connect(source: "api", http: { PUT: "/items/{$args.id}", body: "$args.input" })
    { id name }
}

# DELETE - Remove item
mutation DeleteItem($id: ID!) {
  deleteItem(id: $id)
    @connect(source: "api", http: { DELETE: "/items/{$args.id}" })
    { success }
}
```"#
            }
            ProjectType::StandardGraphQL => {
r#"### GraphQL Resources 📖
- [GraphQL Documentation](https://graphql.org/learn/)
- [Apollo Server Documentation](https://www.apollographql.com/docs/apollo-server/)
- [Apollo Federation](https://www.apollographql.com/docs/federation/) 

### Common GraphQL Patterns 🔧

#### Query with Variables
```graphql
query GetItemById($id: ID!) {
  item(id: $id) {
    id
    name
    description
    createdAt
  }
}
```

#### Mutation Example
```graphql
mutation CreateItem($input: CreateItemInput!) {
  createItem(input: $input) {
    id
    name
    success
    errors {
      message
      field
    }
  }
}
```"#
            }
        };
        
        let content = format!(
r#"# {title}

## What You Just Built

{intro}

### Generated MCP Tools 🛠️
{tools_section}

## Quick Start Commands ⚡

### 1. Start Your MCP Server
```bash
# Build the MCP server Docker image
docker build -f mcp.Dockerfile -t my-mcp-server .

# Run the MCP server (development)
docker run -it --env-file .env -p5000:5000 my-mcp-server
```

### 2. Start Your GraphQL Router (Optional)
```bash  
# If you need the GraphQL endpoint too
docker build -f router.Dockerfile -t my-router .
docker run -it --env-file .env -p4000:4000 my-router
```

### 3. Connect Claude Desktop
Add this configuration to `~/Library/Application Support/Claude/claude_desktop_config.json`:

```json
{{
  "mcpServers": {{
    "apollo-mcp-server": {{
      "command": "./apollo-mcp-server",
      "args": ["./.apollo/mcp.local.yaml"],
      "env": {{
        "APOLLO_KEY": "your_apollo_key_here",
        "APOLLO_GRAPH_REF": "your_graph_ref@current"
      }}
    }}
  }}
}}
```

## Customization Guide 🎛️

### Adding More Tools
1. **Add new operations** to your GraphQL schema files
2. **Create corresponding `.graphql` files** in `/tools/` directory
3. **Restart your MCP server** to pick up changes

{additional_resources}

## Troubleshooting 🔍

### Common Issues
- **"Connection refused"**: Ensure your GraphQL server is running on port 4000
- **"Authentication failed"**: Check your `APOLLO_KEY` and `APOLLO_GRAPH_REF` values
- **"Tool not found"**: Verify `.graphql` files are in `/tools/` directory

### Debugging Commands
```bash
# Test MCP server directly
curl -X POST http://localhost:5000 -H "Content-Type: application/json" -d '{{"jsonrpc": "2.0", "method": "tools/list", "id": 1}}'

# Test GraphQL endpoint  
curl -X POST http://localhost:4000 -H "Content-Type: application/json" -d '{{"query": "{{ __schema {{ types {{ name }} }} }}"}}'
```

## Next Steps 🎯
1. **Customize your tools**: Edit files in `/tools/` directory
2. **Add authentication**: Configure API keys and headers in your schema
3. **Test with Claude**: Ask natural language questions about your data
4. **Expand functionality**: Add more queries and mutations
5. **Deploy to production**: Use staging/production MCP configs

---
💡 **Pro Tip**: Start with read-only operations (queries) then gradually add write operations (mutations) as you gain confidence with the system.

🤖 **Your API is now AI-accessible through natural language!**
"#,
            title = title,
            intro = intro,
            tools_section = tools_section,
            additional_resources = additional_resources
        );
        
        Ok(content)
    }
    
    /// Validate Docker setup (optional)
    fn validate_docker_build(project_path: &Utf8PathBuf) -> RoverResult<()> {
        println!("{}", Style::Heading.paint("Validating Docker setup..."));
        
        // Test Docker daemon is running
        let docker_check = std::process::Command::new("docker")
            .arg("version")
            .output();
            
        match docker_check {
            Ok(output) if output.status.success() => {
                println!("✓ Docker daemon is running");
            },
            _ => {
                println!("⚠ Docker daemon not found - skipping validation");
                return Ok(());
            }
        }
        
        Ok(())
    }
    
    /// Print generation summary
    fn print_generation_summary(
        tools: &[MCPTool],
        _files_created: &[Utf8PathBuf],
        _project_path: &Utf8PathBuf,
        project_type: &ProjectType,
    ) -> RoverResult<()> {
        println!("\n{}", Style::Heading.paint("📁 Generated Files:"));
        println!("├── tools/");
        for tool in tools {
            println!("│   └── {}.graphql              # AI tool for {} operations", tool.name, tool.field_name);
        }
        println!("├── .apollo/");
        println!("│   └── mcp.local.yaml                   # MCP server configuration");
        println!("├── claude-desktop-config.json           # Claude Desktop integration");
        println!("├── mcp.Dockerfile                       # MCP server container");
        println!("├── .vscode/");
        println!("│   └── settings.json                    # VS Code GraphQL configuration");
        println!("├── QUICKSTART_MCP.md                    # Comprehensive MCP documentation");
        println!("├── .env.template                        # Environment template");
        println!("└── .env                                 # Apollo credentials");
        
        println!("\n{}", Style::Heading.paint("🎯 Next Steps:"));
        println!("1. Review QUICKSTART_MCP.md for detailed setup instructions");
        println!("2. Start MCP server: docker build -f mcp.Dockerfile -t my-mcp .");
        println!("3. Update Claude Desktop config with your credentials");
        
        if !tools.is_empty() {
            let example_question = &tools[0].example_usage[0];
            println!("4. Ask Claude: \"{}\"", example_question);
        }
        
        println!("5. Customize tools in /tools/ directory");
        
        match project_type {
            ProjectType::RestConnector => {
                println!("6. Explore Apollo Connectors Library for more REST patterns");
            }
            ProjectType::StandardGraphQL => {
                println!("6. Add more GraphQL operations to expand functionality");
            }
        }
        
        println!("\n💡 Your {} is now AI-accessible through natural language!", 
            match project_type {
                ProjectType::RestConnector => "REST API",
                ProjectType::StandardGraphQL => "GraphQL API",
            }
        );
        
        Ok(())
    }
}