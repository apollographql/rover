use std::fs;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use regex::Regex;

use crate::utils::{PKG_PROJECT_ROOT, PKG_VERSION};

/// Subdirectory of the project root that holds the container action
/// definitions. Each subdirectory's `action.yml` gets shipped to the matching
/// `apollographql-gh-actions/<repo>` at release time.
const CONTAINER_ACTIONS_DIR: &str = "actions";

/// Rewrites the `image:` line in each container `action.yml` so the published
/// release tag pins the action to the matching Rover Docker image. Walks
/// `actions/*/action.yml` so adding a new action is purely a matter of
/// dropping its directory in. Non-container action.yml files (composite,
/// JavaScript) are silently skipped.
pub(crate) fn update_versions() -> Result<()> {
    crate::info!("updating container action image tags.");
    let actions_root = PKG_PROJECT_ROOT.join(CONTAINER_ACTIONS_DIR);
    let entries = fs::read_dir(actions_root.as_path())
        .with_context(|| format!("Could not read {}", actions_root))?
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
///
/// The double-quote delimiters and `image:` prefix are part of the match so
/// the rewrite only touches the actual image-pin line, even if other text in
/// the file happens to contain the same version string.
fn update_action_image(action_file: &Utf8Path) -> Result<()> {
    let old_contents = fs::read_to_string(action_file)
        .with_context(|| format!("Could not read contents of {} to a String", action_file))?;

    let version_regex =
        Regex::new(r#"(image:\s*"docker://ghcr\.io/apollographql/rover:)([^"]+)(")"#)
            .context("Could not create regex for container action image tag")?;
    let Some(captures) = version_regex.captures(&old_contents) else {
        // Composite / JavaScript action — nothing to pin.
        return Ok(());
    };
    // Group 2 is guaranteed non-empty by the regex (uses `+`), so the
    // capture's presence implies a present version.
    let old_version = &captures[2];

    if old_version == PKG_VERSION.as_str() {
        return Ok(());
    }

    crate::info!("updating image tag in `{}`.", action_file);
    let replacement = format!("${{1}}{}${{3}}", *PKG_VERSION);
    let new_contents = version_regex.replace_all(&old_contents, replacement.as_str());
    fs::write(action_file, new_contents.as_ref())
        .with_context(|| format!("Could not write updated image tag to {}", action_file))?;
    Ok(())
}
