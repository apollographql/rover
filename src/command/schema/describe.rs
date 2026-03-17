use std::{
    path::{Path, PathBuf},
    str::FromStr,
};

use clap::Parser;
use rover_client::{
    operations::{
        graph::introspect::GraphIntrospect,
        supergraph::fetch::{SupergraphFetch, SupergraphFetchInput, SupergraphFetchRequest},
    },
    shared::GraphRef,
};
use rover_graphql::GraphQLLayer;
use rover_http::{
    HttpRequest, HttpResponse,
    retry::RetryPolicy,
    timeout::{Timeout, TimeoutLayer},
};
use rover_schema::{
    ParsedSchema, SchemaCoordinate, describe,
    format::{OutputFormat, compact, description, is_tty, sdl},
};
use rover_std::Fs;
use serde::Serialize;
use tower::{Service, ServiceBuilder, ServiceExt, retry::RetryLayer};
use url::Url;

use crate::{RoverOutput, RoverResult, options::ProfileOpt, utils::client::StudioClientConfig};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Human-readable description (default for TTY)
    Description,
    /// Token-efficient compact notation (default for piped output)
    Compact,
    /// Raw SDL
    Sdl,
}

#[derive(Debug, Serialize, Parser)]
/// Describe a graph's schema by type or field
///
/// Displays a structured description of your graph's schema. Start with an
/// overview, then zoom into individual types and fields.
///
/// Piped output defaults to --view compact.
#[command(after_help = "EXAMPLES:\n    \
        rover describe <SCHEMA_SOURCE>\n    \
        rover describe <SCHEMA_SOURCE> --path Post\n    \
        rover describe <SCHEMA_SOURCE> --path Post --depth 1\n    \
        rover describe <SCHEMA_SOURCE> --path User.posts\n    \
        rover describe <SCHEMA_SOURCE> --path Post --view sdl")]
pub struct Describe {
    /// <NAME>@<VARIANT>[:<COORDINATE>]
    ///
    /// graph@variant            Schema overview
    /// graph@variant:Type       Describe a type
    /// graph@variant:Type.field Describe a field
    #[arg(value_name = "SCHEMA_SOURCE", value_parser = clap::value_parser!(SchemaSource))]
    #[serde(skip_serializing)]
    schema_source: SchemaSource,

    #[arg(short = "p", long = "path", value_name = "SCHEMA_PATH", value_parse = clap::value_parser!(SchemaCoordinate))]
    schema_path: Option<SchemaCoordinate>,

    /// Expand referenced types N levels deep
    #[arg(long = "depth", short = 'd', default_value_t = 0)]
    depth: usize,

    /// Show deprecated fields and types
    #[arg(long = "include-deprecated")]
    include_deprecated: bool,

    /// Select output view: description (default TTY), compact (default piped), or sdl
    #[arg(long = "view", short = 'v', value_name = "VIEW")]
    view: Option<ViewMode>,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Describe {
    pub async fn run<S>(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput>
    where
        S: Service<HttpRequest, Response = HttpResponse>,
        S::Error: std::error::Error + Send + 'static,
        S::Future: Send,
    {
        let sdl_string = self.fetch_sdl(&client_config).await?;
        let output_format = self.output_format(is_tty());

        // Parse
        let parsed = ParsedSchema::parse(&sdl_string);
        let schema = parsed.inner();

        // If --sdl, output filtered SDL
        if output_format == OutputFormat::Sdl {
            let sdl_output = sdl::filtered_sdl(coordinate.as_ref(), &sdl_string);
            return Ok(RoverOutput::DescribeResponse {
                content: sdl_output,
                json_data: serde_json::Value::Null,
            });
        }

        // Generate describe result
        let result = match &coordinate {
            None => {
                let overview = describe::overview(schema, &graph_ref.to_string());
                describe::DescribeResult::Overview(overview)
            }
            Some(SchemaCoordinate::Type(tc)) => {
                let detail = describe::type_detail(
                    schema,
                    tc.ty.as_str(),
                    self.include_deprecated,
                    self.depth,
                )
                .map_err(|e| anyhow::anyhow!("{}", e))?;
                describe::DescribeResult::TypeDetail(detail)
            }
            Some(coord @ SchemaCoordinate::TypeAttribute(_)) => {
                let detail =
                    describe::field_detail(schema, coord).map_err(|e| anyhow::anyhow!("{}", e))?;
                describe::DescribeResult::FieldDetail(detail)
            }
            Some(other) => {
                return Err(
                    anyhow::anyhow!("unsupported coordinate for describe: '{other}'").into(),
                );
            }
        };

        let json_data = serde_json::to_value(&result).unwrap_or(serde_json::Value::Null);

        let content = match output_format {
            OutputFormat::Description => description::format_describe(&result),
            OutputFormat::Compact => compact::format_describe_compact(&result),
            OutputFormat::Sdl => unreachable!(),
        };

        Ok(RoverOutput::DescribeResponse { content, json_data })
    }

    fn output_format(&self, is_tty: bool) -> OutputFormat {
        match self.view {
            Some(ViewMode::Sdl) => OutputFormat::Sdl,
            Some(ViewMode::Compact) => OutputFormat::Compact,
            Some(ViewMode::Description) => OutputFormat::Description,
            None => {
                if is_tty {
                    OutputFormat::Description
                } else {
                    OutputFormat::Compact
                }
            }
        }
    }

    async fn fetch_sdl(&self, client_config: &StudioClientConfig) -> RoverResult<String> {
        match &self.schema_source {
            SchemaSource::GraphOS(graph_ref) => {
                let http_service = client_config.authenticated_service(&self.profile)?;
                let client_timeout = client_config.client_timeout().get_duration();
                let graphql_service = ServiceBuilder::new()
                    .layer(GraphQLLayer::default())
                    .layer(RetryLayer::new(RetryPolicy::new(client_timeout * 5)))
                    .layer(TimeoutLayer::new(client_timeout))
                    .service(http_service);
                let mut fetch = SupergraphFetch::new(graphql_service);
                let resp = fetch
                    .ready()
                    .await
                    .map_err(|e| anyhow::Error::from(e))?
                    .call(SupergraphFetchRequest::new(graph_ref.clone()))
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(resp.sdl.contents)
            }
            SchemaSource::File(path) => {
                let utf8_path = camino::Utf8PathBuf::try_from(path.clone()).map_err(|p| {
                    anyhow::anyhow!("path '{}' contains invalid UTF-8", p.display())
                })?;
                Ok(Fs::read_file(utf8_path)?)
            }
            SchemaSource::Url(url) => {
                let http_service = client_config.service()?;
                let mut service = ServiceBuilder::new()
                    .layer_fn(GraphIntrospect::new)
                    .layer(GraphQLLayer::new(url.clone()))
                    .service(http_service);
                let resp = service
                    .ready()
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?
                    .call(())
                    .await
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                Ok(resp.schema_sdl)
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("Invalid schema source")]
pub struct InvalidSchemaSource;

#[derive(Debug, Clone)]
pub enum SchemaSource {
    GraphOS(GraphRef),
    File(PathBuf),
    Url(Url),
}

impl FromStr for SchemaSource {
    type Err = InvalidSchemaSource;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        GraphRef::from_str(s)
            .map(SchemaSource::GraphOS)
            .map_err(|_| InvalidSchemaSource)
            .or_else(|_| {
                Url::parse(s)
                    .map_err(|_| InvalidSchemaSource)
                    .and_then(|url| {
                        if matches!(url.scheme(), "http" | "https") {
                            Ok(SchemaSource::Url(url))
                        } else {
                            Err(InvalidSchemaSource)
                        }
                    })
            })
            .or_else(|_| {
                let path = Path::new(s);
                if path.exists() {
                    Ok(SchemaSource::File(path.to_path_buf()))
                } else {
                    Err(InvalidSchemaSource)
                }
            })
    }
}

/// Parse a combined `GRAPH_REF:COORDINATE` string.
///
/// The heuristic: find the `@variant` portion first. Then look for a `:` after
/// the variant where the text following starts with an uppercase letter (GraphQL
/// type names are PascalCase). This handles variants that contain `:`.
fn parse_graph_ref_and_coordinate(
    input: &str,
) -> RoverResult<(GraphRef, Option<SchemaCoordinate>)> {
    // Find the @ that separates graph name from variant
    let at_pos = input.find('@');

    if let Some(at_pos) = at_pos {
        // Look for a coordinate after the variant
        let after_at = &input[at_pos + 1..];

        // Find the last `:` where the following char is uppercase (a type name)
        let mut coord_split = None;
        for (i, _) in after_at.match_indices(':') {
            let remaining = &after_at[i + 1..];
            if remaining.starts_with(|c: char| c.is_ascii_uppercase()) {
                coord_split = Some(at_pos + 1 + i);
                break;
            }
        }

        if let Some(split_pos) = coord_split {
            let graph_ref_str = &input[..split_pos];
            let coord_str = &input[split_pos + 1..];
            let graph_ref =
                GraphRef::from_str(graph_ref_str).map_err(|e| anyhow::anyhow!("{}", e))?;
            let coordinate = coord_str
                .parse::<SchemaCoordinate>()
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok((graph_ref, Some(coordinate)))
        } else {
            // No coordinate — entire string is the graph ref
            let graph_ref = GraphRef::from_str(input).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok((graph_ref, None))
        }
    } else {
        // No @, but check for a coordinate (`:` followed by uppercase type name)
        if let Some(colon_pos) = input.find(':') {
            let remaining = &input[colon_pos + 1..];
            if remaining.starts_with(|c: char| c.is_ascii_uppercase()) {
                let graph_ref_str = &input[..colon_pos];
                let coord_str = remaining;
                let graph_ref =
                    GraphRef::from_str(graph_ref_str).map_err(|e| anyhow::anyhow!("{}", e))?;
                let coordinate = coord_str
                    .parse::<SchemaCoordinate>()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                return Ok((graph_ref, Some(coordinate)));
            }
        }
        // No coordinate — entire string is the graph ref (gets default variant)
        let graph_ref = GraphRef::from_str(input).map_err(|e| anyhow::anyhow!("{}", e))?;
        Ok((graph_ref, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_graph_ref_only() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph@current").unwrap();
        assert_eq!(graph_ref.graph_id(), "my-graph");
        assert_eq!(graph_ref.variant(), "current");
        assert!(coord.is_none());
    }

    #[test]
    fn parse_graph_ref_with_type() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph@current:Post").unwrap();
        assert_eq!(graph_ref.graph_id(), "my-graph");
        assert_eq!(graph_ref.variant(), "current");
        assert_eq!(coord, Some("Post".parse::<SchemaCoordinate>().unwrap()));
    }

    #[test]
    fn parse_graph_ref_with_field() {
        let (graph_ref, coord) =
            parse_graph_ref_and_coordinate("my-graph@current:User.posts").unwrap();
        assert_eq!(graph_ref.graph_id(), "my-graph");
        assert_eq!(graph_ref.variant(), "current");
        assert_eq!(
            coord,
            Some("User.posts".parse::<SchemaCoordinate>().unwrap())
        );
    }

    #[test]
    fn parse_graph_ref_no_variant() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("mygraph").unwrap();
        assert_eq!(graph_ref.graph_id(), "mygraph");
        assert_eq!(graph_ref.variant(), "current");
        assert!(coord.is_none());
    }

    #[test]
    fn parse_graph_ref_no_variant_with_type() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph:Post").unwrap();
        assert_eq!(graph_ref.graph_id(), "my-graph");
        assert_eq!(graph_ref.variant(), "current");
        assert_eq!(coord, Some("Post".parse::<SchemaCoordinate>().unwrap()));
    }

    #[test]
    fn parse_graph_ref_no_variant_with_field() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph:User.posts").unwrap();
        assert_eq!(graph_ref.graph_id(), "my-graph");
        assert_eq!(graph_ref.variant(), "current");
        assert_eq!(
            coord,
            Some("User.posts".parse::<SchemaCoordinate>().unwrap())
        );
    }
}
