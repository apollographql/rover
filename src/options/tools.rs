use clap::Parser;

// use crate::{utils::parsers::FileDescriptorType, RoverResult};

use std::{io::Read, any::Any};

use crate::RoverResult;

#[derive(Debug, Parser)]
pub struct ToolsMergeOpt {
    /// The path to schema files to merge.
    #[arg(long, short = 's')]
    schemas: String,
}

impl ToolsMergeOpt {
    pub(crate) fn read_file_descriptor(
        &self,
        file_description: &str,
        stdin: &mut impl Read,
    ) -> RoverResult<String> {
        // not implemented
        Ok("".to_string())
    }
}
