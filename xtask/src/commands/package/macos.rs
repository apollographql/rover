use std::{
    io::Write as _,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::{ensure, Context, Result};
use base64::Engine;
use clap::Parser;

use crate::{
    tools::XcrunRunner,
    utils::{PKG_PROJECT_ROOT, PKG_VERSION},
};

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
        let options = zip::write::SimpleFileOptions::default()
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

        let dist_zip = dist_zip.to_str().unwrap_or_else(|| {
            panic!(
                "path to zipped directory '{}' is not valid utf-8",
                dist_zip.display()
            )
        });

        XcrunRunner::new().notarize(
            dist_zip,
            &self.apple_username,
            &self.apple_team_id,
            &self.notarization_password,
        )?;

        Ok(())
    }
}
