use serde::Serialize;

/// The filter to apply to define a contract.
#[derive(Clone, Eq, PartialEq, Debug, Serialize)]
pub struct ContractFilterConfig {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub hide_unreachable_types: bool,
}
