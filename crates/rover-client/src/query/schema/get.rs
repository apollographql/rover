use graphql_client::*;

// I'm not sure where this should live long-term
/// this is because of the custom GraphQLDocument scalar in the schema
type GraphQLDocument = String;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/schema/get.graphql",
    schema_path = "schema.graphql",
    response_derives = "PartialEq, Debug",
    deprecated = "warn"
)]
/// TODO: doc
pub struct GetSchemaQuery;

/// TODO: doc
pub fn execute(_variables: get_schema_query::Variables) -> Result<(), ()>{
    Ok(())
}       


// pub fn perform_my_query(variables: get_schema_query::Variables) -> () {
    //Result<(), Box<dyn Error>> {
    // ()
// };

//     // this is the important line
//     let request_body = UnionQuery::build_query(variables);

//     let client = reqwest::Client::new();
//     let mut res = client.post("/graphql").json(&request_body).send()?;
//     let response_body: Response<union_query::ResponseData> = res.json()?;
//     println!("{:#?}", response_body);
//     Ok(())
// }