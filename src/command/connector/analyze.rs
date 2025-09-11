use std::path::PathBuf;
use std::str::FromStr;

use camino::Utf8PathBuf;
use clap::Parser;
use http::{HeaderMap, HeaderName, HeaderValue};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use crate::composition::supergraph::binary::SupergraphBinary;
use crate::utils::effect::exec::TokioCommand;
use crate::{RoverOutput, RoverResult};
use reqwest::{Method, Url};
use serde_json::Value;

/// Failure modes of Loading Test data from a file or command line input
#[derive(Debug, thiserror::Error)]
pub enum ParsingError {
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    #[error(transparent)]
    InvalidUrl(#[from] url::ParseError),
    #[error("Invalid method value: {0}")]
    InvalidMethod(String),
    #[error(transparent)]
    JsonSerializeError(#[from] serde_json::error::Error),
    #[error("Invalid header value: {0}")]
    InvalidHeaderData(String),
    #[error(transparent)]
    InvalidHeaderValue(#[from] http::header::InvalidHeaderValue),
    #[error(transparent)]
    InvalidHeaderName(#[from] http::header::InvalidHeaderName),
    #[error(transparent)]
    IoError(#[from] std::io::Error),
    #[error("Unexpected Content Type {0}")]
    UnexpectedContentType(String),
}

#[derive(Debug, Parser, Serialize)]
pub struct AnalyzeCurl {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser, Serialize)]
pub enum Command {
    /// Analyze a curl command
    // Boxed to reduce enum size
    Curl(Box<Curl>),
    /// Remove current analysis snapshots
    Clean(Clean),
    /// Start an interactive analysis session
    Interactive(Interactive),
}

/// Command to remove existing analysis files
#[derive(Debug, Parser, Clone, Serialize)]
pub struct Clean {}

/// Start an interactive analysis session
#[derive(Debug, Parser, Clone, Serialize)]
pub struct Interactive {
    /// The port to run the proxy server for the interactive session. Defaults to 9999.
    #[clap(short, long, value_name = "PORT")]
    port: Option<u16>,
}

#[derive(Debug, Parser, Serialize)]
pub struct Curl {
    /// Sets the endpoint to call
    url: Url,

    /// Headers to include in request
    #[clap(short='H', long, value_name = "HEADERS", num_args = 1..)]
    headers: Vec<HeaderData>,

    /// Request method to use for call
    #[clap(short = 'X', long, value_name = "REQUEST")]
    #[serde(skip_serializing)]
    method: Option<Method>,

    /// Connection timeout in seconds
    #[clap(short = 't', long, value_name = "CONNECT_TIMEOUT")]
    timeout: Option<u64>,

    /// Add JSON data to the request
    #[clap(short, long, value_name = "DATA")]
    #[arg(value_parser = parse_json)]
    data: Option<Value>,

    /// Set analysis directory to save data to
    #[clap(short, long, value_name = "ANALYSIS_DIR")]
    analysis_dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub(crate) struct HeaderData {
    pub(crate) name: HeaderName,
    pub(crate) value: HeaderValue,
}

impl Serialize for HeaderData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("HeaderData", 2)?;
        state.serialize_field("name", &self.name.to_string())?;
        state.serialize_field("value", &self.value.to_str().unwrap_or_default())?;
        state.end()
    }
}

impl HeaderData {
    #[allow(dead_code)]
    pub(crate) fn from_header_map(headers: &HeaderMap) -> Vec<Self> {
        headers
            .iter()
            .filter(|(name, _)| !IGNORED_HEADERS.contains(&name.as_str()))
            .map(|(k, v)| HeaderData {
                name: k.clone(),
                value: v.clone(),
            })
            .collect()
    }
}

impl FromStr for HeaderData {
    type Err = ParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some((key, value)) = s.split_once(':') {
            let name = HeaderName::from_str(key)?;
            let value = HeaderValue::from_str(value)?;
            Ok(Self { name, value })
        } else {
            Err(ParsingError::InvalidHeaderData(s.to_string()))
        }
    }
}

fn parse_json(arg: &str) -> Result<Value, serde_json::Error> {
    let data: String = arg.parse().unwrap_or_default();

    serde_json::from_str(&data)
}

impl AnalyzeCurl {
    pub async fn run(&self, supergraph_binary: SupergraphBinary) -> RoverResult<RoverOutput> {
        let exec_command_impl = TokioCommand::default();
        // self.output_dir.as_ref()
        // .and_then(|path| camino::Utf8PathBuf::from_path_buf(path.to_path_buf()).ok())
        let result = match &self.command {
            Command::Curl(curl) =>
            // supergraph_binary
            //     .analyze_curl(
            //         &exec_command_impl,
            //         curl.analysis_dir,
            //         curl.data,
            //         curl.headers,
            //         curl.method,
            //         curl.timeout,
            //         curl.url
            //     )
            //     .await?,
            {
                todo!()
            }
            Command::Clean(_) => supergraph_binary.analyze_clean(&exec_command_impl).await?,
            Command::Interactive(interactive) => {
                supergraph_binary
                    .analyze_interactive(&exec_command_impl, interactive.port)
                    .await?
            }
        };
        Ok(result)
    }
}

static IGNORED_HEADERS: &[&str] = &[
    "content-length",
    "content-range",
    "trailer",
    "transfer-encoding",
    "content-type",
    "content-encoding",
    "content-location",
    "content-language",
    "accept-patch",
    "accept-ranges",
    "age",
    "allow",
    "alt-svc",
    "cache-control",
    "connection",
    "date",
    "delta-base",
    "etag",
    "expires",
    "im",
    "last-modified",
    "host",
    "location",
    "link",
    "pragma",
    "proxy-authenticate",
    "public-key-pins",
    "retry-after",
    "server",
    "set-cookie",
    "strict-transport-security",
    "tk",
    "upgrade",
    "vary",
    "via",
    "warning",
    "www-authenticate",
    "proxy-connection",
    "user-agent",
    "accept",
    "accept-encoding",
];
