use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use camino::Utf8PathBuf;
use http::{HeaderMap, StatusCode};
use lychee_lib::{
    Client, ClientBuilder, Collector, FileType, Input, InputSource, Request,
    Result as LycheeResult, Uri,
};
use tokio_stream::StreamExt;

use crate::utils::PKG_PROJECT_ROOT;

pub(crate) struct LycheeRunner {
    client: Client,
}

impl LycheeRunner {
    pub(crate) fn new(
        retry_wait_time: Duration,
        max_retries: u8,
        exclude_all_private: bool,
    ) -> Result<Self> {
        let client = ClientBuilder::builder()
            .exclude_all_private(exclude_all_private)
            .retry_wait_time(retry_wait_time)
            .max_retries(max_retries)
            .build()
            .client()?;

        Ok(Self { client })
    }

    pub(crate) async fn lint(&self) -> Result<()> {
        crate::info!("Checking HTTP links in repository");

        let inputs: Vec<Input> = get_md_files()
            .iter()
            // Skip the changelog to preserve history, but also to avoid checking hundreds of
            // PR links and similar that don't need validation
            .filter(|file| !file.to_string().contains("CHANGELOG"))
            .map(|file| Input {
                source: InputSource::FsPath(PathBuf::from(file)),
                file_type_hint: Some(FileType::Markdown),
                excluded_paths: None,
                headers: HeaderMap::new(),
            })
            .collect();

        let lychee_client = self.client.clone();

        let links: Vec<Request> = Collector::new(None, None)?
            .collect_links(inputs)
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
            for (uri, status_code) in failed_checks {
                crate::info!(
                    "❌ [Status Code: {}]: {}",
                    status_code
                        .map(|status_code| status_code.to_string())
                        .unwrap_or("unknown".to_string()),
                    uri
                );
            }
            Err(anyhow!("Some links in markdown documentation are down."))
        } else {
            Ok(())
        }
    }
}

async fn get_failed_request(
    lychee_client: Client,
    link: Request,
) -> Option<(Uri, Option<StatusCode>)> {
    let response = lychee_client
        .check(link)
        .await
        .expect("could not execute lychee request");
    if response.status().is_error() {
        let status_code = response.status().code();
        Some((response.1.uri, status_code))
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use http::StatusCode;
    use lychee_lib::{Client, InputSource, Request, Uri};
    use rstest::{fixture, rstest};
    use speculoos::prelude::*;
    use tokio::runtime::Runtime;

    use super::get_failed_request;
    use crate::tools::LycheeRunner;

    #[fixture]
    fn lychee_client() -> Result<Client> {
        LycheeRunner::new(Duration::from_secs(1), 1, false).map(|r| r.client)
    }

    #[rstest]
    #[case::success(200)]
    fn test_get_failed_request_success(
        lychee_client: Result<Client>,
        #[case] response_status_int: usize,
    ) -> Result<()> {
        let lychee_client = lychee_client?;
        let mut server = mockito::Server::new();
        let url = server.url();
        let _mock = server
            .mock("GET", "/success")
            .with_status(response_status_int)
            .create();
        let request = Request::new(
            Uri::try_from(&format!("{}/{}", url, "success") as &str)?,
            InputSource::String("test".to_string()),
            None,
            None,
            None,
        );
        let rt = Runtime::new()?;
        let result = rt.block_on(get_failed_request(lychee_client, request));
        assert_that!(result).is_none();

        Ok(())
    }

    #[rstest]
    #[case::internal_server_error(400, StatusCode::BAD_REQUEST)]
    #[case::internal_server_error(401, StatusCode::UNAUTHORIZED)]
    #[case::internal_server_error(403, StatusCode::FORBIDDEN)]
    #[case::internal_server_error(404, StatusCode::NOT_FOUND)]
    #[case::internal_server_error(500, StatusCode::INTERNAL_SERVER_ERROR)]
    fn test_get_failed_request_failure(
        lychee_client: Result<Client>,
        #[case] response_status_int: usize,
        #[case] response_status_code: StatusCode,
    ) -> Result<()> {
        let lychee_client = lychee_client?;
        let mut server = mockito::Server::new();
        let url = server.url();
        let _mock = server
            .mock("GET", "/success")
            .with_status(response_status_int)
            .create();
        let uri = Uri::try_from(&format!("{}/{}", url, "success") as &str)?;
        let request = Request::new(
            uri.clone(),
            InputSource::String("test".to_string()),
            None,
            None,
            None,
        );
        let rt = Runtime::new()?;
        let result = rt.block_on(get_failed_request(lychee_client, request));
        assert_that!(result)
            .is_some()
            .is_equal_to((uri, Some(response_status_code)));

        Ok(())
    }
}
