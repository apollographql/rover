use std::io::Read;

use clap::Parser;

use crate::{utils::parsers::FileDescriptorType, RoverResult};

#[derive(Debug, Parser)]
pub struct FileOpt {
    /// Path of local file to read. You can pass `-` to use stdin instead of a file.
    #[arg(long, short = 'f')]
    file: FileDescriptorType,
}

impl FileOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> RoverResult<String> {
        self.file.read_file_descriptor(file_description, stdin)
    }
}
