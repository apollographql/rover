#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LicenseFetchInput {
    pub graph_id: String,
}

type QueryVariables = crate::operations::license::fetch::runner::license_fetch_query::Variables;
impl From<LicenseFetchInput> for QueryVariables {
    fn from(input: LicenseFetchInput) -> Self {
        Self {
            graph_id: input.graph_id,
        }
    }
}
