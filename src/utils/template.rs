use std::io::Cursor;

use anyhow::{Result, anyhow};
use camino::Utf8Path;
use rover_std::Fs;
use url::Url;

pub async fn download_template(url: Url, to_path: &Utf8Path) -> Result<()> {
    tracing::debug!("Downloading from {}", &url);
    let res = reqwest::Client::new()
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream")
        .header(reqwest::header::USER_AGENT, "rover-client")
        .send()
        .await?;
    let res = res.bytes().await?;

    if res.is_empty() {
        return Err(anyhow!("No template found"));
    }

    let cursor = Cursor::new(&res);
    let tar = flate2::read::GzDecoder::new(cursor);
    let mut archive = tar::Archive::new(tar);

    archive.unpack(to_path)?;

    let extra_dir_name = Fs::get_dir_entries(to_path)?.find(|_| true);
    if let Some(Ok(extra_dir_name)) = extra_dir_name {
        Fs::copy_dir_all(extra_dir_name.path(), to_path)?;
        Fs::remove_dir_all(extra_dir_name.path())?;
    }

    Ok(())
}
