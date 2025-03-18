use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::env;
use std::io::{Cursor, Read};
use tar::Archive;
use crate::RoverResult;
use crate::utils::client::StudioClientConfig;
use flate2::read::GzDecoder;
use tokio_util::bytes::Bytes;
use tower::{Service, ServiceBuilder, ServiceExt};
use rover_http::extend_headers::ExtendHeadersLayer;
use rover_http::ReqwestService;
use crate::command::init::EditorFamily;

pub const CREATE_PROMPT: &str = "=> You’re about to create a local directory with the following files:";

struct ProjectTemplate {
    template_contents: Option<Vec<(String, Vec<u8>)>>,
    top_level_paths: Vec<String>,
}

pub async fn fetch_repo(client_config: StudioClientConfig, editor: EditorFamily) -> RoverResult<ProjectTemplate> {
    let uri = env::var("CONNECTORS_TEMPLATE_URL").unwrap_or_else(
        |_| "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz".to_string(),
    );

    let request = ReqwestService::builder()
        .build()?;

    let mut http_service = ServiceBuilder::new()
        .service(request);

    let req = http::Request::builder()
        .uri(uri)
        .method(http::Method::GET)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .body()?;

    let service = http_service.ready().await?;
    let resp = service.call(req).await?;

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

    let tarball_cursor = Cursor::new(response_bytes);
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