use clap::Parser;
use rover_client::shared::GraphRef;
use rover_schema::{
    ParsedSchema, SchemaCoordinate, describe,
    format::{self, OutputFormat, compact, description, sdl},
};
use serde::Serialize;
use std::str::FromStr;

use crate::{
    RoverOutput, RoverResult, command::schema_cache, options::ProfileOpt,
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
/// Describe a graph's schema by type or field
///
/// Displays a structured description of your graph's schema. Start with an
/// overview, then zoom into individual types and fields.
///
/// Piped output defaults to --compact.
#[command(after_help = "EXAMPLES:\n    \
        rover describe my-graph@my-variant\n    \
        rover describe my-graph@my-variant:Post\n    \
        rover describe my-graph@my-variant:Post --depth 1\n    \
        rover describe my-graph@my-variant:User.posts\n    \
        rover describe my-graph@my-variant:Post --sdl")]
pub struct Describe {
    /// <NAME>@<VARIANT>[:<COORDINATE>]
    ///
    /// graph@variant            Schema overview
    /// graph@variant:Type       Describe a type
    /// graph@variant:Type.field Describe a field
    #[arg(value_name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph_ref_and_coord: String,

    /// Expand referenced types N levels deep
    #[arg(long = "depth", short = 'd', default_value_t = 0)]
    depth: usize,

    /// Show deprecated fields and types
    #[arg(long = "include-deprecated")]
    include_deprecated: bool,

    /// Output raw SDL
    #[arg(long = "sdl")]
    sdl: bool,

    /// Output token-efficient compact notation
    #[arg(long = "compact")]
    compact: bool,

    /// Skip reading from the local schema cache (still writes to cache)
    #[arg(long = "no-cache")]
    no_cache: bool,

    #[clap(flatten)]
    profile: ProfileOpt,
}

impl Describe {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let (graph_ref, coordinate) = parse_graph_ref_and_coordinate(&self.graph_ref_and_coord)?;

        // Fetch SDL (with caching)
        let sdl_string = schema_cache::fetch_sdl_cached(
            &graph_ref,
            &self.profile,
            &client_config,
            self.no_cache,
        )
        .await?;

        // Parse
        let parsed = ParsedSchema::parse(&sdl_string).map_err(|e| anyhow::anyhow!("{}", e))?;
        let schema = parsed.inner();

        // Determine output format
        let output_format = format::select_format(self.sdl, self.compact, false);

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
            Some(SchemaCoordinate::Type(type_name)) => {
                let detail =
                    describe::type_detail(schema, type_name, self.include_deprecated, self.depth)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                describe::DescribeResult::TypeDetail(detail)
            }
            Some(coord @ SchemaCoordinate::Field { .. }) => {
                let detail =
                    describe::field_detail(schema, coord).map_err(|e| anyhow::anyhow!("{}", e))?;
                describe::DescribeResult::FieldDetail(detail)
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
            let coordinate =
                SchemaCoordinate::parse(coord_str).map_err(|e| anyhow::anyhow!("{}", e))?;
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
                let coordinate =
                    SchemaCoordinate::parse(coord_str).map_err(|e| anyhow::anyhow!("{}", e))?;
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
        assert_eq!(graph_ref.name, "my-graph");
        assert_eq!(graph_ref.variant, "current");
        assert!(coord.is_none());
    }

    #[test]
    fn parse_graph_ref_with_type() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph@current:Post").unwrap();
        assert_eq!(graph_ref.name, "my-graph");
        assert_eq!(graph_ref.variant, "current");
        assert_eq!(coord, Some(SchemaCoordinate::Type("Post".into())));
    }

    #[test]
    fn parse_graph_ref_with_field() {
        let (graph_ref, coord) =
            parse_graph_ref_and_coordinate("my-graph@current:User.posts").unwrap();
        assert_eq!(graph_ref.name, "my-graph");
        assert_eq!(graph_ref.variant, "current");
        assert_eq!(
            coord,
            Some(SchemaCoordinate::Field {
                type_name: "User".into(),
                field_name: "posts".into()
            })
        );
    }

    #[test]
    fn parse_graph_ref_no_variant() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("mygraph").unwrap();
        assert_eq!(graph_ref.name, "mygraph");
        assert_eq!(graph_ref.variant, "current");
        assert!(coord.is_none());
    }

    #[test]
    fn parse_graph_ref_no_variant_with_type() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph:Post").unwrap();
        assert_eq!(graph_ref.name, "my-graph");
        assert_eq!(graph_ref.variant, "current");
        assert_eq!(coord, Some(SchemaCoordinate::Type("Post".into())));
    }

    #[test]
    fn parse_graph_ref_no_variant_with_field() {
        let (graph_ref, coord) = parse_graph_ref_and_coordinate("my-graph:User.posts").unwrap();
        assert_eq!(graph_ref.name, "my-graph");
        assert_eq!(graph_ref.variant, "current");
        assert_eq!(
            coord,
            Some(SchemaCoordinate::Field {
                type_name: "User".into(),
                field_name: "posts".into()
            })
        );
    }
}
