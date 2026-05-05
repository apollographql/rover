use std::{fs, str};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use regex::bytes::Regex;

use crate::utils::{PKG_PROJECT_ROOT, PKG_VERSION};

const CONTAINER_ACTIONS_DIR: &str = "actions";

/// Rewrites the `image:` line in each container-based `action.yml` so the published
/// release tag pins the action to the matching Rover Docker image.
/// Walks `[CONTAINER_ACTIONS_DIR]/*/action.yml`.
pub(crate) fn update_versions() -> Result<()> {
    crate::info!("updating container action image tags.");
    let actions_root = PKG_PROJECT_ROOT.join(CONTAINER_ACTIONS_DIR);
    let entries = fs::read_dir(actions_root.as_path())
        .with_context(|| format!("Could not read {}", &actions_root))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| Utf8PathBuf::from_path_buf(entry.path()).ok())
        .filter(|path| path.is_dir())
        .map(|dir| dir.join("action.yml"))
        .filter(|action_file| action_file.is_file());

    for action_file in entries {
        update_action_image(&action_file)?;
    }
    Ok(())
}

/// Captures and updates the version segment of a line shaped like
///
/// ```yaml
///   image: "docker://ghcr.io/apollographql/rover:<version>"
/// ```
fn update_action_image(action_file: &Utf8Path) -> Result<()> {
    let old_contents = fs::read_to_string(action_file)
        .with_context(|| format!("Could not read contents of {} to a String", action_file))?;

    let version_regex = Regex::new(r#"image:\s*"docker://ghcr\.io/apollographql/rover:([^"]+)""#)
        .context("Could not create regex for container action image tag")?;
    let Some(captures) = version_regex.captures(old_contents.as_bytes()) else {
        return Ok(());
    };
    let old_version = str::from_utf8(
        captures
            .get(1)
            .ok_or_else(|| anyhow!("Could not find version capture group in {}", action_file))?
            .as_bytes(),
    )
    .context("Capture group is not valid UTF-8")?;

    if old_version == PKG_VERSION.as_str() {
        return Ok(());
    }

    crate::info!("updating image tag in `{}`.", action_file);
    let old_pin = format!("rover:{old_version}");
    let new_pin = format!("rover:{}", *PKG_VERSION);
    let new_contents = old_contents.replace(&old_pin, &new_pin);
    fs::write(action_file, new_contents)
        .with_context(|| format!("Could not write updated image tag to {}", action_file))?;
    Ok(())
}
