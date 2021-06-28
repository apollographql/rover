mod mutation_runner;

pub(crate) mod types;

pub use mutation_runner::run;
pub use types::{SubgraphDeleteInput, SubgraphDeleteResponse};
