mod ast_ext;
mod error;
mod operation;
mod printer;

use camino::Utf8PathBuf;
use operation::{ParsedInputs, PersistedQueryOperation};
use serde::Serialize;

use crate::RoverResult;

const MANIFEST_FORMAT: &str = "apollo-persisted-query-manifest";
const MANIFEST_VERSION: u8 = 1;

#[derive(Debug, Serialize)]
pub(super) struct PersistedQueryManifest {
    format: &'static str,
    version: u8,
    operations: Vec<PersistedQueryOperation>,
}

impl PersistedQueryManifest {
    pub(super) fn from_files(files: Vec<Utf8PathBuf>) -> RoverResult<Self> {
        let parsed_inputs = ParsedInputs::from_files(files)?;
        let operations = parsed_inputs.generate_operations()?;
        Ok(Self {
            format: MANIFEST_FORMAT,
            version: MANIFEST_VERSION,
            operations,
        })
    }

    pub(super) const fn operation_count(&self) -> usize {
        self.operations.len()
    }
}
