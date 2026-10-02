//! Which of the places a plugin version can be written decides it.
//!
//! For each plugin a command needs, the request is the first of these that is
//! present, most specific first: a command-line argument, its environment
//! variable, `supergraph.yaml`'s `federation_version` (for `supergraph`
//! only), the project manifest, the global manifest, and finally the built-in
//! floating default. Every plugin-using command takes its request from
//! [`resolve`], so no command can apply an ordering of its own.

use camino::Utf8PathBuf;

use super::{
    error::RequestOrigin,
    layering::{DeclarationLevel, LayeredDeclarations},
    version::{PluginName, VersionRequest},
};

/// The rung of the precedence ladder that supplied a request.
///
/// Later steps need to know it: only a request a manifest supplied can be
/// pinned by that level's lockfile, and an error about a request names where
/// it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestSource {
    /// A version flag, or a `rover plugin install` positional, as `origin`
    /// names it.
    Argument(RequestOrigin),
    /// A version environment variable.
    EnvVar(&'static str),
    /// `federation_version` in the supergraph config, at this path when it
    /// was read from a file.
    SupergraphConfig(Option<Utf8PathBuf>),
    /// A declaration in the manifest at this level.
    Manifest(DeclarationLevel),
    /// Nothing asked for a version, so the plugin's built-in default applies.
    Default,
}

/// Everything a command knows about one plugin's version besides the
/// manifests: what it was given on the command line and in the environment,
/// what its supergraph config says, and what to use when nothing does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestInputs {
    pub argument: Option<(VersionRequest, RequestOrigin)>,
    pub env: Option<(VersionRequest, &'static str)>,
    /// Consulted for the `supergraph` plugin only.
    pub supergraph_config: Option<(VersionRequest, Option<Utf8PathBuf>)>,
    pub default: VersionRequest,
}

impl RequestInputs {
    /// Inputs with nothing given, falling back to `default`.
    pub const fn new(default: VersionRequest) -> Self {
        Self {
            argument: None,
            env: None,
            supergraph_config: None,
            default,
        }
    }

    /// Place a request that clap took from a flag or its environment
    /// variable on the rung `origin` names. Clap folds the two into one
    /// value, and [`RequestOrigin::flag_or_env`] is what tells them apart.
    pub fn with_override(mut self, given: Option<(VersionRequest, RequestOrigin)>) -> Self {
        match given {
            Some((request, RequestOrigin::EnvVar(var))) => self.env = Some((request, var)),
            Some(argument) => self.argument = Some(argument),
            None => {}
        }
        self
    }

    pub fn with_supergraph_config(
        mut self,
        request: Option<VersionRequest>,
        path: Option<Utf8PathBuf>,
    ) -> Self {
        self.supergraph_config = request.map(|request| (request, path));
        self
    }
}

/// The version request a command will use for one plugin, and the rung that
/// supplied it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRequest {
    pub plugin: PluginName,
    pub request: VersionRequest,
    pub source: RequestSource,
}

/// The request for `plugin`: the first present of `inputs` and
/// `declarations`, in precedence order.
pub fn resolve(
    plugin: PluginName,
    inputs: RequestInputs,
    declarations: &LayeredDeclarations,
) -> PluginRequest {
    let RequestInputs {
        argument,
        env,
        supergraph_config,
        default,
    } = inputs;

    let supergraph_config = supergraph_config.filter(|_| plugin == PluginName::Supergraph);
    let declared = declarations.get(plugin).map(|layered| {
        (
            layered.declaration.request.clone(),
            RequestSource::Manifest(layered.level),
        )
    });

    let (request, source) = argument
        .map(|(request, origin)| (request, RequestSource::Argument(origin)))
        .or_else(|| env.map(|(request, var)| (request, RequestSource::EnvVar(var))))
        .or_else(|| {
            supergraph_config
                .map(|(request, path)| (request, RequestSource::SupergraphConfig(path)))
        })
        .or(declared)
        .unwrap_or((default, RequestSource::Default));

    PluginRequest {
        plugin,
        request,
        source,
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::manifest::RoverManifest;

    const FLAG: RequestOrigin = RequestOrigin::Flag("--federation-version");
    const ENV: &str = "APOLLO_ROVER_DEV_COMPOSITION_VERSION";

    fn exact(minor: u64) -> VersionRequest {
        VersionRequest::Exact(Version::new(2, minor, 0))
    }

    fn manifest(minor: u64) -> RoverManifest {
        serde_yaml::from_str(&format!(
            "plugins:\n  supergraph: \"=2.{minor}.0\"\n  router: \"=2.{minor}.0\"\n"
        ))
        .unwrap()
    }

    /// Each source, present or not, carrying its own version: the argument
    /// asks for 2.1.0, the variable 2.2.0, and so on down to the global
    /// manifest's 2.5.0. The default is the bare major.
    fn requested(plugin: PluginName, present: [bool; 5]) -> (VersionRequest, RequestSource) {
        let [argument, env, config, project, global] = present;
        let inputs = RequestInputs {
            argument: argument.then(|| (exact(1), FLAG)),
            env: env.then(|| (exact(2), ENV)),
            supergraph_config: config.then(|| (exact(3), Some("supergraph.yaml".into()))),
            default: VersionRequest::Major(2),
        };
        let declarations = LayeredDeclarations::new(
            global.then(|| manifest(5)).as_ref(),
            project.then(|| manifest(4)).as_ref(),
        );

        let resolved = resolve(plugin, inputs, &declarations);
        assert_that!(resolved.plugin).is_equal_to(plugin);
        (resolved.request, resolved.source)
    }

    /// Every combination of sources, for both a plugin `supergraph.yaml` can
    /// set and one it can't: the most specific source present wins, and
    /// `supergraph.yaml` is not a source for any plugin but `supergraph`.
    #[rstest]
    fn the_most_specific_present_source_wins(
        #[values(PluginName::Supergraph, PluginName::Router)] plugin: PluginName,
        #[values(false, true)] argument: bool,
        #[values(false, true)] env: bool,
        #[values(false, true)] config: bool,
        #[values(false, true)] project: bool,
        #[values(false, true)] global: bool,
    ) {
        let config_applies = config && plugin == PluginName::Supergraph;
        let expected = [
            (argument, exact(1), RequestSource::Argument(FLAG)),
            (env, exact(2), RequestSource::EnvVar(ENV)),
            (
                config_applies,
                exact(3),
                RequestSource::SupergraphConfig(Some("supergraph.yaml".into())),
            ),
            (
                project,
                exact(4),
                RequestSource::Manifest(DeclarationLevel::Project),
            ),
            (
                global,
                exact(5),
                RequestSource::Manifest(DeclarationLevel::Global),
            ),
        ]
        .into_iter()
        .find_map(|(present, request, source)| present.then_some((request, source)))
        .unwrap_or((VersionRequest::Major(2), RequestSource::Default));

        assert_that!(requested(plugin, [argument, env, config, project, global]))
            .is_equal_to(expected);
    }

    #[rstest]
    #[case::from_the_flag(
        RequestOrigin::Flag("--router-version"),
        RequestSource::Argument(RequestOrigin::Flag("--router-version"))
    )]
    #[case::from_the_variable(
        RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION"),
        RequestSource::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION")
    )]
    #[case::from_the_positional(
        RequestOrigin::PluginArgument,
        RequestSource::Argument(RequestOrigin::PluginArgument)
    )]
    fn an_override_lands_on_the_rung_its_origin_names(
        #[case] origin: RequestOrigin,
        #[case] expected: RequestSource,
    ) {
        let inputs =
            RequestInputs::new(VersionRequest::Latest).with_override(Some((exact(1), origin)));

        let resolved = resolve(PluginName::Router, inputs, &LayeredDeclarations::default());

        assert_that!(resolved).is_equal_to(PluginRequest {
            plugin: PluginName::Router,
            request: exact(1),
            source: expected,
        });
    }
}
