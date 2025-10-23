#[cfg(target_os = "macos")]
mod macos;

use std::path::Path;

use anyhow::{bail, ensure, Context, Result};
use camino::Utf8PathBuf;
use clap::Parser;

use crate::{
    target::Target,
    utils::{PKG_PROJECT_NAME, PKG_PROJECT_ROOT, PKG_VERSION, TARGET_DIR},
};

const INCLUDE: &[&str] = &["README.md", "LICENSE"];

#[derive(Debug, Parser)]
pub struct Package {
    /// The target to build Rover for
    #[arg(long = "target", env = "XTASK_TARGET", default_value_t)]
    target: Target,

    /// Output tarball.
    #[arg(long, default_value = "artifacts")]
    output: Utf8PathBuf,

    #[arg(long)]
    rebuild: bool,

    #[arg(long)]
    copy_schema: bool,

    #[cfg(target_os = "macos")]
    #[clap(flatten)]
    macos: macos::PackageMacos,
}

impl Package {
    pub fn run(&self) -> Result<()> {
        if self.rebuild {
            crate::commands::Dist {
                target: self.target.clone(),
                version: None,
            }
            .run()?;
        }

        self.create_tarball("rover")?;

        Ok(())
    }

    fn create_tarball(&self, bin_name: &str) -> Result<()> {
        let bin_name_with_suffix = format!("{}{}", bin_name, std::env::consts::EXE_SUFFIX);
        let release_path = TARGET_DIR
            .join(self.target.to_string())
            .join("release")
            .join(&bin_name_with_suffix);

        ensure!(
            release_path.exists(),
            "Could not find binary at: {}, try running this command with the `--rebuild` flag.",
            release_path
        );

        #[cfg(target_os = "macos")]
        self.macos.run(&release_path, bin_name)?;

        if !self.output.exists() {
            std::fs::create_dir_all(&self.output).context("Couldn't create output directory")?;
        }

        let output_path = if self.output.is_dir() {
            self.output.join(format!(
                "{}-v{}-{}.tar.gz",
                &bin_name, *PKG_VERSION, self.target
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
        ar.sparse(false); // Disable sparse file detection which breaks npm tar extraction (the npm "tar" package silently ignores/skips SparseFile)
        crate::info!("Adding {} to tarball", release_path);
        ar.append_file(
            Path::new("dist").join(bin_name_with_suffix),
            &mut std::fs::File::open(release_path).context("could not open binary")?,
        )
        .context("could not add binary to TGZ archive")?;

        for filename in INCLUDE {
            let resolved_path = if bin_name == PKG_PROJECT_NAME {
                PKG_PROJECT_ROOT.join(filename)
            } else {
                PKG_PROJECT_ROOT
                    .join("plugins")
                    .join(bin_name)
                    .join(filename)
            };
            crate::info!("Adding {} to tarball", &resolved_path);
            ar.append_file(
                Path::new("dist").join(filename),
                &mut std::fs::File::open(resolved_path).context("could not open file")?,
            )
            .context("could not add file to TGZ archive")?;
        }

        ar.finish().context("could not finish TGZ archive")?;

        if self.copy_schema && bin_name == PKG_PROJECT_NAME {
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
