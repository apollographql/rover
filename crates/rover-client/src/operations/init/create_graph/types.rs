use super::runner::create_graph_mutation;

pub(crate) type ResponseData = create_graph_mutation::ResponseData;
pub(crate) type MutationVariables = create_graph_mutation::Variables;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CreateGraphInput {
    pub hidden_from_uninvited_non_admin: bool,
    pub create_graph_id: String,
    pub title: String,
    pub organization_id: String,
}

impl From<CreateGraphInput> for MutationVariables {
    fn from(input: CreateGraphInput) -> Self {
        Self {
            hidden_from_uninvited_non_admin: input.hidden_from_uninvited_non_admin,
            create_graph_id: input.create_graph_id,
            title: input.title,
            organization_id: input.organization_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateGraphResponse {
    pub id: String,
}

impl From<create_graph_mutation::CreateGraphMutationOrganizationCreateGraph>
    for CreateGraphResponse
{
    fn from(graph: create_graph_mutation::CreateGraphMutationOrganizationCreateGraph) -> Self {
        match graph {
            create_graph_mutation::CreateGraphMutationOrganizationCreateGraph::Graph(graph) => Self { id: graph.id },
            create_graph_mutation::CreateGraphMutationOrganizationCreateGraph::GraphCreationError(_error) => {
                Self { id: String::new() }
            }
        }
    }
}
