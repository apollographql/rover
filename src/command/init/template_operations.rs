use camino::Utf8PathBuf;
use itertools::Itertools;
use rover_std::infoln;
use rover_std::prompt::prompt_confirm_default_yes;
use std::io;

pub struct TemplateOperations;

impl TemplateOperations {
    pub fn prompt_creation(artifacts: Vec<Utf8PathBuf>) -> io::Result<bool> {
        println!("The following files will be created:");
        let mut artifacts_sorted = artifacts;
        artifacts_sorted.sort();

        Self::print_grouped_files(artifacts_sorted);

        println!();
        prompt_confirm_default_yes("Proceed with creation?")
    }

    pub fn print_grouped_files(artifacts: Vec<Utf8PathBuf>) {
        for (_, files) in &artifacts
            .into_iter()
            .chunk_by(|artifact| artifact.parent().map(|p| p.to_owned()))
        {
            for file in files {
                if file.file_name().is_some() {
                    infoln!("{}", file);
                }
            }
        }
    }
}