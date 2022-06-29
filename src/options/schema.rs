use structopt::StructOpt;

use crate::{
    utils::parsers::{parse_file_descriptor, FileDescriptorType},
    Result,
};

use std::io::Read;

#[derive(Debug, StructOpt)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[structopt(long, short = "s", parse(try_from_str = parse_file_descriptor))]
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
