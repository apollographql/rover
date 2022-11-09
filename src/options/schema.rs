use clap::Parser;

use crate::{utils::parsers::FileDescriptorType, RoverResult};

use std::io::Read;

#[derive(Debug, Parser)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[arg(long, short = 's')]
    schema: FileDescriptorType,
}

impl SchemaOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> RoverResult<String> {
        self.schema.read_file_descriptor(file_description, stdin)
    }
}
