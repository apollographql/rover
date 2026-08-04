//! Native, in-process composition via the [`apollo_composition`] crate.
//!
//! ## Licensing
//!
//! Skipping the download does *not* skip the ELv2 license requirement. `apollo-composition` and
//! `apollo-federation` are ELv2-licensed and compiled into Rover, so the licensed code runs
//! either way — it just ships with the binary instead of being fetched.
//! [`CompositionPipeline::compose_native`](crate::composition::pipeline::CompositionPipeline)
//! requires acceptance before doing any work, matching the plugin install path.
//!
//! ## Federation version
//!
//! Three unrelated things are all called a "federation version", and conflating them is easy:
//!
//! 1. The **spec** version a subgraph `@link`s, e.g. `.../federation/v2.3`. Major/minor only.
//! 2. The `apollo-federation` **crate** version, e.g. `2.16.1`. crates.io numbering, and *not*
//!    a value users can put in `federation_version`.
//! 3. Rover's [`FederationVersion`] — what `federation_version:` in `supergraph.yaml` means: a
//!    published composition release, matching the plugin and `@apollo/composition` npm.
//!
//! Spec features are backported, so `apollo-federation` supports every spec version from
//! `v2.0` up to [`MAX_SUPPORTED_FEDERATION_SPEC`] — not only the newest. And because
//! composition release `2.N` is the release that first introduced spec `v2.N`, a
//! `federation_version: =2.N` pin in `supergraph.yaml` can be range-checked directly against
//! the spec ceiling — which is what [`NativeComposer::new`] does. Subgraphs that declare a
//! spec version beyond that ceiling are rejected by `apollo-federation` itself during
//! composition, so there is no separate pre-check here.

use apollo_composition::HybridComposition;
use apollo_federation_types::{
    build_plugin::{BuildMessage, BuildMessageLevel, PluginFailureReason, PluginResult},
    composition::Issue,
    config::FederationVersion,
    javascript::SubgraphDefinition,
    rover::{BuildError, BuildErrors, BuildHint},
};

use crate::composition::{
    CompositionError, CompositionSuccess, supergraph::config::full::FullyResolvedSupergraphConfig,
};

/// The newest federation spec version (major, minor) the pinned `apollo-federation` implements.
///
/// `apollo-federation` registers its supported specs in a private `FEDERATION_VERSIONS` table
/// with no public accessor, so this has to be mirrored here and updated when the crate is
/// bumped. [`tests::max_supported_federation_spec_is_accurate`] pins it from both sides by
/// composing subgraphs at this spec and at the next one up, so a stale value fails a test
/// rather than silently misreporting what we support.
const MAX_SUPPORTED_FEDERATION_SPEC: (u64, u64) = (2, 15);

/// Composes a supergraph in-process, without the `supergraph` plugin binary.
#[derive(Debug)]
pub struct NativeComposer;

impl NativeComposer {
    /// Creates a composer for the given federation version, or errors if native composition
    /// cannot honor it.
    ///
    /// [`FederationVersion::LatestFedTwo`] is always accepted — that is what an unpinned
    /// `supergraph.yaml` resolves to. Exact fed-2 pins are accepted up to this build's
    /// [`MAX_SUPPORTED_FEDERATION_SPEC`], a pin above that ceiling returns
    /// [`CompositionError::NativeCompositionSpecTooNew`]. Fed-1 versions always return
    /// [`CompositionError::NativeCompositionRequiresFedTwo`].
    ///
    /// See the module docs on the three meanings of "federation version".
    pub fn new(federation_version: &FederationVersion) -> Result<Self, CompositionError> {
        match federation_version {
            FederationVersion::LatestFedTwo => {}
            // Fed-1 *subgraphs* are upgraded and compose fine; it's fed-1 composition, which
            // always produced a fed-1 supergraph, that has no native equivalent.
            FederationVersion::LatestFedOne | FederationVersion::ExactFedOne(_) => {
                return Err(CompositionError::NativeCompositionRequiresFedTwo {
                    requested: federation_version.clone(),
                });
            }
            FederationVersion::ExactFedTwo(requested) => {
                // Comparing a release version against a spec range is only valid because
                // release `2.N` is the one that introduced spec `v2.N`.
                if (requested.major, requested.minor) > MAX_SUPPORTED_FEDERATION_SPEC {
                    return Err(CompositionError::NativeCompositionSpecTooNew {
                        requested: federation_version.clone(),
                        max_supported: format!(
                            "{}.{}",
                            MAX_SUPPORTED_FEDERATION_SPEC.0, MAX_SUPPORTED_FEDERATION_SPEC.1
                        ),
                    });
                }
            }
        }

        Ok(Self)
    }

    /// Composes the given subgraphs into a supergraph SDL.
    ///
    /// # Warning: blocks the async runtime
    ///
    /// This is CPU-bound work. Calling it from an async context will block the Tokio
    /// worker thread for the duration of composition stalling other tasks on that
    /// thread. Wrap in [`tokio::task::spawn_blocking`] if called from a long-running
    /// async context such as watch mode.
    pub async fn compose(
        self,
        supergraph_config: &FullyResolvedSupergraphConfig,
    ) -> Result<CompositionSuccess, CompositionError> {
        let federation_version = supergraph_config.federation_version().clone();
        let subgraph_definitions = subgraph_definitions(supergraph_config);

        // Must stay `experimental_compose`: the plain `compose` is the hybrid flow that calls
        // out to `@apollo/composition`, which is exactly what this path replaces.
        let result = self.experimental_compose(subgraph_definitions).await;

        into_composition_result(result, federation_version)
    }
}

/// Flattens a fully-resolved supergraph config into what `apollo-composition` wants.
fn subgraph_definitions(
    supergraph_config: &FullyResolvedSupergraphConfig,
) -> Vec<SubgraphDefinition> {
    supergraph_config
        .subgraphs()
        .iter()
        .map(|(name, subgraph)| SubgraphDefinition {
            name: name.clone(),
            // A missing routing URL is legal (e.g. connector-only subgraphs), and empty is how
            // the plugin sees it too when the YAML key is absent.
            url: subgraph.routing_url.clone().unwrap_or_default(),
            sdl: subgraph.schema().clone(),
        })
        .collect()
}

/// Translates `apollo-composition`'s result into Rover's composition types.
///
/// `apollo-federation-types` supplies every hop (`Issue` -> `BuildMessage` ->
/// `BuildHint`/`BuildError`), so all this decides is which bucket each message lands in.
fn into_composition_result(
    result: Result<PluginResult, Vec<Issue>>,
    federation_version: FederationVersion,
) -> Result<CompositionSuccess, CompositionError> {
    let plugin_result = match result {
        Ok(plugin_result) => plugin_result,
        // These issues are a mix of severities, not just errors; `build_failure` filters.
        Err(issues) => {
            return Err(build_failure(
                issues.into_iter().map(BuildMessage::from).collect(),
                federation_version,
            ));
        }
    };

    let PluginResult {
        result,
        build_messages,
        ..
    } = plugin_result;

    let supergraph_sdl = match result {
        Ok(supergraph_sdl) => supergraph_sdl,
        // Unreachable today — `experimental_compose` signals failure via `Err(Vec<Issue>)` — but
        // `PluginResult` can also encode it inline, and honouring that beats returning a
        // success with no SDL.
        Err(reason) => {
            let mut messages = build_messages;
            if !messages
                .iter()
                .any(|message| message.level == BuildMessageLevel::Error)
            {
                // `build_failure` would substitute a generic message here; naming the actual
                // `PluginFailureReason` is more useful.
                messages.push(BuildMessage::new_error(
                    format!(
                        "Composition failed: {}",
                        failure_reason_description(&reason)
                    ),
                    Some("NATIVE_COMPOSITION".to_string()),
                    Some("COMPOSITION_FAILURE".to_string()),
                ));
            }
            return Err(build_failure(messages, federation_version));
        }
    };

    // If composition claimed success but still reported errors, trust the messages.
    let (errors, hints): (Vec<_>, Vec<_>) = build_messages
        .into_iter()
        .partition(|message| message.level == BuildMessageLevel::Error);

    if !errors.is_empty() {
        return Err(build_failure(errors, federation_version));
    }

    Ok(CompositionSuccess {
        supergraph_sdl,
        // Every non-error message becomes a hint, including `Debug`-level ones. `Debug` here is
        // `apollo-federation`'s `HintLevel::Debug` — a low-importance composition hint, not
        // internal tracing — and the plugin reports those too, so filtering them would silently
        // lose hints relative to the plugin path.
        hints: hints.into_iter().map(BuildHint::from).collect(),
        federation_version,
    })
}

/// Builds a [`CompositionError::Build`], keeping only error-level messages.
///
/// The filter is load-bearing. `experimental_compose` appends WARN-level merge hints to the
/// error list it returns, and `BuildError::from` hardcodes `BuildErrorType::Composition` without
/// looking at the level — so unfiltered, hints would be reported to the user as composition
/// errors. `BuildErrors` has nowhere to put non-errors, and the plugin's `BuildResult` drops
/// hints on failure too, so discarding them keeps the two paths in agreement.
fn build_failure(
    messages: Vec<BuildMessage>,
    federation_version: FederationVersion,
) -> CompositionError {
    let mut errors: Vec<BuildError> = messages
        .into_iter()
        .filter(|message| message.level == BuildMessageLevel::Error)
        .map(BuildError::from)
        .collect();

    if errors.is_empty() {
        errors.push(BuildError::from(BuildMessage::new_error(
            "Composition failed, but reported no errors. This is a bug in Rover; please report \
             it at https://github.com/apollographql/rover/issues."
                .to_string(),
            Some("NATIVE_COMPOSITION".to_string()),
            Some("COMPOSITION_FAILURE".to_string()),
        )));
    }

    CompositionError::Build {
        source: errors.into_iter().collect::<BuildErrors>(),
        federation_version,
    }
}

const fn failure_reason_description(reason: &PluginFailureReason) -> &'static str {
    match reason {
        PluginFailureReason::Build => "the subgraphs could not be composed",
        PluginFailureReason::Config => "the composition configuration was invalid",
        PluginFailureReason::InternalFailure => "composition failed for an internal reason",
        // `PluginFailureReason` is `#[non_exhaustive]`.
        _ => "composition failed for an unknown reason",
    }
}

/// These four are required trait methods, but they exist for consumers that splice JavaScript
/// into individual phases. [`HybridComposition::experimental_compose`] is a provided method that
/// is all-Rust and calls none of them, so implementing the trait is pure obligation.
///
/// `unreachable!()` is safe here rather than a fallback path because `apollo-composition` is
/// pinned to an exact version, so this can only change on a deliberate bump — and
/// [`tests::composes_two_subgraphs_natively`] drives the real code, so a bump that starts
/// routing through a hook fails CI instead of panicking for a user.
impl HybridComposition for NativeComposer {
    async fn compose_services_without_satisfiability(
        &mut self,
        _subgraph_definitions: Vec<SubgraphDefinition>,
    ) -> Option<apollo_composition::SupergraphSdl<'_>> {
        unreachable!("{JS_HOOK_CALLED}")
    }

    async fn validate_satisfiability(&mut self) -> Result<Vec<Issue>, Vec<Issue>> {
        unreachable!("{JS_HOOK_CALLED}")
    }

    fn update_supergraph_sdl(&mut self, _supergraph_sdl: String) {
        unreachable!("{JS_HOOK_CALLED}")
    }

    fn add_issues<Source: Iterator<Item = Issue>>(&mut self, _issues: Source) {
        unreachable!("{JS_HOOK_CALLED}")
    }
}

const JS_HOOK_CALLED: &str = "apollo-composition called a JavaScript hook from experimental_compose, which it did not do \
     at the pinned version; revisit the native composition path before bumping the crate";

#[cfg(test)]
mod tests {
    use apollo_federation_types::{
        build_plugin::BuildMessageLevel, composition::Severity, config::SchemaSource,
        rover::BuildErrorType,
    };
    use rstest::rstest;
    use semver::Version;
    use speculoos::prelude::*;

    use super::*;
    use crate::composition::supergraph::config::full::FullyResolvedSubgraph;

    fn subgraph(name: &str, sdl: &str) -> FullyResolvedSubgraph {
        FullyResolvedSubgraph::builder()
            .name(name.to_string())
            .schema(sdl.to_string())
            .routing_url(format!("http://{name}.example.com"))
            .schema_source(SchemaSource::Sdl {
                sdl: sdl.to_string(),
            })
            .build()
    }

    fn supergraph_config(
        subgraphs: Vec<(&str, &str)>,
        federation_version: FederationVersion,
    ) -> FullyResolvedSupergraphConfig {
        FullyResolvedSupergraphConfig::builder()
            .subgraphs(
                subgraphs
                    .into_iter()
                    .map(|(name, sdl)| (name.to_string(), subgraph(name, sdl)))
                    .collect::<std::collections::BTreeMap<_, _>>(),
            )
            .federation_version(federation_version)
            .build()
    }

    const FED_TWO_LINK: &str =
        r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key"])"#;

    /// Connectors needs its own `@link` and an `@source` to point connectors at.
    const CONNECT_PREAMBLE: &str = r#"extend schema
      @link(url: "https://specs.apollo.dev/federation/v2.10", import: ["@key"])
      @link(url: "https://specs.apollo.dev/connect/v0.1", import: ["@source", "@connect"])
      @source(name: "api", http: { baseURL: "http://127.0.0.1" })"#;

    fn connectors_subgraph(source: &str, selection: &str) -> String {
        format!(
            "{CONNECT_PREAMBLE}\n\
             type Query {{ thing: Thing @connect(source: \"{source}\", \
             http: {{ GET: \"/thing\" }}, selection: \"{selection}\") }}\n\
             type Thing {{ id: ID! name: String }}"
        )
    }

    /// `@cacheTag` comes from the federation spec at v2.12+, not from a separate `cacheTag` link
    /// — linking one directly is an `INVALID_LINK_DIRECTIVE_USAGE` error instead. Both formats
    /// here are invalid: `{ namedField }` is malformed, and `$args` is not allowed on an entity.
    const CACHE_TAG_INVALID: &str = r#"extend schema
      @link(url: "https://specs.apollo.dev/federation/v2.12", import: ["@key", "@cacheTag"])
    type Product @key(fields: "upc") @cacheTag(format: "{ namedField }") {
      upc: String!
      name: String
    }
    type Query {
      topProducts(first: Int = 5): [Product] @cacheTag(format: "{$args}")
    }"#;

    /// Collects the codes of a `CompositionError::Build`, failing on any other variant.
    fn build_error_codes(error: CompositionError) -> Vec<String> {
        let CompositionError::Build { source, .. } = error else {
            panic!("expected CompositionError::Build, got {error:?}");
        };
        source.iter().filter_map(|e| e.code.clone()).collect()
    }

    async fn compose_all(
        subgraphs: Vec<(&str, &str)>,
    ) -> Result<CompositionSuccess, CompositionError> {
        let config = supergraph_config(subgraphs, FederationVersion::LatestFedTwo);
        NativeComposer::new(config.federation_version())
            .expect("LatestFedTwo is supported")
            .compose(&config)
            .await
    }

    /// The common case: an unpinned `supergraph.yaml` resolves to this.
    #[test]
    fn accepts_latest_fed_two() {
        assert_that!(NativeComposer::new(&FederationVersion::LatestFedTwo).map(|_| ())).is_ok();
    }

    /// Spec features are backported, so pinning an older composition release is supported.
    #[rstest]
    #[case::oldest(Version::new(2, 0, 0))]
    #[case::mid_range(Version::new(2, 9, 0))]
    #[case::max_supported(Version::new(
        MAX_SUPPORTED_FEDERATION_SPEC.0,
        MAX_SUPPORTED_FEDERATION_SPEC.1,
        0
    ))]
    fn accepts_exact_pins_within_spec_support(#[case] version: Version) {
        assert_that!(NativeComposer::new(&FederationVersion::ExactFedTwo(version)).map(|_| ()))
            .is_ok();
    }

    /// A pin above our spec range asks for features this build doesn't have, so it must fail
    /// rather than compose at something older and call it that version.
    #[test]
    fn rejects_exact_pins_newer_than_spec_support() {
        let too_new = Version::new(
            MAX_SUPPORTED_FEDERATION_SPEC.0,
            MAX_SUPPORTED_FEDERATION_SPEC.1 + 1,
            0,
        );
        let error = NativeComposer::new(&FederationVersion::ExactFedTwo(too_new))
            .expect_err("a pin newer than our spec support cannot be honoured");

        assert_that!(matches!(
            error,
            CompositionError::NativeCompositionSpecTooNew { .. }
        ))
        .is_true();
    }

    #[rstest]
    #[case::latest(FederationVersion::LatestFedOne)]
    #[case::exact(FederationVersion::ExactFedOne(Version::new(0, 36, 0)))]
    fn rejects_federation_one(#[case] version: FederationVersion) {
        let error =
            NativeComposer::new(&version).expect_err("native composition is federation 2 only");

        assert_that!(matches!(
            error,
            CompositionError::NativeCompositionRequiresFedTwo { .. }
        ))
        .is_true();
    }

    /// Fails when the `supergraph` plugin version CI tests against has outrun what native
    /// composition implements.
    ///
    /// This is the one signal that native composition is falling behind, and it deliberately
    /// fails rather than warns: the bump that triggers it arrives as an automated Renovate PR, so
    /// anything quieter would merge unnoticed.
    ///
    /// It lives here, in the fast unit-test leg, rather than in the e2e parity suite. The parity
    /// tests pin their own version so that they keep working when the plugin moves ahead —
    /// otherwise the coverage would go dark exactly when it matters, and every matrix leg would
    /// red with a misleading message instead of this one.
    #[test]
    fn native_composition_keeps_up_with_the_plugin_version_ci_tests() {
        const MANIFEST: &str = include_str!("../../../latest_plugin_versions.json");

        let manifest: serde_json::Value =
            serde_json::from_str(MANIFEST).expect("latest_plugin_versions.json is not valid JSON");
        let latest = manifest["supergraph"]["versions"]["latest-2"]
            .as_str()
            .expect("`supergraph.versions.latest-2` is missing or not a string");
        let latest = Version::parse(latest.trim_start_matches('v'))
            .expect("`supergraph.versions.latest-2` is not a semver version");

        let (major, minor) = MAX_SUPPORTED_FEDERATION_SPEC;
        assert!(
            (latest.major, latest.minor) <= (major, minor),
            "CI tests against supergraph plugin {latest}, but native composition only implements \
             federation specs up to {major}.{minor}, so it can no longer compose what the plugin \
             can.\n\n\
             To fix: bump `apollo-composition` in Cargo.toml and update \
             MAX_SUPPORTED_FEDERATION_SPEC to the spec ceiling of the `apollo-federation` it \
             pins.\n\n\
             If native composition cannot catch up yet, this is the decision to make \
             deliberately: users passing `--native-composition` will be unable to compose at the \
             version everything else defaults to."
        );
    }

    /// Pins [`MAX_SUPPORTED_FEDERATION_SPEC`] against what `apollo-federation` actually accepts,
    /// from both directions: a subgraph at that spec must compose, and one at the next minor up
    /// must be rejected. If the crate gains a spec version, the second half fails and the
    /// constant needs bumping; if it drops one, the first half fails.
    #[tokio::test]
    async fn max_supported_federation_spec_is_accurate() {
        async fn composes_at_spec(major: u64, minor: u64) -> bool {
            let sdl = format!(
                r#"extend schema @link(url: "https://specs.apollo.dev/federation/v{major}.{minor}", import: ["@key"])
                type Query {{ thing: Thing }}
                type Thing @key(fields: "id") {{ id: ID! }}"#
            );
            let config = supergraph_config(vec![("one", &sdl)], FederationVersion::LatestFedTwo);
            NativeComposer::new(&FederationVersion::LatestFedTwo)
                .expect("LatestFedTwo is accepted")
                .compose(&config)
                .await
                .is_ok()
        }

        let (major, minor) = MAX_SUPPORTED_FEDERATION_SPEC;
        assert!(
            composes_at_spec(major, minor).await,
            "expected federation spec v{major}.{minor} to be supported"
        );
        assert!(
            !composes_at_spec(major, minor + 1).await,
            "federation spec v{major}.{} now composes; bump MAX_SUPPORTED_FEDERATION_SPEC",
            minor + 1
        );
    }

    /// Real subgraphs through the real crate — no plugin binary, no JavaScript, no mocks.
    #[tokio::test]
    async fn composes_two_subgraphs_natively() {
        let config = supergraph_config(
            vec![
                (
                    "products",
                    &format!(
                        "{FED_TWO_LINK}
                        type Query {{ products: [Product!]! }}
                        type Product @key(fields: \"id\") {{ id: ID! name: String! }}"
                    ),
                ),
                (
                    "reviews",
                    &format!(
                        "{FED_TWO_LINK}
                        type Query {{ reviews: [Review!]! }}
                        type Review {{ id: ID! body: String! product: Product! }}
                        type Product @key(fields: \"id\") {{ id: ID! }}"
                    ),
                ),
            ],
            FederationVersion::LatestFedTwo,
        );

        let composer = NativeComposer::new(config.federation_version())
            .expect("LatestFedTwo is supported natively");

        let success = composer
            .compose(&config)
            .await
            .expect("two valid subgraphs should compose");

        assert_that!(&success.supergraph_sdl).contains("PRODUCTS");
        assert_that!(&success.supergraph_sdl).contains("REVIEWS");
        assert_that!(&success.supergraph_sdl).contains("join__Graph");
        // Contributed only by `reviews`, so this proves the merge happened.
        assert_that!(&success.supergraph_sdl).contains("body");

        assert_that!(&success.federation_version).is_equal_to(FederationVersion::LatestFedTwo);
    }

    /// Connectors validation and expansion live in `apollo-composition`, above plain merge, so
    /// this is the one part of the pipeline that merge-only fixtures never reach.
    #[tokio::test]
    async fn expands_connectors_into_the_supergraph() {
        let subgraph = connectors_subgraph("api", "id name");
        let success = compose_all(vec![("api", &subgraph)])
            .await
            .expect("a valid connectors subgraph should compose");

        // Both prove expansion actually ran, rather than the connector passing through untouched.
        assert_that!(&success.supergraph_sdl).contains("specs.apollo.dev/connect");
        assert_that!(&success.supergraph_sdl).contains(r#"@join__directive(name: "source""#);
    }

    /// Validation that only `apollo-composition` performs — connectors and `@cacheTag` — has to
    /// reach the user through the same `CompositionError::Build` as a merge failure.
    ///
    /// The expected codes were observed from the pinned crate, so if it changes what it reports
    /// these fail rather than silently altering Rover's output.
    #[rstest]
    #[case::connector_invalid_selection(connectors_subgraph("api", "id {"), "INVALID_SELECTION")]
    #[case::connector_unknown_source(connectors_subgraph("nope", "id"), "SOURCE_NAME_MISMATCH")]
    #[case::cache_tag_invalid_format(CACHE_TAG_INVALID.to_string(), "CACHE_TAG_INVALID_FORMAT")]
    #[tokio::test]
    async fn surfaces_validation_errors_as_build_errors(
        #[case] sdl: String,
        #[case] expected_code: &str,
    ) {
        let error = compose_all(vec![("api", &sdl)])
            .await
            .expect_err("this subgraph should fail validation");

        let codes = build_error_codes(error);
        assert_that!(&codes).contains(expected_code.to_string());
    }

    /// `Severity::Info` is the level most composition hints actually use, so it must survive as a
    /// hint — neither dropped nor promoted into the error list.
    #[rstest]
    #[case::overridden_field_can_be_removed(
        &format!("{FED_TWO_LINK}\ntype Query {{ t: T }}\ntype T @key(fields: \"id\") {{ id: ID! f: String }}"),
        r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", "@override"])
           type T @key(fields: "id") { id: ID! f: String @override(from: "a") }"#,
        "OVERRIDDEN_FIELD_CAN_BE_REMOVED"
    )]
    #[case::inconsistent_but_compatible_field_type(
        r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", "@shareable"])
           type Query { v: V }
           type V { x: String @shareable }"#,
        r#"extend schema @link(url: "https://specs.apollo.dev/federation/v2.3", import: ["@key", "@shareable"])
           type V { x: String! @shareable }"#,
        "INCONSISTENT_BUT_COMPATIBLE_FIELD_TYPE"
    )]
    #[tokio::test]
    async fn reports_info_level_hints(
        #[case] a: &str,
        #[case] b: &str,
        #[case] expected_code: &str,
    ) {
        let success = compose_all(vec![("a", a), ("b", b)])
            .await
            .expect("hints alone should not fail composition");

        let codes: Vec<_> = success
            .hints
            .iter()
            .filter_map(|h| h.code.clone())
            .collect();
        assert_that!(&codes).contains(expected_code.to_string());
    }

    /// Must surface as `CompositionError::Build` — the same variant the plugin produces — so
    /// Rover's existing output and exit-code handling works unchanged.
    #[tokio::test]
    async fn reports_composition_errors_as_build_errors() {
        let config = supergraph_config(
            vec![
                (
                    "one",
                    &format!(
                        "{FED_TWO_LINK}
                        type Query {{ thing: Thing }}
                        type Thing @key(fields: \"id\") {{ id: ID! shared: Int! }}"
                    ),
                ),
                (
                    "two",
                    &format!(
                        "{FED_TWO_LINK}
                        type Query {{ other: Thing }}
                        type Thing @key(fields: \"id\") {{ id: ID! shared: String! }}"
                    ),
                ),
            ],
            FederationVersion::LatestFedTwo,
        );

        let error = NativeComposer::new(config.federation_version())
            .expect("LatestFedTwo is supported natively")
            .compose(&config)
            .await
            .expect_err("mismatched field types should fail composition");

        let CompositionError::Build { source, .. } = error else {
            panic!("expected CompositionError::Build, got {error:?}");
        };

        assert_that!(&source.iter().count()).is_greater_than(0);
        // Errors must be typed as composition errors so Rover's error codes stay correct.
        assert_that!(
            &source
                .iter()
                .all(|e| e.get_type() == BuildErrorType::Composition)
        )
        .is_true();
    }

    /// `experimental_compose` appends WARN-level merge hints to the error list it returns, and
    /// `BuildError::from` ignores level — so without filtering these would reach the user as
    /// composition errors.
    #[test]
    fn keeps_only_errors_from_a_mixed_severity_failure() {
        let issue = |code: &str, severity| Issue {
            code: code.to_string(),
            message: format!("{code} message"),
            locations: vec![],
            severity,
        };

        let error = into_composition_result(
            Err(vec![
                issue("REAL_ERROR", Severity::Error),
                issue("JUST_A_HINT", Severity::Warning),
            ]),
            FederationVersion::LatestFedTwo,
        )
        .expect_err("an Err from composition is a failure");

        let CompositionError::Build { source, .. } = error else {
            panic!("expected CompositionError::Build, got {error:?}");
        };

        let codes: Vec<_> = source.iter().filter_map(|e| e.code.clone()).collect();
        assert_that!(&codes).is_equal_to(vec!["REAL_ERROR".to_string()]);
    }

    #[test]
    fn treats_error_level_messages_as_failure_even_when_composition_claims_success() {
        let result = into_composition_result(
            Ok(PluginResult::new(
                Ok("type Query { hello: String }".to_string()),
                vec![BuildMessage::new_error(
                    "something went wrong".to_string(),
                    None,
                    Some("SOME_ERROR".to_string()),
                )],
            )),
            FederationVersion::LatestFedTwo,
        );

        assert_that!(matches!(result, Err(CompositionError::Build { .. }))).is_true();
    }

    #[test]
    fn keeps_every_non_error_message_as_a_hint() {
        let message = |text: &str, level| {
            let mut message = BuildMessage::new_error(text.to_string(), None, None);
            message.level = level;
            message
        };

        let success = into_composition_result(
            Ok(PluginResult::new(
                Ok("type Query { hello: String }".to_string()),
                vec![
                    message("a warning", BuildMessageLevel::Warn),
                    message("some info", BuildMessageLevel::Info),
                    // `HintLevel::Debug` is a low-importance composition hint, and the plugin
                    // reports it, so dropping it here would lose parity.
                    message("a debug hint", BuildMessageLevel::Debug),
                ],
            )),
            FederationVersion::LatestFedTwo,
        )
        .expect("non-error messages should not fail composition");

        let messages: Vec<_> = success.hints.iter().map(|h| h.message.clone()).collect();
        assert_that!(&messages).is_equal_to(vec![
            "a warning".to_string(),
            "some info".to_string(),
            "a debug hint".to_string(),
        ]);
    }

    #[test]
    fn inline_plugin_failure_always_produces_at_least_one_error() {
        let result = into_composition_result(
            Ok(PluginResult::new_failure(
                vec![],
                PluginFailureReason::InternalFailure,
            )),
            FederationVersion::LatestFedTwo,
        );

        let Err(CompositionError::Build { source, .. }) = result else {
            panic!("expected a build failure");
        };
        assert_that!(&source.iter().count()).is_equal_to(1);
    }
}
