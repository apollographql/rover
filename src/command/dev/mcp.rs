use crate::command::dev::router::config::RouterAddress;
use clap::Parser;
use http::header::{InvalidHeaderName, InvalidHeaderValue};
use http::{HeaderMap, HeaderName, HeaderValue};
use mcp_apollo_server::errors::ServerError;
use mcp_apollo_server::operations::OperationSource;
use mcp_apollo_server::server::ManifestSource;
use mcp_apollo_server::server::UplinkConfig;
use mcp_apollo_server::server::{SchemaSource, Server, Transport};
use secrecy::SecretString;
use serde::Serialize;
use std::net::AddrParseError;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use std::{env, io};

#[derive(Debug, Clone, Serialize, Parser)]
pub struct Opts {
    /// Enable the MCP server
    #[arg(long = "mcp")]
    pub enabled: bool,

    /// Start the server using the SSE transport on the given port
    #[arg(long = "mcp-port")]
    port: Option<u16>,

    /// Expose the schema to the MCP client through `schema` and `execute` tools - defaults to true
    #[arg(long = "mcp-introspection")]
    introspection: bool,

    /// Expose a tool to open queries in Apollo Explorer
    #[arg(long = "mcp-explorer")]
    explorer: bool,

    /// Enable use of uplink to get the schema and persisted queries
    #[arg(long = "mcp-uplink")]
    uplink: bool,

    /// Operation files to expose as MCP tools
    #[arg(long = "mcp-operations", num_args=0..)]
    operations: Vec<PathBuf>,

    /// Headers to send to the endpoint
    #[arg(long = "mcp-header", action = clap::ArgAction::Append)]
    headers: Vec<String>,

    /// The path to the persisted query manifest containing operations
    #[arg(long = "mcp-manifest")]
    manifest: Option<PathBuf>,
}

/// Serve the MCP server at the specified address, proxying requests to the router address
pub(crate) async fn serve<P: AsRef<Path>>(
    options: &Opts,
    router_address: RouterAddress,
    schema_path: P,
) -> Result<(), Error> {
    let port = options.port.unwrap_or(5000_u16);
    let transport = Transport::SSE { port };

    let schema_source = SchemaSource::File {
        path: PathBuf::from(schema_path.as_ref()),
        watch: true,
    };

    let mut default_headers = HeaderMap::new();
    for header in options.headers.clone() {
        let parts: Vec<&str> = header.split(':').map(|s| s.trim()).collect();
        match (parts.first(), parts.get(1), parts.get(2)) {
            (Some(key), Some(value), None) => {
                default_headers.append(HeaderName::from_str(key)?, HeaderValue::from_str(value)?);
            }
            _ => return Err(Error::Header(header)),
        }
    }

    let operation_source = if let Some(manifest) = options.manifest.clone() {
        OperationSource::Manifest(ManifestSource::LocalHotReload(vec![manifest]))
    } else if options.uplink {
        OperationSource::Manifest(ManifestSource::Uplink(uplink_config()?))
    } else if !options.operations.is_empty() {
        OperationSource::Files(options.operations.clone())
    } else {
        if !options.introspection {
            return Err(Error::NoOperations);
        }
        OperationSource::None
    };

    let introspection = options.introspection;

    tokio::spawn(async move {
        Server::builder()
            .transport(transport)
            .schema_source(schema_source)
            .operation_source(operation_source)
            .headers(default_headers)
            .endpoint(router_address.pretty_string())
            .introspection(introspection)
            .explorer(true)
            .build()
            .start()
            .await
    });

    eprintln!("MCP server running at http://127.0.0.1:{port}");
    Ok(())
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("MCP server error: {0}")]
    Io(#[from] io::Error),

    #[error("Invalid address in MCP URL: {0}")]
    InvalidUrlAddress(#[from] AddrParseError),

    #[error("MCP server error: {0}")]
    Server(#[from] ServerError),

    #[error("Invalid header value: {0}")]
    HeaderValue(#[from] InvalidHeaderValue),

    #[error("Invalid header name: {0}")]
    HeaderName(#[from] InvalidHeaderName),

    #[error("Invalid header: {0}")]
    Header(String),

    #[error("No operations defined")]
    NoOperations,
}

fn uplink_config() -> Result<UplinkConfig, ServerError> {
    Ok(UplinkConfig {
        apollo_key: SecretString::from(
            env::var("APOLLO_KEY")
                .map_err(|_| ServerError::EnvironmentVariable(String::from("APOLLO_KEY")))?,
        ),
        apollo_graph_ref: env::var("APOLLO_GRAPH_REF")
            .map_err(|_| ServerError::EnvironmentVariable(String::from("APOLLO_GRAPH_REF")))?,
        poll_interval: Duration::from_secs(10),
        timeout: Duration::from_secs(30),
        endpoints: None, // Use the default endpoints
    })
}
