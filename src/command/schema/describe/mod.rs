use std::{
    io::{self, Read},
    path::{Path, PathBuf},
};

use clap::Parser;
use rover_schema::{ParsedSchema, SchemaCoordinate};
use rover_std::Fs;
use serde::Serialize;

use crate::{RoverOutput, RoverResult};

mod output;
pub use output::DescribeOutput;
use output::filtered_sdl;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputFormat {
    Description,
    Sdl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    /// Human-readable description (default for TTY)
    Description,
    /// Raw SDL
    Sdl,
}

#[derive(Debug, Serialize, Parser)]
/// Describe a graph's schema by type or field
///
/// Displays a structured description of your graph's schema. Start with an
/// overview, then zoom into individual types and fields.
///
/// Reads from a file or from stdin when no file is given (or when - is passed).
#[command(after_help = "EXAMPLES:\n    \
    rover schema describe schema.graphql\n    \
    rover schema describe schema.graphql --coord Post\n    \
    rover schema describe schema.graphql --coord User.posts\n    \
    rover schema describe schema.graphql --coord Post --view sdl\n    \
    cat schema.graphql | rover schema describe\n    \
    rover schema describe -")]
pub struct Describe {
    /// SDL file to read. Pass - or omit to read from stdin.
    #[arg(value_name = "FILE")]
    #[serde(skip_serializing)]
    file: Option<PathBuf>,

    /// Schema coordinate to inspect (e.g. Post or User.posts)
    #[arg(short = 'c', long = "coord", value_name = "SCHEMA_COORDINATE", value_parser = clap::value_parser!(SchemaCoordinate))]
    #[serde(skip)]
    schema_coordinate: Option<SchemaCoordinate>,

    /// Expand referenced types N levels deep
    #[arg(long = "depth", short = 'd', default_value_t = 0)]
    depth: usize,

    /// Show deprecated fields and types
    #[arg(long = "include-deprecated")]
    include_deprecated: bool,

    /// Select output view: description (default TTY) or sdl
    #[arg(long = "view", short = 'v', value_name = "VIEW")]
    view: Option<ViewMode>,
}

impl Describe {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        let (sdl_string, source_label) = self.read_sdl()?;
        let output_format = self.output_format();
        let schema = ParsedSchema::parse(&sdl_string);

        if matches!(output_format, OutputFormat::Sdl) {
            let sdl = filtered_sdl(self.schema_coordinate.as_ref(), schema.inner());
            return Ok(RoverOutput::CliOutput(Box::new(DescribeOutput::Sdl(sdl))));
        }

        let output = match &self.schema_coordinate {
            None => DescribeOutput::Overview(schema.overview(source_label)),
            Some(SchemaCoordinate::Type(tc)) => {
                let detail = schema
                    .type_detail(&tc.ty, self.include_deprecated, self.depth)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                DescribeOutput::Type(detail)
            }
            Some(coord @ SchemaCoordinate::TypeAttribute(_)) => {
                let detail = schema
                    .field_detail(coord)
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                DescribeOutput::Field(detail)
            }
            Some(other) => {
                return Err(
                    anyhow::anyhow!("unsupported coordinate for describe: '{other}'").into(),
                );
            }
        };

        Ok(RoverOutput::CliOutput(Box::new(output)))
    }

    fn output_format(&self) -> OutputFormat {
        match self.view {
            Some(ViewMode::Sdl) => OutputFormat::Sdl,
            Some(ViewMode::Description) | None => OutputFormat::Description,
        }
    }

    /// Returns `(sdl_contents, display_label)`.
    fn read_sdl(&self) -> RoverResult<(String, String)> {
        let use_stdin = match &self.file {
            None => true,
            Some(p) => p == Path::new("-"),
        };

        if use_stdin {
            let mut sdl = String::new();
            io::stdin()
                .read_to_string(&mut sdl)
                .map_err(|e| anyhow::anyhow!("failed to read from stdin: {}", e))?;
            return Ok((sdl, "<stdin>".to_string()));
        }

        let path = self.file.as_ref().unwrap();
        let utf8_path = camino::Utf8PathBuf::try_from(path.clone()).map_err(|p| {
            anyhow::anyhow!("path '{}' contains invalid UTF-8", p.as_path().display())
        })?;
        let label = utf8_path.to_string();
        Ok((Fs::read_file(utf8_path)?, label))
    }
}
