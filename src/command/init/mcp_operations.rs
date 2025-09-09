use std::fs;
use std::io::{self, Cursor};
use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use rover_std::{Fs, Style};
use crate::RoverResult;

const APOLLO_MCP_SERVER_BINARY_NAME: &str = "apollo-mcp-server";

#[derive(Debug)]
pub struct MCPSetupResult {
    pub binary_path: Utf8PathBuf,
    pub env_file: Utf8PathBuf,
    pub claude_config: Option<Utf8PathBuf>,
}

pub struct MCPOperations;

impl MCPOperations {
    pub fn setup_mcp_project(
        project_path: &Utf8PathBuf,
        api_key: &str,
        graph_ref: &str,
    ) -> RoverResult<MCPSetupResult> {
        println!("{}", Style::Heading.paint("Setting up MCP server..."));

        // Download apollo-mcp-server binary
        let binary_path = Self::download_apollo_mcp_server(project_path)?;
        
        // Generate .env file
        let env_file = Self::generate_env_file(project_path, api_key, graph_ref)?;
        
        // Check Node version and optionally generate Claude Desktop config
        let claude_config = Self::setup_claude_desktop_config(project_path, api_key, graph_ref)?;

        Ok(MCPSetupResult {
            binary_path,
            env_file,
            claude_config,
        })
    }

    fn download_apollo_mcp_server(project_path: &Utf8PathBuf) -> RoverResult<Utf8PathBuf> {
        let binary_path = project_path.join(APOLLO_MCP_SERVER_BINARY_NAME);
        
        // Detect OS and architecture
        let download_url = Self::get_download_url()?;
        
        println!("{}", Style::Heading.paint("Downloading apollo-mcp-server..."));
        
        // Use tokio::task::spawn_blocking to run blocking code from async context
        let download_url_clone = download_url.clone();
        let binary_path_clone = binary_path.clone();
        
        let result = std::thread::spawn(move || {
            let client = reqwest::blocking::Client::new();
            let response = client.get(&download_url_clone)
                .send()
                .map_err(|e| anyhow!("Failed to download apollo-mcp-server: {}", e))?;
            
            let bytes = response.bytes()
                .map_err(|e| anyhow!("Failed to read response: {}", e))?;
            
            // Extract tar.gz file
            let cursor = Cursor::new(bytes);
            let tar = flate2::read::GzDecoder::new(cursor);
            let mut archive = tar::Archive::new(tar);
            
            // Extract files to a temporary directory first
            let project_dir = binary_path_clone.parent().unwrap();
            let temp_extract_dir = project_dir.join("temp_apollo_mcp");
            
            if temp_extract_dir.exists() {
                fs::remove_dir_all(&temp_extract_dir).ok();
            }
            fs::create_dir_all(&temp_extract_dir)
                .map_err(|e| anyhow!("Failed to create temp directory: {}", e))?;
            
            archive.unpack(&temp_extract_dir)
                .map_err(|e| anyhow!("Failed to extract archive: {}", e))?;
            
            // Find the apollo-mcp-server binary in the extracted files
            let mut binary_found = false;
            
            // Check in dist/ subdirectory first
            let dist_dir = temp_extract_dir.join("dist");
            let binary_locations = vec![
                dist_dir.join("apollo-mcp-server"),
                dist_dir.join("apollo-mcp-server.exe"),
                temp_extract_dir.join("apollo-mcp-server"),
                temp_extract_dir.join("apollo-mcp-server.exe"),
            ];
            
            for potential_binary in binary_locations {
                if potential_binary.exists() && potential_binary.is_file() {
                    fs::copy(&potential_binary, &binary_path_clone)
                        .map_err(|e| anyhow!("Failed to copy binary: {}", e))?;
                    binary_found = true;
                    break;
                }
            }
            
            // Clean up temp directory
            fs::remove_dir_all(&temp_extract_dir).ok();
            
            if !binary_found {
                return Err(anyhow!("apollo-mcp-server binary not found in archive"));
            }
                
            Ok::<(), anyhow::Error>(())
        }).join().map_err(|_| anyhow!("Download thread panicked"))?;
        
        result?;

        // Make executable on Unix systems
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&binary_path)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&binary_path, perms)?;
        }

        println!("{} apollo-mcp-server downloaded to {}", Style::Success.paint("✓"), binary_path);
        
        Ok(binary_path)
    }

    fn get_download_url() -> Result<String> {
        let os = std::env::consts::OS;
        let arch = std::env::consts::ARCH;
        
        let version = "v0.7.5"; // Pin to stable version
        
        let target = match (os, arch) {
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("windows", "aarch64") => "aarch64-pc-windows-msvc",
            ("windows", "x86_64") => "x86_64-pc-windows-msvc",
            _ => return Err(anyhow!("Unsupported platform: {} {}", os, arch)),
        };
        
        Ok(format!(
            "https://github.com/apollographql/apollo-mcp-server/releases/download/{}/apollo-mcp-server-{}-{}.tar.gz",
            version, version, target
        ))
    }

    fn generate_env_file(
        project_path: &Utf8PathBuf,
        api_key: &str,
        graph_ref: &str,
    ) -> RoverResult<Utf8PathBuf> {
        let env_path = project_path.join(".env");
        
        let env_content = format!(
            "APOLLO_KEY={}\nAPOLLO_GRAPH_REF={}\n",
            api_key, graph_ref
        );
        
        Fs::write_file(&env_path, env_content)?;
        
        println!("{} .env file created", Style::Success.paint("✓"));
        
        Ok(env_path)
    }

    pub async fn publish_minimal_schema(
        client: &rover_client::blocking::StudioClient,
        graph_ref: &rover_client::shared::GraphRef,
    ) -> RoverResult<()> {
        println!("{}", Style::Heading.paint("Publishing minimal schema to Apollo Studio..."));
        
        // Create a minimal federated schema that MCP server can use
        let minimal_schema = r#"
extend schema @link(url: "https://specs.apollo.dev/federation/v2.11", import: ["@key", "@shareable"])

type Query {
  _service: _Service
}

type _Service {
  sdl: String!
}
"#;

        use rover_client::operations::subgraph::publish::*;
        use rover_client::shared::GitContext;
        
        rover_client::operations::subgraph::publish::run(
            SubgraphPublishInput {
                graph_ref: graph_ref.clone(),
                subgraph: "mcp-placeholder".to_string(),
                url: Some("http://localhost:4000".to_string()),
                schema: minimal_schema.to_string(),
                git_context: GitContext {
                    branch: None,
                    commit: None,
                    author: None,
                    remote_url: None,
                },
                convert_to_federated_graph: false,
            },
            client,
        )
        .await?;
        
        println!("{} Minimal schema published for MCP server", Style::Success.paint("✓"));
        Ok(())
    }

    pub fn compose_supergraph_schema(project_path: &Utf8PathBuf) -> RoverResult<()> {
        println!("{}", Style::Heading.paint("Composing supergraph from connectors..."));
        
        let connectors_dir = project_path.join("connectors");
        let supergraph_config = connectors_dir.join("supergraph.yaml");
        let output_schema = project_path.join("supergraph-schema.graphql");
        
        if !supergraph_config.exists() {
            return Err(anyhow!("supergraph.yaml not found in connectors directory").into());
        }
        
        // Run rover supergraph compose command
        let output = std::process::Command::new("rover")
            .args(&[
                "supergraph", 
                "compose",
                "--config", supergraph_config.as_str()
            ])
            .current_dir(project_path)
            .output()
            .map_err(|e| anyhow!("Failed to run rover supergraph compose: {}", e))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("Supergraph composition failed: {}", stderr).into());
        }
        
        // Write the composed schema to file
        let composed_schema = String::from_utf8_lossy(&output.stdout);
        Fs::write_file(&output_schema, composed_schema.to_string())?;
        
        println!("{} Supergraph schema composed and saved to supergraph-schema.graphql", Style::Success.paint("✓"));
        
        Ok(())
    }

    fn setup_claude_desktop_config(project_path: &Utf8PathBuf, api_key: &str, graph_ref: &str) -> RoverResult<Option<Utf8PathBuf>> {
        // Check Node version
        if !Self::check_node_version()? {
            println!(
                "{} Node.js 18+ required for Claude Desktop integration. Skipping claude_desktop_config.json generation.", 
                Style::WarningHeading.paint("⚠")
            );
            return Ok(None);
        }

        let claude_config_path = project_path.join("claude_desktop_config.json");
        let binary_path = project_path.join(APOLLO_MCP_SERVER_BINARY_NAME);
        let mcp_config_path = project_path.join(".apollo").join("mcp.local.yaml");
        
        let claude_config = serde_json::json!({
            "mcpServers": {
                "apollo-mcp-server": {
                    "command": binary_path.as_str(),
                    "args": [mcp_config_path.as_str()],
                    "env": {
                        "APOLLO_KEY": api_key,
                        "APOLLO_GRAPH_REF": graph_ref
                    }
                }
            }
        });
        
        let claude_config_str = serde_json::to_string_pretty(&claude_config)?;
        Fs::write_file(&claude_config_path, claude_config_str)?;
        
        println!("{} claude_desktop_config.json created", Style::Success.paint("✓"));
        
        Ok(Some(claude_config_path))
    }

    fn check_node_version() -> RoverResult<bool> {
        let output = std::process::Command::new("node")
            .arg("--version")
            .output();
            
        match output {
            Ok(output) => {
                if output.status.success() {
                    let version_str = String::from_utf8_lossy(&output.stdout);
                    let version = version_str.trim().strip_prefix('v').unwrap_or(&version_str);
                    
                    // Parse major version
                    if let Some(major_str) = version.split('.').next() {
                        if let Ok(major) = major_str.parse::<u32>() {
                            return Ok(major >= 18);
                        }
                    }
                }
            }
            Err(_) => {
                println!("{} Node.js not found", Style::WarningHeading.paint("⚠"));
                return Ok(false);
            }
        }
        
        Ok(false)
    }

    pub fn display_mcp_success_message(
        project_name: String,
        setup_result: &MCPSetupResult,
        graph_ref: &str,
    ) {
        println!("\n{}", Style::Success.paint("🎉 MCP server project created successfully!"));
        println!("\n{}: {}", Style::Heading.paint("Project"), project_name);
        println!("{}: {}", Style::Heading.paint("Graph"), graph_ref);
        println!("{}: {}", Style::Heading.paint("Binary"), setup_result.binary_path);
        
        println!("\n{}", Style::Heading.paint("Next steps:"));
        println!("  {} Configure API keys for your connectors:", Style::Command.paint("1."));
        println!("     • AWS: Set up AWS credentials for Lambda/DynamoDB access");
        println!("     • Luma: Add your Luma API key to router configuration");
        println!("     • Update .apollo/router.local.yaml with your API keys");
        
        println!("  {} Start the MCP Server:", Style::Command.paint("2."));
        println!("     export $(cat .env | xargs)");
        println!("     ./apollo-mcp-server .apollo/mcp.local.yaml");
        println!("     (Server will start on http://127.0.0.1:5000)");
        
        println!("  {} Start local development (in another terminal):", Style::Command.paint("3."));
        println!("     export $(cat .env | xargs)");
        println!("     APOLLO_ROVER_DEV_ROUTER_VERSION=2.6.0 rover dev --supergraph-config connectors/supergraph.yaml");
        
        println!("  {} Test GraphQL with Apollo Sandbox:", Style::Command.paint("4."));
        println!("     Open http://localhost:4000 to query your connectors");
        
        println!("  {} Test MCP Server with Inspector:", Style::Command.paint("5."));
        println!("     npx @modelcontextprotocol/inspector");
        println!("     - Transport: Streamable HTTP");
        println!("     - URL: http://127.0.0.1:5000/mcp");
        
        println!("  {} Docker deployment (recommended for production):", Style::Command.paint("6."));
        println!("     docker build --tag mcp-server -f mcp.Dockerfile .");
        println!("     docker build --tag mcp-router -f router.Dockerfile .");
        println!("     docker run -it --env-file .env -p5000:5000 mcp-server");
        println!("     docker run -it --env-file .env -p4000:4000 mcp-router");
        
        if setup_result.claude_config.is_some() {
            println!("  {} Claude Desktop setup:", Style::Command.paint("7."));
            println!("     • Install Claude Desktop from https://claude.ai/download");
            println!("     • Ensure Node.js 18+ is installed and in your PATH");
            println!("     • Copy claude_desktop_config.json to Claude's config directory");
            println!("     • Restart Claude Desktop to load the MCP server");
            println!("     • See: https://www.apollographql.com/docs/apollo-mcp-server/quickstart#step-4-connect-claude-desktop");
        }
        
        println!("\n{} Graph created for local development!", Style::Success.paint("ℹ"));
        println!("   Connectors run locally only - for production, deploy as Docker containers.");
    }
}