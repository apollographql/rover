//! FR30: `supergraph.yaml` outranks the manifest for the `supergraph` plugin,
//! and says so when the two disagree.

use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

use rover_print::print::{Print, PrintExt};

use super::{PluginRequest, RequestSource};
use crate::plugin::{layering::LayeredDeclarations, manifest::MANIFEST_FILE, version::PluginName};

/// Whether this run has already said that `supergraph.yaml` overrode the
/// manifest.
///
/// A long-running session re-reads `supergraph.yaml` every time it changes,
/// and repeating the same advice on each reload trains people to ignore it.
#[derive(Debug, Default)]
pub struct ManifestOverridden {
    reported: AtomicBool,
}

impl ManifestOverridden {
    /// The flag for this invocation. Production call sites share it so that
    /// "once per invocation" means what it says; tests construct their own.
    pub fn process() -> &'static Self {
        static PROCESS: OnceLock<ManifestOverridden> = OnceLock::new();
        PROCESS.get_or_init(Self::default)
    }

    /// Warn, unless this run already has, when `request` was taken from
    /// `supergraph.yaml` and the manifest declares a different `supergraph`
    /// request. A flag or environment variable that outranks both overrides
    /// `supergraph.yaml` too, and is no disagreement to report.
    pub fn warn_once<P: Print + ?Sized>(
        &self,
        printer: &P,
        request: &PluginRequest,
        declarations: &LayeredDeclarations,
    ) {
        let from_config = request.plugin == PluginName::Supergraph
            && matches!(request.source, RequestSource::SupergraphConfig(_));
        let disagrees = declarations
            .get(PluginName::Supergraph)
            .is_some_and(|layered| layered.declaration.request != request.request);

        if from_config && disagrees && !self.reported.swap(true, Ordering::Relaxed) {
            printer.warnln(format!(
                "`supergraph.yaml` sets `federation_version: {}`, overriding the `supergraph` \
                 version declared in `{MANIFEST_FILE}`.",
                request.request
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use rover_print::print::testing::TerminalCapture;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::{
        error::RequestOrigin, layering::DeclarationLevel, manifest::RoverManifest,
    };

    const WARNING: &str = "warning: `supergraph.yaml` sets `federation_version: =2.9.3`, \
                           overriding the `supergraph` version declared in `rover.yaml`.";

    fn declaring(yaml: &str) -> LayeredDeclarations {
        let manifest: RoverManifest = serde_yaml::from_str(yaml).unwrap();
        LayeredDeclarations::new(None, Some(&manifest))
    }

    fn supergraph(request: &str, source: RequestSource) -> PluginRequest {
        PluginRequest {
            plugin: PluginName::Supergraph,
            request: request.parse().unwrap(),
            source,
        }
    }

    fn warned(request: &PluginRequest, declarations: &LayeredDeclarations) -> Vec<String> {
        let printer = TerminalCapture::new(false);
        ManifestOverridden::default().warn_once(&printer, request, declarations);
        printer.lines()
    }

    #[rstest]
    #[case::exactly("plugins:\n  supergraph: \"=2.8.0\"\n")]
    #[case::floating("plugins:\n  supergraph: \"2\"\n")]
    fn a_disagreeing_manifest_is_named(#[case] manifest: &str) {
        let request = supergraph("=2.9.3", RequestSource::SupergraphConfig(None));

        assert_that!(warned(&request, &declaring(manifest))).is_equal_to(vec![WARNING.to_string()]);
    }

    #[rstest]
    #[case::an_agreeing_manifest(
        supergraph("=2.9.3", RequestSource::SupergraphConfig(None)),
        "plugins:\n  supergraph: \"=2.9.3\"\n"
    )]
    #[case::the_same_request_in_a_legacy_spelling(
        supergraph("2", RequestSource::SupergraphConfig(None)),
        "plugins:\n  supergraph: latest-2\n"
    )]
    #[case::a_manifest_without_supergraph(
        supergraph("=2.9.3", RequestSource::SupergraphConfig(None)),
        "plugins:\n  router: \"=2.1.0\"\n"
    )]
    #[case::a_flag_outranking_both(
        supergraph(
            "=2.7.0",
            RequestSource::Argument(RequestOrigin::Flag("--federation-version"))
        ),
        "plugins:\n  supergraph: \"=2.8.0\"\n"
    )]
    #[case::the_manifest_itself(
        supergraph("=2.8.0", RequestSource::Manifest(DeclarationLevel::Project)),
        "plugins:\n  supergraph: \"=2.8.0\"\n"
    )]
    fn nothing_overridden_says_nothing(#[case] request: PluginRequest, #[case] manifest: &str) {
        assert_that!(warned(&request, &declaring(manifest))).is_equal_to(Vec::<String>::new());
    }

    #[rstest]
    fn the_warning_is_given_once_however_often_supergraph_yaml_is_read() {
        let printer = TerminalCapture::new(false);
        let overridden = ManifestOverridden::default();
        let declarations = declaring("plugins:\n  supergraph: \"=2.8.0\"\n");

        for request in ["=2.9.3", "=2.7.0", "=2.9.3"] {
            let request = supergraph(request, RequestSource::SupergraphConfig(None));
            overridden.warn_once(&printer, &request, &declarations);
        }

        assert_that!(printer.lines()).is_equal_to(vec![WARNING.to_string()]);
    }
}
