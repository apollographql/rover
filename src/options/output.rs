use std::str::FromStr;

use camino::Utf8PathBuf;
use clap::Parser;

use crate::cli::FormatType;

#[derive(Debug, Parser)]
pub struct Output {
    /// The file path to write the command output to.
    #[clap(long)]
    output: OutputType,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum OutputType {
    LegacyOutputType(FormatType),
    File(Utf8PathBuf),
}

impl FromStr for OutputType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(format_type) = FormatType::from_str(s) {
            Ok(Self::LegacyOutputType(format_type))
        } else {
            Ok(Self::File(Utf8PathBuf::from(s)))
        }
    }
}
