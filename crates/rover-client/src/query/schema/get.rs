#[derive(GraphQLQuery)]
#[graphql(
    query_path = "graphql/schema/get.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug",
    deprecated = "warn"
)]
struct GetQuery;

impl GetQuery {
       
}
