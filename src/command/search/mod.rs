use clap::Parser;
use rover_client::shared::GraphRef;
use rover_schema::{
    ParsedSchema,
    format::{self, OutputFormat, compact, description},
    search,
};
use serde::Serialize;

use crate::{
    RoverOutput, RoverResult, command::schema_cache, options::ProfileOpt,
    utils::client::StudioClientConfig,
};

#[derive(Debug, Serialize, Parser)]
/// Search a graph's schema by keyword
///
/// Find types, fields, and operations by name or description. Results include
/// complete paths from Query/Mutation roots with all intermediate types.
///
/// Piped output defaults to --compact.
#[command(after_help = "EXAMPLES:\n    \
        rover search my-graph@my-variant \"search terms\"\n    \
        rover search my-graph@my-variant \"create post\"")]
pub struct Search {
    /// <NAME>@<VARIANT> of graph in Apollo Studio.
    /// @<VARIANT> may be left off, defaulting to @current
    #[arg(value_name = "GRAPH_REF")]
    #[serde(skip_serializing)]
    graph_ref: GraphRef,

    /// Search terms
    #[arg(value_name = "TERMS")]
    #[serde(skip_serializing)]
    terms: String,

    /// Max result paths
    #[arg(long = "limit", default_value_t = 5)]
    limit: usize,

    /// Show deprecated fields in results
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

impl Search {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        // Fetch SDL (with caching)
        let sdl_string = schema_cache::fetch_sdl_cached(
            &self.graph_ref,
            &self.profile,
            &client_config,
            self.no_cache,
        )
        .await?;

        // Parse
        let parsed = ParsedSchema::parse(&sdl_string).map_err(|e| anyhow::anyhow!("{}", e))?;
        let schema = parsed.inner();

        // Search
        let results = search::search(schema, &self.terms, self.limit, self.include_deprecated)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let json_data = serde_json::to_value(&results).unwrap_or(serde_json::Value::Null);

        // Format
        let output_format = format::select_format(self.sdl, self.compact, false);
        let content = match output_format {
            OutputFormat::Description => description::format_search(&results, &self.terms),
            OutputFormat::Compact => compact::format_search_compact(&results),
            OutputFormat::Sdl => {
                // For search with --sdl, we'd need to extract SDL for all matched types
                // For now, fall back to description format
                description::format_search(&results, &self.terms)
            }
        };

        Ok(RoverOutput::SearchResponse { content, json_data })
    }
}
