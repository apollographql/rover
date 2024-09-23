use std::fmt::Debug;

use apollo_federation_types::{
    config::FederationVersion,
    rover::{BuildErrors, BuildHint},
};
use camino::Utf8PathBuf;
use derive_getters::Getters;

pub mod events;
pub mod run_composition;
pub mod runner;
pub mod supergraph;
pub mod types;

#[cfg(feature = "composition-js")]
mod watchers;

#[derive(Getters, Debug, Clone, Eq, PartialEq)]
pub struct CompositionSuccess {
    supergraph_sdl: String,
    hints: Vec<BuildHint>,
    federation_version: FederationVersion,
}

#[derive(Eq, PartialEq, thiserror::Error, Debug)]
pub enum CompositionError {
    #[error("Failed to run the composition binary")]
    Binary { error: String },
    #[error("Failed to parse output of `{binary} compose`")]
    InvalidOutput { binary: Utf8PathBuf, error: String },
    #[error("Invalid input for `{binary} compose`")]
    InvalidInput { binary: Utf8PathBuf, error: String },
    #[error("Failed to read the file at: {path}")]
    ReadFile { path: Utf8PathBuf, error: String },
    #[error("Encountered {} while trying to build a supergraph.", .source.length_string())]
    Build {
        source: BuildErrors,
        // NB: in do_compose (rover_client/src/error -> BuildErrors) this includes num_subgraphs,
        // but this is only important if we end up with a RoverError (it uses a singular or plural
        // error message); so, leaving TBD if we go that route because it'll require figuring out
        // from something like the supergraph_config how many subgraphs we attempted to compose
        // (alternatively, we could just reword the error message to allow for either)
    },
}

#[cfg(test)]
pub fn compose_output() -> String {
    "{\"Ok\":{\"supergraphSdl\":\"schema\\n  @link(url: \\\"https://specs.apollo.dev/link/v1.0\\\")\\n  @link(url: \\\"https://specs.apollo.dev/join/v0.3\\\", for: EXECUTION)\\n  @link(url: \\\"https://specs.apollo.dev/tag/v0.3\\\", import: [\\\"@tag\\\"])\\n  @link(url: \\\"https://specs.apollo.dev/inaccessible/v0.2\\\", import: [\\\"@inaccessible\\\"], for: SECURITY)\\n{\\n  query: Query\\n}\\n\\ndirective @inaccessible on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION\\n\\ndirective @join__enumValue(graph: join__Graph!) repeatable on ENUM_VALUE\\n\\ndirective @join__field(graph: join__Graph, requires: join__FieldSet, provides: join__FieldSet, type: String, external: Boolean, override: String, usedOverridden: Boolean) repeatable on FIELD_DEFINITION | INPUT_FIELD_DEFINITION\\n\\ndirective @join__graph(name: String!, url: String!) on ENUM_VALUE\\n\\ndirective @join__implements(graph: join__Graph!, interface: String!) repeatable on OBJECT | INTERFACE\\n\\ndirective @join__type(graph: join__Graph!, key: join__FieldSet, extension: Boolean! = false, resolvable: Boolean! = true, isInterfaceObject: Boolean! = false) repeatable on OBJECT | INTERFACE | UNION | ENUM | INPUT_OBJECT | SCALAR\\n\\ndirective @join__unionMember(graph: join__Graph!, member: String!) repeatable on UNION\\n\\ndirective @link(url: String, as: String, for: link__Purpose, import: [link__Import]) repeatable on SCHEMA\\n\\ndirective @tag(name: String!) repeatable on FIELD_DEFINITION | OBJECT | INTERFACE | UNION | ARGUMENT_DEFINITION | SCALAR | ENUM | ENUM_VALUE | INPUT_OBJECT | INPUT_FIELD_DEFINITION | SCHEMA\\n\\nscalar join__FieldSet\\n\\nenum join__Graph {\\n  PANDAS @join__graph(name: \\\"pandas\\\", url: \\\"http://localhost:4003\\\")\\n  PRODUCTS @join__graph(name: \\\"products\\\", url: \\\"http://localhost:4002\\\")\\n  USERS @join__graph(name: \\\"users\\\", url: \\\"http://localhost:4001\\\")\\n}\\n\\nscalar link__Import \\n\\nenum link__Purpose {\\n  \\\"\\\"\\\"\\n  `SECURITY` features provide metadata necessary to securely resolve fields.\\n  \\\"\\\"\\\"\\n  SECURITY\\n\\n  \\\"\\\"\\\"\\n  `EXECUTION` features provide metadata necessary for operation execution.\\n  \\\"\\\"\\\"\\n  EXECUTION\\n}\\n\\ntype Panda\\n @join__type(graph: PANDAS)\\n{\\n  name: ID!\\n  favoriteFood: String @tag(name: \\\"nom-nom-nom\\\")\\n}\\n\\ntype Product implements ProductItf & SkuItf\\n  @join__implements(graph: PRODUCTS, interface: \\\"ProductItf\\\")\\n  @join__implements(graph: PRODUCTS, interface: \\\"SkuItf\\\")\\n  @join__type(graph: PRODUCTS, key: \\\"id\\\")\\n  @join__type(graph: PRODUCTS, key: \\\"sku package\\\")\\n @join__type(graph: PRODUCTS, key: \\\"sku variation { id }\\\")\\n{\\n  id: ID! @tag(name: \\\"hi-from-products\\\")\\n  sku: String\\n  package: String\\n  variation: ProductVariation\\n  dimensions: ProductDimension\\n  createdBy: User\\n  hidden: String\\n}\\n\\ntype ProductDimension\\n  @join__type(graph: PRODUCTS)\\n{\\n  size: String\\n  weight: Float\\n}\\n\\ninterface ProductItf implements SkuItf\\n  @join__implements(graph: PRODUCTS, interface: \\\"SkuItf\\\")\\n  @join__type(graph: PRODUCTS)\\n{\\n  id: ID!\\n  sku: String\\n  package: String\\n  variation: ProductVariation\\n  dimensions: ProductDimension\\n  createdBy: User\\n  hidden: String @inaccessible\\n}\\n\\ntype ProductVariation\\n  @join__type(graph: PRODUCTS)\\n{\\n  id: ID!\\n}\\n\\ntype Query\\n  @join__type(graph: PANDAS)\\n  @join__type(graph: PRODUCTS)\\n  @join__type(graph: USERS)\\n{\\n  allPandas: [Panda] @join__field(graph: PANDAS)\\n  panda(name: ID!): Panda @join__field(graph: PANDAS)\\n  allProducts: [ProductItf] @join__field(graph: PRODUCTS)\\n  product(id: ID!): ProductItf @join__field(graph: PRODUCTS)\\n}\\n\\nenum ShippingClass\\n  @join__type(graph: PRODUCTS)\\n{\\n  STANDARD @join__enumValue(graph: PRODUCTS)\\n  EXPRESS @join__enumValue(graph: PRODUCTS)\\n}\\n\\ninterface SkuItf\\n  @join__type(graph: PRODUCTS)\\n{\\n  sku: String\\n}\\n\\ntype User\\n  @join__type(graph: PRODUCTS, key: \\\"email\\\")\\n  @join__type(graph: USERS, key: \\\"email\\\")\\n{\\n  email: ID! @tag(name: \\\"test-from-users\\\")\\n  totalProductsCreated: Int\\n  name: String @join__field(graph: USERS)\\n}\",\"hints\":[{\"message\":\"[UNUSED_ENUM_TYPE]: Enum type \\\"ShippingClass\\\" is defined but unused. It will be included in the supergraph with all the values appearing in any subgraph (\\\"as if\\\" it was only used as an output type).\",\"code\":\"UNUSED_ENUM_TYPE\",\"nodes\":[],\"omittedNodesCount\":0}]}}".to_string()
}
