use clap::Parser;

use crate::{
    RoverResult,
    utils::{effect::read_stdin::ReadStdin, parsers::FileDescriptorType},
};

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
        read_stdin_impl: &mut impl ReadStdin,
    ) -> RoverResult<String> {
        self.file
            .read_file_descriptor(file_description, read_stdin_impl)
    }
}
