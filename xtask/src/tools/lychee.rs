use crate::utils::PKG_PROJECT_ROOT;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use lychee_lib::{
    Client, ClientBuilder, Collector, FileType, Input, InputSource, Request,
    Result as LycheeResult, Uri,
};
use reqwest::StatusCode;
use std::{collections::HashSet, fs, path::PathBuf, time::Duration};
use tokio::runtime::Runtime;
use tokio_stream::StreamExt;

pub(crate) struct LycheeRunner {
    client: Client,
}

impl LycheeRunner {
    pub(crate) fn new() -> Result<Self> {
        let accepted = Some(HashSet::from_iter(vec![
            StatusCode::OK,
            StatusCode::TOO_MANY_REQUESTS,
        ]));

        let client = ClientBuilder::builder()
            .exclude_all_private(true)
            .exclude_mail(true)
            .retry_wait_time(Duration::from_secs(30))
            .max_retries(5u8)
            .accepted(accepted)
            .build()
            .client()?;

        Ok(Self { client })
    }

    pub(crate) fn lint(&self) -> Result<()> {
        crate::info!("Checking HTTP links in repository");

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
            let links: Vec<Request> = Collector::new(None)
                .collect_links(inputs)
                .await
                .collect::<LycheeResult<Vec<_>>>()
                .await?;

            let failed_link_futures: Vec<_> = links
                .into_iter()
                .map(|link| tokio::spawn(get_failed_request(lychee_client.clone(), link)))
                .collect();

            let links_size = failed_link_futures.len();

            let mut failed_checks = Vec::with_capacity(links_size);
            for f in failed_link_futures.into_iter() {
                if let Some(failure) = f.await.expect("unexpected error while processing links") {
                    failed_checks.push(failure);
                }
            }

            crate::info!("{} links checked.", links_size);

            if !failed_checks.is_empty() {
                for failed_check in failed_checks {
                    crate::info!("❌ {}", failed_check.as_str());
                }

                Err(anyhow!("Some links in markdown documentation are down."))
            } else {
                Ok(())
            }
        })?;

        Ok(())
    }
}

async fn get_failed_request(lychee_client: Client, link: Request) -> Option<Uri> {
    let response = lychee_client
        .check(link)
        .await
        .expect("could not execute lychee request");
    if response.status().is_failure() {
        Some(response.1.uri)
    } else {
        crate::info!("✅ {}", response.1.uri.as_str());
        None
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
                        // check every file except for the changelog (there are too many links)
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
