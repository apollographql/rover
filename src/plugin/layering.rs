//! Combining the global and project manifests' plugin declarations.
//!
//! Layering is per plugin, not per file: a project that declares only `router`
//! still gets the global level's `supergraph` declaration.
//!
//! This covers only the two manifest levels of version precedence. Flags,
//! environment variables, and `supergraph.yaml` all outrank both, and the
//! built-in default applies only when neither level declares a plugin;
//! combining those is the resolver's job, not this module's.

use std::collections::BTreeMap;

use camino::Utf8Path;

use super::{
    discovery::ManifestDirs,
    error::PluginFailure,
    manifest::{MANIFEST_FILE, PluginDeclaration, RoverManifest},
    version::PluginName,
};

/// Which level's manifest a declaration came from.
///
/// This is where a version *request* came from, which is independent of which
/// level's install root the plugin is later found in: a version declared
/// globally still installs into the project. It names only the manifest
/// sources of a request; a request from a flag, an environment variable, or
/// `supergraph.yaml` did not come from a manifest at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclarationLevel {
    Project,
    Global,
}

/// A declaration, together with the level that supplied it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayeredDeclaration {
    pub declaration: PluginDeclaration,
    pub level: DeclarationLevel,
}

/// At most one declaration per plugin, drawn from whichever level declares it,
/// preferring the project, and the levels' `allow_automatic_download`,
/// layered the same way.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct LayeredDeclarations {
    plugins: BTreeMap<PluginName, LayeredDeclaration>,
    allow_automatic_download: Option<bool>,
}

impl LayeredDeclarations {
    /// Read the manifest at each level in `dirs` and layer them. A level with
    /// no manifest contributes nothing; a manifest that exists but cannot be
    /// used fails the whole read, at either level, even when the other level
    /// declares every plugin.
    pub fn load(dirs: &ManifestDirs) -> Result<Self, Box<PluginFailure>> {
        let read = |dir: Option<&Utf8Path>| -> Result<Option<RoverManifest>, Box<PluginFailure>> {
            Ok(match dir {
                Some(dir) => RoverManifest::load(&dir.join(MANIFEST_FILE))?,
                None => None,
            })
        };

        // The project first: when both are broken, the one being worked on is
        // the more likely to be the one just edited.
        let project = read(dirs.project.as_deref())?;
        let global = read(dirs.global.as_deref())?;

        Ok(Self::new(global.as_ref(), project.as_ref()))
    }

    /// Layer a project manifest over a global one. Either may be absent: no
    /// project means only the global declarations apply, and no manifest at
    /// either level means nothing is declared.
    ///
    /// Takes the manifests as given; [`Self::load`] is what refuses one this
    /// version of Rover cannot honor.
    pub fn new(global: Option<&RoverManifest>, project: Option<&RoverManifest>) -> Self {
        let declared_at = |manifest: Option<&RoverManifest>, plugin, level| {
            manifest
                .and_then(|manifest| manifest.plugins.get(plugin))
                .map(|declaration| LayeredDeclaration {
                    declaration: declaration.clone(),
                    level,
                })
        };

        let declarations = PluginName::ALL
            .into_iter()
            .filter_map(|plugin| {
                declared_at(project, plugin, DeclarationLevel::Project)
                    .or_else(|| declared_at(global, plugin, DeclarationLevel::Global))
                    .map(|layered| (plugin, layered))
            })
            .collect();

        // Layered like a declaration: a project that sets the key, either
        // way, decides for itself whatever the global level says.
        let allow_automatic_download = [project, global]
            .into_iter()
            .flatten()
            .find_map(|manifest| manifest.allow_automatic_download);

        Self {
            plugins: declarations,
            allow_automatic_download,
        }
    }

    pub fn get(&self, plugin: PluginName) -> Option<&LayeredDeclaration> {
        self.plugins.get(&plugin)
    }

    /// Whether the manifests let a command download a plugin on its own:
    /// the project's `allow_automatic_download` if it sets one, otherwise the
    /// global level's, and no when neither does.
    pub fn allow_automatic_download(&self) -> bool {
        self.allow_automatic_download.unwrap_or(false)
    }

    /// Every declaration, in the order [`PluginName::ALL`] names the plugins.
    pub fn iter(&self) -> impl Iterator<Item = (PluginName, &LayeredDeclaration)> {
        PluginName::ALL
            .into_iter()
            .filter_map(|plugin| self.get(plugin).map(|layered| (plugin, layered)))
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;
    use crate::plugin::version::VersionRequest;

    fn manifest(yaml: &str) -> RoverManifest {
        serde_yaml::from_str(yaml).unwrap()
    }

    /// The layered result as `(plugin, version as written, level)`, which is
    /// the whole of what layering decides.
    fn summary(layered: &LayeredDeclarations) -> Vec<(PluginName, String, DeclarationLevel)> {
        layered
            .iter()
            .map(|(plugin, layered)| (plugin, layered.declaration.written.clone(), layered.level))
            .collect()
    }

    const BROKEN: &str = indoc! {r#"
        plugins:
          - supergraph
    "#};

    const GLOBAL: &str = indoc! {r#"
        plugins:
          supergraph: "2"
          router: latest
    "#};

    #[rstest]
    #[case::the_project_wins_per_plugin(
        Some(GLOBAL),
        Some(indoc! {r#"
            plugins:
              router: "=2.1.0"
        "#}),
        vec![
            (PluginName::Supergraph, "2", DeclarationLevel::Global),
            (PluginName::Router, "=2.1.0", DeclarationLevel::Project),
        ]
    )]
    #[case::each_level_fills_in_what_the_other_leaves_out(
        Some(indoc! {r#"
            plugins:
              supergraph: "2"
        "#}),
        Some(indoc! {r#"
            plugins:
              apollo-mcp-server: latest
        "#}),
        vec![
            (PluginName::Supergraph, "2", DeclarationLevel::Global),
            (PluginName::ApolloMcpServer, "latest", DeclarationLevel::Project),
        ]
    )]
    #[case::the_levels_interleave_in_plugin_order(
        Some(indoc! {r#"
            plugins:
              apollo-mcp-server: latest
              supergraph: "2"
        "#}),
        Some(indoc! {r#"
            plugins:
              router: "=2.1.0"
        "#}),
        vec![
            (PluginName::Supergraph, "2", DeclarationLevel::Global),
            (PluginName::Router, "=2.1.0", DeclarationLevel::Project),
            (PluginName::ApolloMcpServer, "latest", DeclarationLevel::Global),
        ]
    )]
    #[case::a_project_declaring_what_global_does_hides_it(
        Some(GLOBAL),
        Some(indoc! {r#"
            plugins:
              supergraph: "=2.9.3"
              router: "2"
        "#}),
        vec![
            (PluginName::Supergraph, "=2.9.3", DeclarationLevel::Project),
            (PluginName::Router, "2", DeclarationLevel::Project),
        ]
    )]
    #[case::no_project_means_only_global_declarations(
        Some(GLOBAL),
        None,
        vec![
            (PluginName::Supergraph, "2", DeclarationLevel::Global),
            (PluginName::Router, "latest", DeclarationLevel::Global),
        ]
    )]
    #[case::an_empty_project_changes_nothing(
        Some(GLOBAL),
        Some(""),
        vec![
            (PluginName::Supergraph, "2", DeclarationLevel::Global),
            (PluginName::Router, "latest", DeclarationLevel::Global),
        ]
    )]
    #[case::no_global_manifest(
        None,
        Some(indoc! {r#"
            plugins:
              router: "=2.1.0"
        "#}),
        vec![(PluginName::Router, "=2.1.0", DeclarationLevel::Project)]
    )]
    #[case::no_manifest_at_all(None, None, vec![])]
    fn declarations_layer_per_plugin(
        #[case] global: Option<&str>,
        #[case] project: Option<&str>,
        #[case] expected: Vec<(PluginName, &str, DeclarationLevel)>,
    ) {
        let global = global.map(manifest);
        let project = project.map(manifest);

        let layered = LayeredDeclarations::new(global.as_ref(), project.as_ref());

        assert_that!(summary(&layered)).is_equal_to(
            expected
                .into_iter()
                .map(|(plugin, written, level)| (plugin, written.to_string(), level))
                .collect::<Vec<_>>(),
        );
    }

    /// `.rover/` directories for both levels, with `global` and `project` as
    /// their manifests' contents where given.
    fn levels(global: Option<&str>, project: Option<&str>) -> (assert_fs::TempDir, ManifestDirs) {
        let temp = assert_fs::TempDir::new().unwrap();
        let root = camino::Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        let dirs = ManifestDirs {
            global: Some(root.join("home/.rover")),
            project: Some(root.join("work/.rover")),
        };
        for (dir, contents) in [(&dirs.global, global), (&dirs.project, project)] {
            let dir = dir.as_ref().unwrap();
            std::fs::create_dir_all(dir).unwrap();
            if let Some(contents) = contents {
                std::fs::write(dir.join(MANIFEST_FILE), contents).unwrap();
            }
        }
        (temp, dirs)
    }

    #[rstest]
    fn loading_layers_the_manifests_on_disk() {
        let (_temp, dirs) = levels(
            Some(GLOBAL),
            Some(indoc! {r#"
                plugins:
                  router: "=2.1.0"
            "#}),
        );

        let layered = LayeredDeclarations::load(&dirs).unwrap();

        assert_that!(summary(&layered)).is_equal_to(vec![
            (
                PluginName::Supergraph,
                "2".to_string(),
                DeclarationLevel::Global,
            ),
            (
                PluginName::Router,
                "=2.1.0".to_string(),
                DeclarationLevel::Project,
            ),
        ]);
    }

    #[rstest]
    fn a_level_without_a_manifest_contributes_nothing() {
        // Both `.rover/` directories exist; only the project's has a manifest.
        let (_temp, dirs) = levels(
            None,
            Some(indoc! {r#"
                plugins:
                  router: latest
            "#}),
        );

        let layered = LayeredDeclarations::load(&dirs).unwrap();

        assert_that!(summary(&layered)).is_equal_to(vec![(
            PluginName::Router,
            "latest".to_string(),
            DeclarationLevel::Project,
        )]);
    }

    #[rstest]
    fn no_project_loads_the_global_level_alone() {
        let (_temp, mut dirs) = levels(Some(GLOBAL), None);
        dirs.project = None;

        let layered = LayeredDeclarations::load(&dirs).unwrap();

        assert_that!(summary(&layered)).is_equal_to(vec![
            (
                PluginName::Supergraph,
                "2".to_string(),
                DeclarationLevel::Global,
            ),
            (
                PluginName::Router,
                "latest".to_string(),
                DeclarationLevel::Global,
            ),
        ]);
    }

    #[rstest]
    #[case::at_the_project_level(Some(GLOBAL), Some(BROKEN), DeclarationLevel::Project)]
    // Even though the project declares every plugin the global level does.
    #[case::at_the_global_level(
        Some(BROKEN),
        Some(indoc! {r#"
            plugins:
              supergraph: "2"
              router: "2"
              apollo-mcp-server: latest
        "#}),
        DeclarationLevel::Global
    )]
    #[case::the_project_is_reported_when_both_are_broken(
        Some(BROKEN),
        Some(BROKEN),
        DeclarationLevel::Project
    )]
    fn an_unusable_manifest_at_either_level_fails_the_load(
        #[case] global: Option<&str>,
        #[case] project: Option<&str>,
        #[case] culprit: DeclarationLevel,
    ) {
        let (_temp, dirs) = levels(global, project);

        let error = LayeredDeclarations::load(&dirs).expect_err("should not load");

        let culprit = match culprit {
            DeclarationLevel::Project => &dirs.project,
            DeclarationLevel::Global => &dirs.global,
        }
        .as_ref()
        .unwrap()
        .join(MANIFEST_FILE);
        let problem = std::error::Error::source(&*error).map(ToString::to_string);
        assert_that!((error.to_string(), problem)).is_equal_to((
            format!("`{culprit}` is not a valid manifest."),
            Some(
                "plugins: invalid type: sequence, expected a mapping of plugin name to version at \
                 line 2 column 3"
                    .to_string(),
            ),
        ));
    }

    const ALLOWED: &str = "allow_automatic_download: true\n";
    const REFUSED: &str = "allow_automatic_download: false\n";

    #[rstest]
    #[case::neither_level_sets_it(Some(GLOBAL), Some(GLOBAL), false)]
    #[case::no_manifest_at_all(None, None, false)]
    #[case::the_global_level_allows_it(Some(ALLOWED), None, true)]
    #[case::the_project_allows_it(None, Some(ALLOWED), true)]
    #[case::the_project_refuses_what_global_allows(Some(ALLOWED), Some(REFUSED), false)]
    #[case::the_project_allows_what_global_refuses(Some(REFUSED), Some(ALLOWED), true)]
    // A project manifest that leaves the key out leaves it to the global level.
    #[case::a_project_without_the_key_defers_to_global(Some(ALLOWED), Some(GLOBAL), true)]
    fn allow_automatic_download_layers_with_the_project_winning(
        #[case] global: Option<&str>,
        #[case] project: Option<&str>,
        #[case] expected: bool,
    ) {
        let global = global.map(manifest);
        let project = project.map(manifest);

        let layered = LayeredDeclarations::new(global.as_ref(), project.as_ref());

        assert_that!(layered.allow_automatic_download()).is_equal_to(expected);
    }

    #[rstest]
    fn loading_reads_allow_automatic_download_from_disk() {
        let (_temp, dirs) = levels(Some(ALLOWED), Some(GLOBAL));

        let layered = LayeredDeclarations::load(&dirs).unwrap();

        assert_that!(layered.allow_automatic_download()).is_true();
    }

    #[rstest]
    fn a_layered_declaration_keeps_the_whole_declaration() {
        let project = manifest(indoc! {r#"
            plugins:
              supergraph: latest-2
        "#});

        let layered = LayeredDeclarations::new(None, Some(&project));

        assert_that!(layered.get(PluginName::Supergraph)).is_equal_to(Some(&LayeredDeclaration {
            declaration: PluginDeclaration {
                request: VersionRequest::Major(2),
                written: "latest-2".to_string(),
            },
            level: DeclarationLevel::Project,
        }));
        assert_that!(layered.get(PluginName::Router)).is_none();
    }
}
