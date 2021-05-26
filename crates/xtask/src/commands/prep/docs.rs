use anyhow::{anyhow, Context, Result};
use camino::Utf8PathBuf;

use std::{convert::TryFrom, fs};

use crate::utils;

pub(crate) fn build_error_code_reference() -> Result<()> {
    utils::info("updating error reference material.");
    let project_root = utils::project_root()?;
    let docs_path = project_root.join("docs").join("source").join("errors.md");
    let codes_dir = project_root
        .join("src")
        .join("error")
        .join("metadata")
        .join("codes");

    // sort code files alphabetically
    let raw_code_files = fs::read_dir(&codes_dir)?;

    let mut code_files = Vec::new();
    for raw_code_file in raw_code_files {
        let raw_code_file = raw_code_file?;
        if raw_code_file.file_type()?.is_dir() {
            return Err(anyhow!("Error code directory {} contains a directory {:?}. It must only contain markdown files.", &codes_dir, raw_code_file.file_name()));
        } else {
            code_files.push(raw_code_file);
        }
    }
    code_files.sort_by_key(|f| f.path());

    let mut all_descriptions = String::new();

    // for each code description, get the name of the code from the filename,
    // and add it as a header. Then push the header and description to the
    // all_descriptions string
    for code in code_files {
        let path = Utf8PathBuf::try_from(code.path())?;

        let contents = fs::read_to_string(&path)?;
        let code_name = path
            .file_name()
            .ok_or_else(|| anyhow!("Path {} doesn't have a file name", &path))?
            .replace(".md", "");

        let description = format!("### {}\n\n{}\n\n", code_name, contents);

        all_descriptions.push_str(&description);
    }

    let docs_content = fs::read_to_string(&docs_path)
        .with_context(|| format!("Could not read contents of {} to a String", &docs_path))?;

    // build up a new docs page with existing content line-by-line
    // and then concat the loaded code descriptions after
    let mut new_content = String::new();
    for line in docs_content.lines() {
        new_content.push_str(line);
        new_content.push('\n');
        if line.contains("<!-- BUILD_CODES -->") {
            break;
        }
    }
    new_content.push_str(&all_descriptions);

    fs::write(&docs_path, new_content)?;

    Ok(())
}
