use std::time::SystemTime;

use anyhow::Result;
use billboard::{Alignment, Billboard};
use camino::Utf8PathBuf;
use reqwest::Client;
use rover_std::{Fs, Style};

use crate::PKG_VERSION;
use houston as config;
use rover_client::releases::{Version, get_latest_release};

const ONE_HOUR: u64 = 60 * 60;
const ONE_DAY: u64 = ONE_HOUR * 24;

/// check for newer versions of rover.
///
/// If this fn is run explicitly from a user-facing command, we pass `force` to
/// check for newer versions, even if we recently checked for updates.
///
/// If `force` is not passed, we check for updates every day at most
pub async fn check_for_update(config: config::Config, force: bool, client: Client) -> Result<()> {
    let version_file = config.home.join("version.toml");
    let current_time = SystemTime::now();
    // if we don't end up checking, we don't want to overwrite the last checked time
    let mut checked = false;

    // check fs for last check time
    let last_checked_time = get_last_checked_time_from_disk(&version_file);

    if force || last_checked_time.is_none() {
        do_update_check(&mut checked, force, client).await?;
    } else if let Some(last_checked_time) = last_checked_time {
        let time_since_check = current_time.duration_since(last_checked_time)?.as_secs();
        tracing::trace!(
            "Time since last update check: {:?}h",
            time_since_check / ONE_HOUR
        );

        if time_since_check > ONE_DAY {
            do_update_check(&mut checked, force, client).await?;
        }
    }

    if checked {
        tracing::trace!("Checked for available updates. Writing current time to disk");
        Fs::write_file(&version_file, toml::to_string(&current_time)?)?;
    }

    Ok(())
}

async fn do_update_check(
    checked: &mut bool,
    should_output_if_updated: bool,
    client: Client,
) -> Result<()> {
    let latest_version = get_latest_release(client).await?;
    let pretty_latest = Style::Version.paint(format!("v{latest_version}"));
    if latest_version > Version::parse(PKG_VERSION)? {
        let message = format!(
            "There is a newer version of Rover available: {} (currently running v{})\n\nFor instructions on how to install, run {}",
            &pretty_latest,
            PKG_VERSION,
            Style::Command.paint("`rover docs open start`")
        );
        Billboard::builder()
            .box_alignment(Alignment::Left)
            .build()
            .eprint(message);
    } else if should_output_if_updated {
        eprintln!(
            "Rover is up to date with the latest release {}.",
            &pretty_latest
        );
    }

    *checked = true;
    Ok(())
}

fn get_last_checked_time_from_disk(version_file: &Utf8PathBuf) -> Option<SystemTime> {
    match Fs::read_file(version_file) {
        Ok(contents) => toml::from_str(&contents)
            .inspect_err(|_err| {
                tracing::debug!("Failed to parse last update check time from version file")
            })
            .ok(),
        Err(e) => {
            tracing::debug!(
                "Failed to read version file containing last update check time: {}",
                e
            );
            None
        }
    }
}
