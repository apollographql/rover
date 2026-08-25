use std::{
    collections::{BTreeMap, HashMap},
    io::Write,
    path::Path,
    str::FromStr,
    sync::Arc,
};

use anyhow::Result;
use apollo_federation_types::config::{FederationVersion, SchemaSource, SubgraphConfig};
use assert_fs::{
    TempDir,
    prelude::{FileTouch, FileWriteStr, PathChild},
};
use camino::Utf8PathBuf;
use mockall::predicate;
use rand::RngExt;
use rover_client::RoverClientError;
use rover_studio::types::GraphRef;
use rstest::fixture;
use tower::{ServiceBuilder, ServiceExt};
use uuid::Uuid;

use super::{
    error::ResolveSubgraphError,
    full::{
        FullyResolvedSubgraph,
        introspect::{MakeResolveIntrospectSubgraphRequest, ResolveIntrospectSubgraphFactory},
    },
    resolver::{
        InitializedSupergraphConfigResolver, SupergraphConfigResolver,
        fetch_remote_subgraph::{
            FetchRemoteSubgraphError, FetchRemoteSubgraphFactory, FetchRemoteSubgraphRequest,
            MakeFetchRemoteSubgraphError, RemoteSubgraph,
        },
        fetch_remote_subgraphs::{FetchRemoteSubgraphsRequest, MakeFetchRemoteSubgraphsError},
    },
    unresolved::UnresolvedSubgraph,
};
use crate::{
    config::SupergraphConfigYaml,
    utils::{effect::read_stdin::MockReadStdin, parsers::FileDescriptorType},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubgraphFederationVersion {
    One,
    Two,
}

impl SubgraphFederationVersion {
    pub fn is_fed_two(&self) -> bool {
        matches!(self, SubgraphFederationVersion::Two)
    }
}

fn graph_id_or_variant() -> String {
    const ALPHA_CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const ADDITIONAL_CHARSET: &[u8] =
        b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_";
    let mut rng = rand::rng();
    let mut value = format!(
        "{}",
        ALPHA_CHARSET[rng.random_range(0..ALPHA_CHARSET.len())] as char
    );
    let remaining = rng.random_range(0..62);
    for _ in 0..remaining {
        let c = ADDITIONAL_CHARSET[rng.random_range(0..ADDITIONAL_CHARSET.len())] as char;
        value.push(c);
    }
    value
}

#[fixture]
pub fn graph_ref() -> GraphRef {
    let graph = graph_id_or_variant();
    let variant = graph_id_or_variant();
    GraphRef::from_str(&format!("{graph}@{variant}")).unwrap()
}

#[fixture]
pub fn subgraph_name() -> String {
    format!("subgraph_{}", Uuid::new_v4().as_simple())
}

#[fixture]
pub fn sdl() -> String {
    format!(
        "type Query {{ test_{}: String! }}",
        Uuid::new_v4().as_simple()
    )
}

#[fixture]
pub fn sdl_fed2(sdl: String) -> String {
    let link_directive = "extend schema @link(url: \"https://specs.apollo.dev/federation/v2.3\", import: [\"@key\", \"@shareable\"])";
    format!("{link_directive}\n{sdl}")
}

#[fixture]
pub fn routing_url() -> String {
    format!("http://example.com/{}", Uuid::new_v4().as_simple())
}

#[derive(Clone, Debug)]
pub struct SdlSubgraphScenario {
    pub sdl: String,
    pub unresolved_subgraph: UnresolvedSubgraph,
    pub subgraph_federation_version: SubgraphFederationVersion,
    pub routing_url: String,
}

#[fixture]
pub fn sdl_subgraph_scenario(
    sdl: String,
    subgraph_name: String,
    #[default(SubgraphFederationVersion::One)]
    subgraph_federation_version: SubgraphFederationVersion,
    routing_url: String,
) -> SdlSubgraphScenario {
    let sdl = if subgraph_federation_version.is_fed_two() {
        sdl_fed2(sdl)
    } else {
        sdl
    };
    SdlSubgraphScenario {
        sdl: sdl.to_string(),
        unresolved_subgraph: UnresolvedSubgraph::new(
            subgraph_name,
            SubgraphConfig {
                schema: SchemaSource::Sdl { sdl },
                routing_url: Some(routing_url.to_string()),
            },
        ),
        subgraph_federation_version,
        routing_url,
    }
}

#[derive(Clone, Debug)]
pub struct RemoteSubgraphScenario {
    pub sdl: String,
    pub graph_ref: GraphRef,
    pub unresolved_subgraph: UnresolvedSubgraph,
    pub subgraph_name: String,
    pub routing_url: String,
    pub subgraph_federation_version: SubgraphFederationVersion,
}

#[fixture]
pub fn remote_subgraph_scenario(
    sdl: String,
    subgraph_name: String,
    routing_url: String,
    #[default(SubgraphFederationVersion::One)]
    subgraph_federation_version: SubgraphFederationVersion,
) -> RemoteSubgraphScenario {
    let graph_ref = graph_ref();
    let sdl = if subgraph_federation_version.is_fed_two() {
        sdl_fed2(sdl)
    } else {
        sdl
    };
    RemoteSubgraphScenario {
        sdl,
        graph_ref: graph_ref.clone(),
        unresolved_subgraph: UnresolvedSubgraph::new(
            subgraph_name.to_string(),
            SubgraphConfig {
                schema: SchemaSource::Subgraph {
                    graphref: graph_ref.to_string(),
                    subgraph: subgraph_name.to_string(),
                },
                routing_url: Some(routing_url.to_string()),
            },
        ),
        subgraph_name,
        routing_url,
        subgraph_federation_version,
    }
}

#[derive(Clone, Debug)]
pub struct IntrospectSubgraphScenario {
    pub sdl: String,
    pub routing_url: String,
    pub introspection_headers: HashMap<String, String>,
    pub unresolved_subgraph: UnresolvedSubgraph,
    pub subgraph_federation_version: SubgraphFederationVersion,
}

#[fixture]
pub fn introspect_subgraph_scenario(
    sdl: String,
    subgraph_name: String,
    routing_url: String,
    #[default(SubgraphFederationVersion::One)]
    subgraph_federation_version: SubgraphFederationVersion,
) -> IntrospectSubgraphScenario {
    let sdl = if subgraph_federation_version.is_fed_two() {
        sdl_fed2(sdl)
    } else {
        sdl
    };
    let introspection_headers = HashMap::from_iter([(
        "x-introspection-key".to_string(),
        "x-introspection-header".to_string(),
    )]);
    IntrospectSubgraphScenario {
        sdl,
        routing_url: routing_url.to_string(),
        introspection_headers: introspection_headers.clone(),
        unresolved_subgraph: UnresolvedSubgraph::new(
            subgraph_name,
            SubgraphConfig {
                schema: SchemaSource::SubgraphIntrospection {
                    subgraph_url: url::Url::from_str(&routing_url).unwrap(),
                    introspection_headers: Some(introspection_headers),
                },
                routing_url: Some(routing_url),
            },
        ),
        subgraph_federation_version,
    }
}

#[derive(Clone, Debug)]
pub struct FileSubgraphScenario {
    pub sdl: String,
    pub routing_url: String,
    pub schema_file_path: Utf8PathBuf,
    pub unresolved_subgraph: UnresolvedSubgraph,
    pub subgraph_federation_version: SubgraphFederationVersion,
}

impl FileSubgraphScenario {
    pub fn write_schema_file(&self, root_dir: &Path) -> Result<()> {
        let full_schema_path = Utf8PathBuf::try_from(root_dir.join(&self.schema_file_path))?;
        let mut file = std::fs::File::create(full_schema_path.as_std_path())?;
        file.write_all(self.sdl.as_bytes())?;
        Ok(())
    }
}

#[fixture]
pub fn file_subgraph_scenario(
    sdl: String,
    subgraph_name: String,
    routing_url: String,
    #[default(SubgraphFederationVersion::One)]
    subgraph_federation_version: SubgraphFederationVersion,
) -> FileSubgraphScenario {
    let sdl = if subgraph_federation_version.is_fed_two() {
        sdl_fed2(sdl)
    } else {
        sdl
    };
    let schema_file_path = Utf8PathBuf::from_str("schema.graphql").unwrap();
    FileSubgraphScenario {
        sdl,
        routing_url: routing_url.clone(),
        schema_file_path: schema_file_path.clone(),
        unresolved_subgraph: UnresolvedSubgraph::new(
            subgraph_name,
            SubgraphConfig {
                schema: SchemaSource::File {
                    file: schema_file_path.into_std_path_buf(),
                },
                routing_url: Some(routing_url),
            },
        ),
        subgraph_federation_version,
    }
}

/// Adds a `SubgraphConfig` for `sdl_subgraph_scenario` (if present) to `local_subgraphs`, keyed
/// as `"sdl-subgraph"`. Shared by several tests in `resolver::tests` and `pipeline::tests` that
/// build a `supergraph.yaml` containing an inline-SDL subgraph.
pub fn setup_sdl_subgraph_scenario(
    sdl_subgraph_scenario: Option<&SdlSubgraphScenario>,
    local_subgraphs: &mut BTreeMap<String, SubgraphConfig>,
) {
    if let Some(sdl_subgraph_scenario) = sdl_subgraph_scenario {
        let schema_source = SchemaSource::Sdl {
            sdl: sdl_subgraph_scenario.sdl.to_string(),
        };
        let subgraph_config = SubgraphConfig {
            routing_url: Some(routing_url()),
            schema: schema_source,
        };
        local_subgraphs.insert("sdl-subgraph".to_string(), subgraph_config);
    }
}

/// Writes `local_supergraph_config_str` to a temp `supergraph.yaml` (returning a
/// `FileDescriptorType::File` pointing at it), or mocks `read_stdin` to return it (returning
/// `FileDescriptorType::Stdin`), depending on `load_supergraph_config_from_file`. Shared by
/// several tests in `resolver::tests` and `pipeline::tests`.
pub fn setup_file_descriptor(
    load_supergraph_config_from_file: bool,
    local_supergraph_config_dir: &TempDir,
    local_supergraph_config_str: &str,
    mock_read_stdin: &mut MockReadStdin,
) -> Result<FileDescriptorType> {
    let file_descriptor_type = if load_supergraph_config_from_file {
        let local_supergraph_config_file = local_supergraph_config_dir.child("supergraph.yaml");
        local_supergraph_config_file.touch()?;
        local_supergraph_config_file.write_str(local_supergraph_config_str)?;
        let path =
            Utf8PathBuf::from_path_buf(local_supergraph_config_file.path().to_path_buf()).unwrap();
        mock_read_stdin.expect_read_stdin().times(0);
        FileDescriptorType::File(path)
    } else {
        mock_read_stdin
            .expect_read_stdin()
            .times(1)
            .with(predicate::eq("supergraph config"))
            .returning({
                let local_supergraph_config_str = local_supergraph_config_str.to_string();
                move |_| Ok(local_supergraph_config_str.to_string())
            });
        FileDescriptorType::Stdin
    };
    Ok(file_descriptor_type)
}

/// Builds an `InitializedSupergraphConfigResolver` for the scenario exercised by both
/// `resolver::tests::test_fully_resolve_subgraphs_errors_on_fed_one_pin_with_fed_two_subgraph`
/// and `pipeline::tests::resolve_federation_version_rejects_fed_one_pin_with_fed_two_subgraph`:
/// a `federation_version: 1` pin in `supergraph.yaml` alongside one Federation-2 (`@link`-using)
/// subgraph. Returns the resolver, the two factories needed to drive `fully_resolve_subgraphs`
/// (directly, or via `CompositionPipeline::resolve_federation_version`), the supergraph root
/// path, and the backing `TempDir` -- keep the `TempDir` alive for as long as the resolver and
/// factories are in use.
pub async fn fed_one_pin_with_fed_two_subgraph_resolver() -> (
    InitializedSupergraphConfigResolver,
    ResolveIntrospectSubgraphFactory,
    FetchRemoteSubgraphFactory,
    Utf8PathBuf,
    TempDir,
) {
    let subgraph_scenario = sdl_subgraph_scenario(
        sdl_fed2(sdl()),
        subgraph_name(),
        SubgraphFederationVersion::Two,
        routing_url(),
    );

    let mut local_subgraphs = BTreeMap::new();
    setup_sdl_subgraph_scenario(Some(&subgraph_scenario), &mut local_subgraphs);

    let supergraph_config = SupergraphConfigYaml {
        subgraphs: local_subgraphs,
        federation_version: Some(FederationVersion::LatestFedOne),
    };
    let supergraph_config_str = serde_yaml::to_string(&supergraph_config).unwrap();

    let local_supergraph_config_dir = TempDir::new().expect("Couldn't create temp dir.");
    let mut mock_read_stdin = MockReadStdin::new();
    let file_descriptor_type = setup_file_descriptor(
        true,
        &local_supergraph_config_dir,
        &supergraph_config_str,
        &mut mock_read_stdin,
    )
    .expect("Couldn't setup file descriptor.");

    let (fetch_remote_subgraphs_service, _) =
        tower_test::mock::spawn::<FetchRemoteSubgraphsRequest, BTreeMap<String, SubgraphConfig>>();
    let fetch_remote_subgraphs_factory =
        ServiceBuilder::new()
            .boxed_clone()
            .service_fn(move |_: ()| {
                let fetch_remote_subgraphs_service = fetch_remote_subgraphs_service.clone();
                async move {
                    Ok::<_, MakeFetchRemoteSubgraphsError>(
                        ServiceBuilder::new()
                            .map_err(RoverClientError::ServiceReady)
                            .service(fetch_remote_subgraphs_service.into_inner())
                            .boxed_clone(),
                    )
                }
            });

    let resolver =
        SupergraphConfigResolver::load_remote_subgraphs(fetch_remote_subgraphs_factory, None)
            .await
            .expect("Couldn't load remote subgraphs.")
            .load_from_file_descriptor(&mut mock_read_stdin, Some(&file_descriptor_type))
            .expect("Couldn't load local subgraphs.")
            .skip_default_subgraph();

    let (fetch_remote_subgraph_service, _) =
        tower_test::mock::spawn::<FetchRemoteSubgraphRequest, RemoteSubgraph>();
    let fetch_remote_subgraph_factory: FetchRemoteSubgraphFactory = ServiceBuilder::new()
        .boxed_clone()
        .service_fn(move |_: ()| {
            let fetch_remote_subgraph_service = fetch_remote_subgraph_service.clone();
            async move {
                Ok::<_, MakeFetchRemoteSubgraphError>(
                    ServiceBuilder::new()
                        .map_err(FetchRemoteSubgraphError::Service)
                        .service(fetch_remote_subgraph_service.into_inner())
                        .boxed_clone(),
                )
            }
        });

    let (resolve_introspect_subgraph_service, mut resolve_introspect_subgraph_handle) =
        tower_test::mock::spawn::<(), FullyResolvedSubgraph>();
    // we never introspect subgraphs in this scenario, but we still have to account for the effect
    resolve_introspect_subgraph_handle.allow(0);
    let resolve_introspect_subgraph_factory: ResolveIntrospectSubgraphFactory =
        ServiceBuilder::new().boxed_clone().service_fn(
            move |_: MakeResolveIntrospectSubgraphRequest| {
                let resolve_introspect_subgraph_service =
                    resolve_introspect_subgraph_service.clone();
                async move {
                    Ok(ServiceBuilder::new()
                        .boxed_clone()
                        .map_err(|err| ResolveSubgraphError::IntrospectionError {
                            subgraph_name: "dont-call-me".to_string(),
                            source: Arc::new(err),
                        })
                        .service(resolve_introspect_subgraph_service.into_inner()))
                }
            },
        );

    let supergraph_root =
        Utf8PathBuf::from_path_buf(local_supergraph_config_dir.path().to_path_buf()).unwrap();

    (
        resolver,
        resolve_introspect_subgraph_factory,
        fetch_remote_subgraph_factory,
        supergraph_root,
        local_supergraph_config_dir,
    )
}
