use crate::command::init::{EditorFamily, InitProjectActions};
use crate::RoverResult;
use flate2::read::GzDecoder;
use rover_http::{Full, ReqwestService};
use rover_std::{infoln, Style};
use std::env;
use std::fs;
use std::io::{Cursor, Read};
use std::path::Path;
use tar::Archive;
use tower::Service;

pub const CREATE_PROMPT: &str =
    "=> You’re about to create a local directory with the following files:";

pub struct ConnectorProject {
    template_files: Vec<ArchiveEntry>,
    pub editor_family: EditorFamily,
}

struct ArchiveEntry {
    path: String,
    is_top_level: bool,
    contents: Vec<u8>,
}

impl ConnectorProject {
    pub fn new(editor_family: EditorFamily) -> Self {
        Self {
            template_files: Vec::new(),
            editor_family,
        }
    }
}

impl InitProjectActions for ConnectorProject {
    async fn fetch_repo(&mut self, http_service: &mut ReqwestService) -> RoverResult<()> {
        let uri = env::var("CONNECTORS_TEMPLATE_URL").unwrap_or_else(
            |_| "https://github.com/apollographql/rover-connectors-starter/archive/refs/heads/main.tar.gz".to_string(),
        );

        let req = http::Request::builder()
            .method(http::Method::GET)
            .header(reqwest::header::ACCEPT, "application/octet-stream")
            .uri(uri)
            .body(Full::default())?;

        let res = http_service.call(req).await?;
        let res = res.body().to_vec();
        let template_files = extract_tarball(res, self.editor_family)?;
        self.template_files = template_files;

        Ok(())
    }

    fn display_files(&self) -> RoverResult<()> {
        let template_contents = &self.template_files;
        println!("\n{}\n", Style::Heading.paint(CREATE_PROMPT));
        if !template_contents.is_empty() {
            for entry in template_contents.iter().filter(|e| e.is_top_level) {
                infoln!("{}", entry.path);
            }
        } else {
            eprintln!("No template files are currently available to display.");
        }

        Ok(())
    }

    fn write_template(&self, target_path: &str) -> RoverResult<()> {
        let template_contents = &self.template_files;
        let target_path = target_path;
        if !template_contents.is_empty() {
            for ArchiveEntry {
                path,
                contents,
                is_top_level: _is_top_level,
            } in template_contents
            {
                let full_path = Path::new(target_path).join(path);

                if let Some(parent) = full_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                
                fs::write(full_path, contents)?;
            }
        }

        Ok(())
    }
}

fn extract_tarball(data: Vec<u8>, editor: EditorFamily) -> RoverResult<Vec<ArchiveEntry>> {
    let tarball_cursor = Cursor::new(data);
    let decompressor = GzDecoder::new(tarball_cursor);
    let mut archive = Archive::new(decompressor);

    let mut extracted_files = Vec::new();

    for entry in archive.entries()? {
        let mut file = entry?;

        let file_path = strip_base_path(file.path().unwrap().to_string_lossy().to_string());

        if skip_file(&file_path, editor) {
            continue;
        }

        let mut file_contents = Vec::new();
        file.read_to_end(&mut file_contents)?;

        let is_top_level = is_top_level_path(&file_path);

        let archive_entry = ArchiveEntry {
            path: file_path,
            is_top_level,
            contents: file_contents,
        };

        extracted_files.push(archive_entry);
    }

    Ok(extracted_files)
}

// in the archive, the base path is always the name of the tarball itself.
// we don't want that so we strip it here
fn strip_base_path(original: String) -> String {
    original
        .splitn(2, '/')
        .nth(1)
        .unwrap_or(&original)
        .to_string()
}

// just to check if this is a top level path. we only display those to reduce the num
// of paths
fn is_top_level_path(path: &str) -> bool {
    !path.trim_end_matches('/').contains('/')
}

fn skip_file(file_path: &str, editor: EditorFamily) -> bool {
    file_path.is_empty()
        || file_path.starts_with("pax_global_header")
        || (editor == EditorFamily::VSCode && file_path.starts_with(".idea"))
        || (editor == EditorFamily::Jetbrains && file_path.starts_with(".vscode"))
}
