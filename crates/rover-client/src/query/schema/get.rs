use graphql_client::*;
use crate::blocking::Client;
use crate::RoverClientError;

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
/// This struct is used to generate the module containing `Variables` and 
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. get_schema_query
pub struct GetSchemaQuery;

// TODO: should we also add a struct for api config (key & endpoint?)
/// The main function to be used from this module. This function "executes" the
/// `get` functionality from apollo studio
pub fn execute(variables: get_schema_query::Variables, api_key: String) 
    -> Result<Option<get_schema_query::ResponseData>, RoverClientError<'static>>{
        let client = Client::new(api_key, None); // TODO: change from default uri
        // TODO: why not get_schema_query here?
        // needs the Struct, not module?
        client.post::<GetSchemaQuery>(variables)
    }