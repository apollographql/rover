#[cfg(target_os = "macos")]
mod macos;

use anyhow::{bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use std::path::Path;
use structopt::StructOpt;

use crate::target::{Target, POSSIBLE_TARGETS};
use crate::utils::{PKG_PROJECT_NAME, PKG_PROJECT_ROOT, PKG_VERSION, RELEASE_BIN, TARGET_DIR};

const INCLUDE: &[&str] = &["README.md", "LICENSE"];

#[derive(Debug, StructOpt)]
pub struct Package {
    /// The target to build Rover for
    #[structopt(long = "target", env = "XTASK_TARGET", default_value, possible_values = &POSSIBLE_TARGETS)]
    target: Target,

    /// Output tarball.
    #[structopt(long, default_value = "artifacts")]
    output: Utf8PathBuf,

    #[structopt(long)]
    rebuild: bool,

    #[structopt(long)]
    copy_schema: bool,

    #[cfg(target_os = "macos")]
    #[structopt(flatten)]
    macos: macos::PackageMacos,
}

impl Package {
    pub fn run(&self) -> Result<()> {
        if self.rebuild {
            crate::commands::Dist {
                target: self.target.clone(),
                version: None,
            }
            .run(true)?;
        }

        for bin in &["rover", "rover-fed"] {
            self.create_tarball(bin)?;
        }

        Ok(())
    }

    fn create_tarball(&self, bin_name: &str) -> Result<()> {
        let mut release_path = TARGET_DIR.join(self.target.to_string()).join("release");
        if cfg!(windows) {
            release_path.push(format!("{}.exe", bin_name));
        } else {
            release_path.push(bin_name)
        }
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
                "{}-v{}-{}.tar.gz",
                bin_name, *PKG_VERSION, self.target
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

        if self.copy_schema {
            std::fs::copy(
                PKG_PROJECT_ROOT
                    .join("crates")
                    .join("rover-client")
                    .join(".schema")
                    .join("schema.graphql"),
                self.output.join(format!(
                    "{}-v{}-schema.graphql",
                    PKG_PROJECT_NAME, *PKG_VERSION
                )),
            )
            .context("could not include schema in artifacts")?;
        }

        Ok(())
    }
}
