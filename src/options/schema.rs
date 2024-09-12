use clap::Parser;

use crate::{utils::parsers::FileDescriptorType, RoverResult};

use std::io::Read;

#[derive(Debug, Parser)]
pub struct SchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[arg(long, short = 's')]
    schema: FileDescriptorType,
}

pub struct FileWithMetadata {
    pub schema: String,
    pub file_path: String,
}

impl SchemaOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> RoverResult<String> {
        self.schema.read_file_descriptor(file_description, stdin)
    }

    pub(crate) fn read_file_descriptor_with_metadata(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> RoverResult<FileWithMetadata> {
        match self.schema.read_file_descriptor(file_description, stdin) {
            Ok(proposed_schema) => Ok(FileWithMetadata {
                schema: proposed_schema,
                file_path: match &self.schema {
                    FileDescriptorType::Stdin => "stdin".to_owned(),
                    FileDescriptorType::File(file_path) => file_path.to_string(),
                },
            }),
            Err(e) => Err(e),
        }
    }
}
