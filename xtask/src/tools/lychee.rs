use reqwest::StatusCode;
use std::{collections::HashSet, fs, path::PathBuf, time::Duration};
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;

use lychee_lib::{
    Client, ClientBuilder, Collector, FileType, Input, InputSource, Result as LycheeResult,
};
use saucer::{anyhow, Result, Utf8PathBuf};

use crate::utils::PKG_PROJECT_ROOT;

pub(crate) struct LycheeRunner {
    client: Client,
    verbose: bool,
}

impl LycheeRunner {
    pub(crate) fn new(verbose: bool) -> Result<Self> {
        let accepted = Some(HashSet::from_iter(vec![
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS,
        ]));

        let client = ClientBuilder::builder()
            .exclude_all_private(true)
            .retry_wait_time(Duration::from_secs(30))
            .max_retries(5u8)
            .accepted(accepted)
            .build()
            .client()?;

        Ok(Self { client, verbose })
    }

    pub(crate) fn lint(&self) -> Result<()> {
        if self.verbose {
            println!("Checking links in documentation");
        }

        let inputs: Vec<Input> = get_md_files()
            .iter()
            .map(|file| Input {
                source: InputSource::FsPath(PathBuf::from(file)),
                file_type_hint: Some(FileType::Markdown),
                excluded_paths: None,
            })
            .collect();

        let rt = Runtime::new()?;

        let lychee_client = self.client.clone();

        rt.block_on(async move {
            let links = Collector::new(None)
                .collect_links(inputs)
                .await
                .collect::<LycheeResult<Vec<_>>>()
                .await?;

            let mut has_failures = false;
            let links_size = links.len();

            for link in links {
                let response = lychee_client.check(link).await?;
                if response.status().is_failure() {
                    has_failures = true;
                    if self.verbose {
                        println!("[x] {}", response.1.uri.as_str());
                    }
                } else if response.status().is_success() {
                    println!("[✓] {}", response.1.uri.as_str());
                }
            }

            println!("{} links checked.", links_size);

            if has_failures {
                Err(anyhow!("Some links in markdown documentation are down."))
            } else {
                Ok(())
            }
        })?;

        Ok(())
    }
}

fn get_md_files() -> Vec<Utf8PathBuf> {
    let mut md_files = Vec::new();

    walk_dir(PKG_PROJECT_ROOT.as_str(), &mut md_files);

    md_files
}

fn walk_dir(base_dir: &str, md_files: &mut Vec<Utf8PathBuf>) {
    if let Ok(entries) = fs::read_dir(base_dir) {
        for entry in entries.flatten() {
            if let Ok(file_type) = entry.file_type() {
                if file_type.is_file() {
                    if let Ok(file_name) = entry.file_name().into_string() {
                        if file_name.ends_with(".md") {
                            if let Ok(entry_path) = Utf8PathBuf::try_from(entry.path()) {
                                md_files.push(entry_path)
                            }
                        }
                    }
                } else if file_type.is_dir() {
                    if let Ok(dir_name) = entry.file_name().into_string() {
                        // we can't do much if a link is broken in node_modules (and it's big!)
                        if dir_name != "node_modules"
                            // we don't need to check the Rust compiler's output for broken links
                            && dir_name != "target"
                            // the docs have their own link checker, no need to check twice
                            && dir_name != "docs"
                            && dir_name != "dev-docs"
                            // also no need to recurse through hidden directories
                            && !dir_name.starts_with('.')
                        {
                            walk_dir(&dir_name, md_files);
                        }
                    }
                }
            }
        }
    }
}
