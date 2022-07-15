use clap::Parser;

use crate::{
    utils::parsers::{parse_file_descriptor, FileDescriptorType},
    Result,
};

use std::{fmt::Display, io::Read};

#[derive(Debug, Parser)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[clap(long, short = 's', parse(try_from_str = parse_file_descriptor))]
    schema: FileDescriptorType,
}

impl SchemaOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> Result<String> {
        self.schema.read_file_descriptor(file_description, stdin)
    }
}

impl Display for SchemaOpt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "--schema {}", self.schema)
    }
}
