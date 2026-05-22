mod output;

use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Parser;
use rover_schema::ParsedSchema;
use rover_std::Fs;
use serde::Serialize;

use self::output::SearchOutput;
use crate::{RoverOutput, RoverResult};

#[derive(Debug, Serialize, Parser)]
/// Search a GraphQL schema for types and fields by keyword
///
/// Queries are comma-separated clauses; a result matches if any clause matches.
/// Within a clause, all terms must match (space-separated). Names are split on
/// camelCase / snake_case; matching is case-insensitive. Results are ranked by
/// match tier (strongest first), then alphabetically by coordinate:
///
///   Exact        substring of the name or a name token
///   Stem         shares an English stem with a name token
///   Fuzzy        within one edit of a name token (terms ≥ 4 chars;
///                shorter terms must match a token exactly)
///   Description  appears in the SDL description (with no name match)
///
/// Pass `-` as FILE to read an SDL as a file from stdin.
#[command(after_help = "EXAMPLES:\n    \
    rover schema search schema.graphql email\n    \
    rover schema search schema.graphql \"create post\"\n    \
    rover schema search schema.graphql \"email, displayName\"\n    \
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
        let (sdl, label) = self.read_sdl()?;
        let query = self.terms.join(" ");
        let schema = ParsedSchema::parse(&sdl, &label);
        let results = schema.search(&query, self.limit, self.include_deprecated);
        Ok(RoverOutput::CliOutput(Box::new(SearchOutput {
            query,
            results,
        })))
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
