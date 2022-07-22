use saucer::{clap, Parser};

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

#[derive(Debug, Clone, Parser)]
pub struct OptionalSchemaOpt {
    /// The schema file to check. You can pass `-` to use stdin instead of a file.
    #[clap(long, short = 's')]
    schema: Option<FileDescriptorType>,
}

impl OptionalSchemaOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> Result<Option<String>> {
        if let Some(schema) = &self.schema {
            Ok(Some(schema.read_file_descriptor(file_description, stdin)?))
        } else {
            Ok(None)
        }
    }
}
