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
    discovery::ManifestDirs,
    error::{PluginFailure, RequestOrigin},
    layering::{DeclarationLevel, LayeredDeclarations},
    lockfile::{LOCKFILE, PluginLockfile},
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
    /// A floating declaration in the manifest at this level, pinned to the
    /// exact release that level's lockfile records for it.
    Lockfile(DeclarationLevel),
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

impl PluginRequest {
    /// This request, pinned to the release the lockfile at the level that
    /// supplied it records, when it is floating and that lockfile has an
    /// entry for the plugin. A pinned request needs nothing from the
    /// registry. A request from anywhere but a manifest is never pinned,
    /// since no lockfile records it.
    pub fn locked(self, dirs: &ManifestDirs) -> Result<Self, Box<PluginFailure>> {
        let RequestSource::Manifest(level) = self.source else {
            return Ok(self);
        };
        let dir = match level {
            DeclarationLevel::Project => dirs.project.as_deref(),
            DeclarationLevel::Global => dirs.global.as_deref(),
        };
        let Some(dir) = dir.filter(|_| self.request.is_floating()) else {
            return Ok(self);
        };
        let lockfile = PluginLockfile::load(&dir.join(LOCKFILE))?;

        Ok(
            match lockfile.as_ref().and_then(|lock| lock.get(self.plugin)) {
                Some(entry) => Self {
                    request: VersionRequest::Exact(entry.resolved.clone()),
                    source: RequestSource::Lockfile(level),
                    ..self
                },
                None => self,
            },
        )
    }

    /// Where this request came from, as an error about it names that, or
    /// `None` for the built-in default, which nobody asked for.
    pub fn origin(&self) -> Option<RequestOrigin> {
        match &self.source {
            RequestSource::Argument(origin) => Some(origin.clone()),
            RequestSource::EnvVar(var) => Some(RequestOrigin::EnvVar(var)),
            RequestSource::SupergraphConfig(path) => {
                Some(RequestOrigin::SupergraphConfig(path.clone()))
            }
            RequestSource::Manifest(_) => Some(RequestOrigin::Manifest),
            RequestSource::Lockfile(_) => Some(RequestOrigin::Lockfile),
            RequestSource::Default => None,
        }
    }
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

    mod locked {
        use std::fs;

        use assert_fs::TempDir;

        use super::*;

        const LOCK: &str = "version = 1\n\n[[plugins]]\nname = \"supergraph\"\nrequested = \"2\"\nresolved = \"2.9.3\"\n";

        /// A project and a global level, with `lockfile` written at
        /// `locked_at` only.
        fn levels(locked_at: DeclarationLevel, lockfile: &str) -> (TempDir, ManifestDirs) {
            let temp = TempDir::new().unwrap();
            let root = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
            let dirs = ManifestDirs {
                global: Some(root.join("global")),
                project: Some(root.join("project")),
            };
            let dir = match locked_at {
                DeclarationLevel::Project => dirs.project.clone(),
                DeclarationLevel::Global => dirs.global.clone(),
            }
            .unwrap();
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join(LOCKFILE), lockfile).unwrap();
            (temp, dirs)
        }

        fn request(request: VersionRequest, source: RequestSource) -> PluginRequest {
            PluginRequest {
                plugin: PluginName::Supergraph,
                request,
                source,
            }
        }

        #[rstest]
        #[case::project(DeclarationLevel::Project)]
        #[case::global(DeclarationLevel::Global)]
        fn a_floating_declaration_takes_its_own_levels_locked_release(
            #[case] level: DeclarationLevel,
        ) {
            let (_temp, dirs) = levels(level, LOCK);
            let declared = request(VersionRequest::Major(2), RequestSource::Manifest(level));

            assert_that!(declared.locked(&dirs).unwrap()).is_equal_to(request(
                VersionRequest::Exact(Version::new(2, 9, 3)),
                RequestSource::Lockfile(level),
            ));
        }

        #[rstest]
        #[case::declared_at_the_other_level(
            DeclarationLevel::Global,
            request(
                VersionRequest::Major(2),
                RequestSource::Manifest(DeclarationLevel::Project)
            )
        )]
        #[case::declared_exactly(
            DeclarationLevel::Project,
            request(exact(8), RequestSource::Manifest(DeclarationLevel::Project))
        )]
        #[case::from_a_flag(
            DeclarationLevel::Project,
            request(VersionRequest::Major(2), RequestSource::Argument(FLAG))
        )]
        #[case::from_the_environment(
            DeclarationLevel::Project,
            request(VersionRequest::Major(2), RequestSource::EnvVar(ENV))
        )]
        #[case::from_supergraph_yaml(
            DeclarationLevel::Project,
            request(VersionRequest::Major(2), RequestSource::SupergraphConfig(None))
        )]
        #[case::the_default(
            DeclarationLevel::Project,
            request(VersionRequest::Major(2), RequestSource::Default)
        )]
        fn any_other_request_resolves_as_asked(
            #[case] locked_at: DeclarationLevel,
            #[case] asked: PluginRequest,
        ) {
            let (_temp, dirs) = levels(locked_at, LOCK);

            assert_that!(asked.clone().locked(&dirs).unwrap()).is_equal_to(asked);
        }

        #[rstest]
        fn a_lockfile_without_the_plugin_leaves_the_request_floating() {
            let lock = LOCK.replace("supergraph", "router");
            let (_temp, dirs) = levels(DeclarationLevel::Project, &lock);
            let declared = request(
                VersionRequest::Latest,
                RequestSource::Manifest(DeclarationLevel::Project),
            );

            assert_that!(declared.clone().locked(&dirs).unwrap()).is_equal_to(declared);
        }

        #[rstest]
        fn an_unusable_lockfile_is_reported_rather_than_skipped() {
            let (temp, dirs) = levels(DeclarationLevel::Project, "version = 9\n");
            let declared = request(
                VersionRequest::Major(2),
                RequestSource::Manifest(DeclarationLevel::Project),
            );

            let failure = declared.locked(&dirs).expect_err("should not pin");

            assert_that!(failure.to_string()).is_equal_to(format!(
                "`{}` was written by a newer version of Rover, in lockfile format version 9.",
                Utf8PathBuf::try_from(temp.path().join("project").join(LOCKFILE)).unwrap()
            ));
        }
    }
}
