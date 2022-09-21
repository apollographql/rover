use async_graphql::{InputObject, SimpleObject, ID};

#[derive(SimpleObject)]
/// An example type which can be resolved either as an entity or via direct query
pub(crate) struct Thing {
    pub(crate) id: ID,
    pub(crate) name: Option<String>,
}

#[derive(InputObject)]
pub(crate) struct CreateThing {
    pub(crate) id: ID,
    pub(crate) name: Option<String>,
}
