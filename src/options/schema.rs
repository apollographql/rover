use saucer::{clap, Parser};
use serde::Serialize;

use crate::{utils::parsers::FileDescriptorType, Result};

use std::io::Read;

#[derive(Debug, Parser)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[clap(long, short = 's')]
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
