#[cfg(target_os = "macos")]
mod macos;

use anyhow::{bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use std::path::Path;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::utils::{PKG_PROJECT_ROOT, PKG_VERSION, RELEASE_BIN, TARGET_DIR};

const INCLUDE: &[&str] = &["README.md", "LICENSE"];

#[derive(Debug, StructOpt)]
pub struct Package {
    /// The target to build Rover for
    #[structopt(long = "target", default_value, possible_values = &POSSIBLE_TARGETS)]
    target: Target,

    /// Output tarball.
    #[structopt(long, default_value = "artifacts")]
    output: Utf8PathBuf,

    #[cfg(target_os = "macos")]
    #[structopt(flatten)]
    macos: macos::PackageMacos,
}

impl Package {
    pub fn run(&self) -> Result<()> {
        let release_path = TARGET_DIR
            .join(self.target.to_string())
            .join("release")
            .join(RELEASE_BIN);

        ensure!(
            release_path.exists(),
            "Could not find binary at: {}",
            release_path
        );

        #[cfg(target_os = "macos")]
        self.macos.run(&release_path)?;

        if !self.output.exists() {
            std::fs::create_dir_all(&self.output).context("Couldn't create output directory")?;
        }

        let output_path = if self.output.is_dir() {
            self.output.join(format!(
                "{}-{}-{}.tar.gz",
                RELEASE_BIN, *PKG_VERSION, self.target
            ))
        } else {
            bail!("--output must be a path to a directory, not a file.");
        };

        crate::info!("Creating tarball: {}", output_path);
        let mut file = flate2::write::GzEncoder::new(
            std::io::BufWriter::new(
                std::fs::File::create(&output_path).context("could not create TGZ file")?,
            ),
            flate2::Compression::default(),
        );
        let mut ar = tar::Builder::new(&mut file);
        crate::info!("Adding {} to tarball", release_path);
        ar.append_file(
            Path::new("dist").join(RELEASE_BIN),
            &mut std::fs::File::open(release_path).context("could not open binary")?,
        )
        .context("could not add file to TGZ archive")?;

        for path in INCLUDE {
            crate::info!("Adding {}...", path);
            ar.append_file(
                Path::new("dist").join(path),
                &mut std::fs::File::open(PKG_PROJECT_ROOT.join(path))
                    .context("could not open binary")?,
            )
            .context("could not add file to TGZ archive")?;
        }

        ar.finish().context("could not finish TGZ archive")?;

        Ok(())
    }
}
