use anyhow::{bail, ensure, Context, Result};
use base64::Engine;
use clap::Parser;
use serde_json_traversal::serde_json_traversal;
use std::io::Write as _;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::utils::{PKG_PROJECT_ROOT, PKG_VERSION};

const ENTITLEMENTS: &str = "macos-entitlements.plist";

#[derive(Debug, Parser)]
pub struct PackageMacos {
    /// Keychain keychain_password.
    #[arg(long, env = "MACOS_KEYCHAIN_PASSWORD", hide_env_values = true)]
    keychain_password: String,

    /// Certificate bundle in base64.
    #[arg(long, env = "MACOS_CERT_BUNDLE_BASE64", hide_env_values = true)]
    cert_bundle_base64: String,

    /// Certificate bundle keychain_password.
    #[arg(long, env = "MACOS_CERT_BUNDLE_PASSWORD", hide_env_values = true)]
    cert_bundle_password: String,

    /// Primary bundle ID.
    #[arg(long, env = "MACOS_PRIMARY_BUNDLE_ID")]
    primary_bundle_id: String,

    /// Apple team ID.
    #[arg(long, env = "APPLE_TEAM_ID")]
    apple_team_id: String,

    /// Apple username.
    #[arg(long, env = "APPLE_USERNAME")]
    apple_username: String,

    /// Notarization password.
    #[arg(long, env = "APPLE_NOTARIZATION_PASSWORD", hide_env_values = true)]
    notarization_password: String,
}

impl PackageMacos {
    pub fn run(&self, release_path: impl AsRef<Path>, bin_name: &str) -> Result<()> {
        let release_path = release_path.as_ref();
        let temp = tempfile::tempdir().context("could not create temporary directory")?;

        crate::info!("Temporary directory created at: {}", temp.path().display());

        let keychain_name = temp.path().file_name().unwrap().to_str().unwrap();

        let entitlements = PKG_PROJECT_ROOT.join(ENTITLEMENTS);
        ensure!(entitlements.exists(), "could not find entitlements file");

        crate::info!("Creating keychain...");
        ensure!(
            Command::new("security")
                .args(["create-keychain", "-p"])
                .arg(&self.keychain_password)
                .arg(keychain_name)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Removing relock timeout on keychain...");
        ensure!(
            Command::new("security")
                .arg("set-keychain-settings")
                .arg(keychain_name)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Decoding certificate bundle...");
        let certificate_path = temp.path().join("certificate.p12");
        std::fs::write(
            &certificate_path,
            base64::prelude::BASE64_STANDARD
                .decode(&self.cert_bundle_base64)
                .context("could not decode base64 encoded certificate bundle")?,
        )
        .context("could not write decoded certificate to file")?;

        crate::info!("Importing codesigning certificate to build keychain...");
        ensure!(
            Command::new("security")
                .arg("import")
                .arg(&certificate_path)
                .arg("-k")
                .arg(keychain_name)
                .arg("-P")
                .arg(&self.cert_bundle_password)
                .arg("-T")
                .arg(which::which("codesign").context("could not find codesign")?)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Adding the codesign tool to the security partition-list...");
        ensure!(
            Command::new("security")
                .args([
                    "set-key-partition-list",
                    "-S",
                    "apple-tool:,apple:,codesign:",
                    "-s",
                    "-k"
                ])
                .arg(&self.keychain_password)
                .arg(keychain_name)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Setting default keychain...");
        ensure!(
            Command::new("security")
                .args(["default-keychain", "-d", "user", "-s"])
                .arg(keychain_name)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Unlocking keychain...");
        ensure!(
            Command::new("security")
                .args(["unlock-keychain", "-p"])
                .arg(&self.keychain_password)
                .arg(keychain_name)
                .status()
                .context("could not start command security")?
                .success(),
            "command exited with error",
        );

        crate::info!("Verifying keychain is set up correctly...");
        let output = Command::new("security")
            .args(["find-identity", "-v", "-p", "codesigning"])
            .stderr(Stdio::inherit())
            .output()
            .context("could not start command security")?;
        let _ = std::io::stdout().write(&output.stdout);
        ensure!(output.status.success(), "command exited with error",);
        ensure!(
            !String::from_utf8_lossy(&output.stdout).contains("0 valid identities found"),
            "no valid identities found",
        );

        crate::info!("Signing code (step 1)...");
        ensure!(
            Command::new("codesign")
                .arg("--sign")
                .arg(&self.apple_team_id)
                .args(["--options", "runtime", "--entitlements"])
                .arg(&entitlements)
                .args(["--force", "--timestamp"])
                .arg(release_path)
                .arg("-v")
                .status()
                .context("could not start command codesign")?
                .success(),
            "command exited with error",
        );

        crate::info!("Signing code (step 2)...");
        ensure!(
            Command::new("codesign")
                .args(["-vvv", "--deep", "--strict"])
                .arg(release_path)
                .status()
                .context("could not start command codesign")?
                .success(),
            "command exited with error",
        );

        crate::info!("Zipping dist...");
        let dist_zip = temp
            .path()
            .join(format!("{}-{}.zip", bin_name, *PKG_VERSION));
        let mut zip = zip::ZipWriter::new(std::io::BufWriter::new(
            std::fs::File::create(&dist_zip).context("could not create file")?,
        ));
        let options = zip::write::FileOptions::default()
            .compression_method(zip::CompressionMethod::Stored)
            .unix_permissions(0o755);
        let path = Path::new("dist").join(bin_name);
        crate::info!("Adding {} as {}...", release_path.display(), path.display());
        zip.start_file(path.to_str().unwrap(), options)?;
        std::io::copy(
            &mut std::io::BufReader::new(
                std::fs::File::open(release_path).context("could not open file")?,
            ),
            &mut zip,
        )?;
        zip.finish()?;

        crate::info!("Beginning notarization process...");
        let output = Command::new("xcrun")
            .args(["altool", "--notarize-app", "--primary-bundle-id"])
            .arg(&self.primary_bundle_id)
            .arg("--username")
            .arg(&self.apple_username)
            .arg("--password")
            .arg(&self.notarization_password)
            .arg("--asc-provider")
            .arg(&self.apple_team_id)
            .arg("--file")
            .arg(&dist_zip)
            .args(["--output-format", "json"])
            .stderr(Stdio::inherit())
            .output()
            .context("could not start command xcrun")?;
        let _ = std::io::stdout().write(&output.stdout);
        ensure!(output.status.success(), "command exited with error",);
        let json: serde_json::Value =
            serde_json::from_slice(&output.stdout).context("could not parse json output")?;
        let success_message = serde_json_traversal!(json => success-message)
            .unwrap()
            .as_str()
            .unwrap();
        let request_uuid = serde_json_traversal!(json => notarization-upload => RequestUUID)
            .unwrap()
            .as_str()
            .unwrap();
        crate::info!("Success message: {}", success_message);
        crate::info!("Request UUID: {}", request_uuid);

        let start_time = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(60 * 10);
        let result = loop {
            crate::info!("Checking notarization status...");
            let output = Command::new("xcrun")
                .args(["altool", "--notarization-info"])
                .arg(request_uuid)
                .arg("--username")
                .arg(&self.apple_username)
                .arg("--password")
                .arg(&self.notarization_password)
                .args(["--output-format", "json"])
                .stderr(Stdio::inherit())
                .output()
                .context("could not start command xcrun")?;

            let status = if !output.status.success() {
                // NOTE: if the exit status is failure we need to keep trying otherwise the
                //       process becomes a bit flaky
                crate::info!("command exited with error");
                None
            } else {
                let json: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .context("could not parse json output")?;
                serde_json_traversal!(json => notarization-info => Status)
                    .ok()
                    .and_then(|x| x.as_str())
                    .map(|x| x.to_string())
            };

            if !matches!(
                status.as_deref(),
                Some("in progress") | None if start_time.elapsed() < duration
            ) {
                break status;
            }

            std::thread::sleep(std::time::Duration::from_secs(5));
        };
        match result.as_deref() {
            Some("success") => crate::info!("Notarization successful"),
            Some("in progress") => bail!("Notarization timeout"),
            Some(other) => bail!("Notarization failed: {}", other),
            None => bail!("Notarization failed without status message"),
        }

        Ok(())
    }
}
