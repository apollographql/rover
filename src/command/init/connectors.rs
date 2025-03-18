use crate::command::init::EditorFamily;
use crate::utils::client::StudioClientConfig;
use crate::RoverResult;
use flate2::read::GzDecoder;
use http::Error;
use itertools::Itertools;
use rover_http::{Full, ReqwestService};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;
use tower::{Service, ServiceBuilder, ServiceExt};

pub const CREATE_PROMPT: &str =
    "=> You’re about to create a local directory with the following files:";

struct ProjectTemplate {
    template_contents: Option<Vec<(String, Vec<u8>)>>,
    top_level_paths: Vec<String>,
}

pub async fn fetch_repo(
    client_config: StudioClientConfig,
    editor: EditorFamily,
) -> RoverResult<ProjectTemplate> {
    let uri = env::var("CONNECTORS_TEMPLATE_URL").unwrap_or_else(
        |_| "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz".to_string(),
    );

    let request = ReqwestService::builder().build()?;

    let mut http_service = ServiceBuilder::new().service(request);

    let req = http::Request::builder()
        .method(http::Method::GET)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .uri(uri)
        .body(Full::default())?;

    let service = http_service.ready().await?;
    let res = service.call(req).await?;
    let res = res.body().bytes();

    // let response_bytes = client_config
    //     .get_reqwest_client()
    //     .unwrap()
    //     .get(uri)
    //     .header(reqwest::header::USER_AGENT, "rover-client")
    //     .header(reqwest::header::ACCEPT, "application/octet-stream")
    //     .send()
    //     .await?
    //     .error_for_status()?
    //     .bytes()
    //     .await?;
    //

    let res = res.collect_vec();
    let blah: Result<Vec<u8>, std::io::Error> = res.into_iter().collect();
    let blah = blah.unwrap();

    //let tarball_cursor = Cursor::new(res);
    let tarball_cursor = Cursor::new(blah);
    let decompressor = GzDecoder::new(tarball_cursor);
    let mut archive = Archive::new(decompressor);

    let mut extracted_files = Vec::new();
    let mut top_level_set = HashSet::new();

    for entry in archive.entries()? {
        let mut file = entry?;
        let file_path = strip_base_path(file.path()?.to_string_lossy().to_string());

        if let Some(top_level_path) = file_path.split('/').next() {
            top_level_set.insert(top_level_path.to_string());
        }

        let mut file_contents = Vec::new();
        file.read_to_end(&mut file_contents)?;
        extracted_files.push((file_path, file_contents));
    }

    let project = ProjectTemplate {
        template_contents: Some(extracted_files),
        top_level_paths: top_level_set.into_iter().collect(),
    };

    Ok(project)
}

impl ProjectTemplate {
    pub fn write_template(&self, target_path: &str) -> RoverResult<()> {
        if let Some(contents) = &self.template_contents {
            for (relative_path, file_bytes) in contents {
                let full_path = Path::new(target_path).join(relative_path);

                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }

                fs::write(full_path, file_bytes)?;
            }
        }
        Ok(())
    }
}

fn strip_base_path(original: String) -> String {
    let parts: Vec<&str> = original.splitn(2, '/').collect();
    if parts.len() > 1 {
        parts[1].to_string()
    } else {
        original.to_string()
    }
}

