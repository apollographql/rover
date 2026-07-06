mod manifest;
mod output;

use camino::Utf8PathBuf;
use clap::Parser;
use manifest::PersistedQueryManifest;
use output::GenerateOutput;
use rover_print::print::PrintExt;
use rover_std::Fs;
use serde::Serialize;

use crate::{RoverOutput, RoverResult, options::FileDiscoveryOpt};

#[derive(Debug, Serialize, Parser)]
pub struct Generate {
    #[clap(flatten)]
    #[serde(flatten)]
    file_discovery: FileDiscoveryOpt,

    /// Path to write the generated manifest to. If omitted, the manifest is printed to stdout.
    #[arg(long = "manifest-path", short = 'm', value_name = "FILE")]
    manifest_path: Option<Utf8PathBuf>,
}

impl Generate {
    pub async fn run<P: rover_print::print::Print>(&self, stderr: &P) -> RoverResult<RoverOutput> {
        let files = self.file_discovery.find(&["graphql"])?;
        let manifest = PersistedQueryManifest::from_files(files)?;
        let operation_count = manifest.operation_count();

        if operation_count == 0 {
            stderr.warnln("no operations found during manifest generation. You may need to adjust the glob pattern used to search files in this project.")?;
        }

        let manifest_value = serde_json::to_value(&manifest)?;

        let output = match &self.manifest_path {
            Some(path) => {
                let manifest_json = format!("{}\n", serde_json::to_string_pretty(&manifest_value)?);
                Fs::write_file(path, manifest_json)?;
                GenerateOutput::File {
                    path: path.clone(),
                    operation_count,
                }
            }
            None => GenerateOutput::Stdout {
                manifest: manifest_value,
            },
        };

        Ok(RoverOutput::CliOutput(Box::new(output)))
    }
}
