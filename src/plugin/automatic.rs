//! Whether a plugin-using command may download a plugin it needs on its own.
//!
//! From Rover 1.0 it may not unless something opts in: `allow_automatic_download`
//! in either level's manifest, or `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD`,
//! which outranks both. "Automatic" is the load-bearing word: an explicit
//! `rover plugin install` is never automatic, so nothing here governs it.

use super::{error::DownloadControl, layering::LayeredDeclarations};

/// Whether a command may download a plugin installed at neither level.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum AutomaticDownloads {
    /// Nothing opted in.
    #[default]
    NotAllowed,
    /// The environment variable or a manifest opted in.
    Allowed,
}

impl AutomaticDownloads {
    /// What this process allows, given the manifests' layered `declarations`:
    /// allowed when `APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD` is `1` or `true`,
    /// whatever either manifest says, and otherwise as the manifests decide.
    ///
    /// Like every other boolean Rover variable, any other value is the same
    /// as leaving it unset, so it can only opt in, never out.
    pub fn in_scope(declarations: &LayeredDeclarations) -> Self {
        if crate::utils::allow_automatic_download() || declarations.allow_automatic_download() {
            Self::Allowed
        } else {
            Self::NotAllowed
        }
    }

    /// The control this amounts to for a command that installs on the fly,
    /// if it forbids downloading: [`DownloadControl::NotOptedIn`] unless
    /// something opted in.
    pub const fn control(self) -> Option<DownloadControl> {
        match self {
            Self::NotAllowed => Some(DownloadControl::NotOptedIn),
            Self::Allowed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::{plugin::manifest::RoverManifest, utils::ALLOW_AUTOMATIC_DOWNLOAD_ENV};

    /// The declarations of a project manifest that sets
    /// `allow_automatic_download` to `value`, or leaves it out for `None`.
    fn project(value: Option<bool>) -> LayeredDeclarations {
        let yaml = value.map_or_else(String::new, |value| {
            format!("allow_automatic_download: {value}\n")
        });
        let manifest: RoverManifest = serde_yaml::from_str(&yaml).unwrap();
        LayeredDeclarations::new(None, Some(&manifest))
    }

    #[rstest]
    #[case::nothing_opts_in(None, None, AutomaticDownloads::NotAllowed)]
    #[case::the_manifest_opts_in(None, Some(true), AutomaticDownloads::Allowed)]
    #[case::the_manifest_opts_out(None, Some(false), AutomaticDownloads::NotAllowed)]
    #[case::the_variable_opts_in(Some("true"), None, AutomaticDownloads::Allowed)]
    #[case::the_variable_as_one(Some("1"), None, AutomaticDownloads::Allowed)]
    #[case::the_variable_outranks_a_manifest_that_opts_out(
        Some("TRUE"),
        Some(false),
        AutomaticDownloads::Allowed
    )]
    #[case::a_variable_that_is_off_leaves_it_to_the_manifest(
        Some("false"),
        Some(true),
        AutomaticDownloads::Allowed
    )]
    #[case::a_variable_that_is_off_opts_nothing_in(Some("0"), None, AutomaticDownloads::NotAllowed)]
    fn the_variable_outranks_the_manifests(
        #[case] variable: Option<&str>,
        #[case] manifest: Option<bool>,
        #[case] expected: AutomaticDownloads,
    ) {
        temp_env::with_var(ALLOW_AUTOMATIC_DOWNLOAD_ENV, variable, || {
            assert_that!(AutomaticDownloads::in_scope(&project(manifest))).is_equal_to(expected);
        });
    }
}
