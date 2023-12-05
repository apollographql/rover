use crate::shared::GraphRef;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseFetchInput {
    pub graph_ref: GraphRef,
}

type QueryVariables = crate::operations::license::fetch::runner::license_fetch_query::Variables;
impl From<LicenseFetchInput> for QueryVariables {
    fn from(input: LicenseFetchInput) -> Self {
        Self {
            graph_id: input.graph_ref.name,
        }
    }
}
