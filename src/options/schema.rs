use clap::Parser;

use crate::{
    RoverResult,
    utils::{effect::read_stdin::ReadStdin, parsers::FileDescriptorType},
};

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
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<String> {
        self.schema
            .read_file_descriptor(file_description, read_stdin_impl)
    }

    pub(crate) fn read_file_descriptor_with_metadata(
        &self,
        file_description: &str,
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<FileWithMetadata> {
        match self
            .schema
            .read_file_descriptor(file_description, read_stdin_impl)
        {
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
