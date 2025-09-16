use super::runner::list_graphs_for_organization;
use serde::Serialize;

pub(crate) type QueryVariables = list_graphs_for_organization::Variables;
pub(crate) type QueryResponseData = list_graphs_for_organization::ResponseData;

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct ListGraphsInput {
    pub organization_id: String,
}

impl From<ListGraphsInput> for QueryVariables {
    fn from(input: ListGraphsInput) -> Self {
        Self {
            organization_id: input.organization_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct ListGraphsResponse {
    pub organization: OrganizationWithGraphs,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct OrganizationWithGraphs {
    pub id: String,
    pub name: String,
    pub graphs: Vec<GraphInfo>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct GraphInfo {
    pub id: String,
    pub name: String,
    pub variants: Vec<VariantInfo>,
}

#[derive(Debug, Clone, Serialize, Eq, PartialEq)]
pub struct VariantInfo {
    pub name: String,
}
