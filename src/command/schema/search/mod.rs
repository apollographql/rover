use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Parser;
use itertools::Itertools;
use rover_schema::{ParsedSchema, SearchResult, root_paths::RootPath};
use rover_std::Fs;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, command::CliOutput};

#[derive(Debug, Serialize, Parser)]
/// Search a GraphQL schema for types and fields by keyword
///
/// Searches names (with camelCase / snake_case splitting) and descriptions.
/// All terms must match. Results are ranked: name matches above description matches.
///
/// Pass - as FILE to read from stdin.
#[command(after_help = "EXAMPLES:\n    \
    rover schema search schema.graphql email\n    \
    rover schema search schema.graphql \"create post\"\n    \
    cat schema.graphql | rover schema search - user\n    \
    rover schema search schema.graphql author --limit 20\n    \
    rover schema search schema.graphql id --include-deprecated")]
pub struct Search {
    /// SDL file to read. Pass - to read from stdin.
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Search terms. All terms must match (space-separated or quoted).
    #[arg(value_name = "TERMS", required = true)]
    terms: Vec<String>,

    /// Maximum number of results to return.
    #[arg(long, short = 'n', default_value_t = 10)]
    limit: usize,

    /// Include deprecated fields and enum values in results.
    #[arg(long)]
    include_deprecated: bool,
}

impl Search {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let (sdl, _label) = self.read_sdl()?;
        let query = self.terms.join(" ");
        let schema = ParsedSchema::parse(&sdl, "<input>");
        let results = schema.search(&query, self.limit, self.include_deprecated);
        Ok(RoverOutput::CliOutput(Box::new(SearchOutput { query, results })))
    }

    fn read_sdl(&self) -> RoverResult<(String, String)> {
        if self.file == Path::new("-") {
            if io::IsTerminal::is_terminal(&io::stdin()) {
                return Err(anyhow::anyhow!(
                    "stdin is a terminal — pipe a schema file or pass a file path instead of -"
                )
                .into());
            }
            let mut sdl = String::new();
            io::stdin()
                .read_to_string(&mut sdl)
                .map_err(|e| anyhow::anyhow!("failed to read from stdin: {}", e))?;
            return Ok((sdl, "<stdin>".to_string()));
        }

        let utf8_path = camino::Utf8PathBuf::try_from(self.file.clone()).map_err(|p| {
            anyhow::anyhow!("path '{}' contains invalid UTF-8", p.as_path().display())
        })?;
        let label = utf8_path.to_string();
        Ok((Fs::read_file(utf8_path)?, label))
    }
}

#[derive(Debug, Serialize)]
pub struct SearchOutput {
    query: String,
    results: Vec<SearchResult>,
}

impl CliOutput for SearchOutput {
    fn text(&self) -> String {
        if self.results.is_empty() {
            return format!("No results for \"{}\"", self.query);
        }

        let header = format!(
            "{} result{} for \"{}\"",
            self.results.len(),
            if self.results.len() == 1 { "" } else { "s" },
            self.query
        );

        let items = self
            .results
            .iter()
            .map(format_result)
            .join("\n\n");

        format!("{header}\n\n{items}")
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        serde_json::to_value(self)
    }
}

fn format_result(r: &SearchResult) -> String {
    let first_line = match &r.description {
        Some(desc) => format!("{} — {}", r.coordinate, desc),
        None => r.coordinate.to_string(),
    };

    let kind_line = if r.via.is_empty() {
        format!("  {}", r.kind)
    } else {
        let paths = r.via.iter().map(format_root_path).join(", ");
        format!("  {}  ·  via {}", r.kind, paths)
    };

    format!("{first_line}\n{kind_line}")
}

fn format_root_path(p: &RootPath) -> String {
    p.segments
        .iter()
        .map(|s| format!("{}.{}", s.type_name, s.field_name))
        .join(" -> ")
}
