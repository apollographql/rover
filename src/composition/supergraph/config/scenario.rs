use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

use anyhow::Result;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use rand::Rng;
use rover_client::shared::GraphRef;
use rstest::fixture;
use uuid::Uuid;

use super::unresolved::UnresolvedSubgraph;

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
    let mut rng = rand::thread_rng();
    let mut value = format!(
        "{}",
        ALPHA_CHARSET[rng.gen_range(0..ALPHA_CHARSET.len())] as char
    );
    let remaining = rng.gen_range(0..62);
    for _ in 0..remaining {
        let c = ADDITIONAL_CHARSET[rng.gen_range(0..ADDITIONAL_CHARSET.len())] as char;
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
    format!("{}\n{}", link_directive, sdl)
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
    pub _subgraph_name: String,
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
        _subgraph_name: subgraph_name.to_string(),
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
