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
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and 
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. get_schema_query
pub struct GetSchemaQuery;

/// The main function to be used from this module. This function fetches a 
/// schema from apollo studio and returns it in either json or sdl format
pub fn run(variables: get_schema_query::Variables, client: Client) 
    -> Result<String, RoverClientError>{
        let res = client.post::<GetSchemaQuery>(variables);
        // TODO (future) handle sdl printing

        // if asking for a json response, try serializing the schema
        // first unwrap the Result<Option<>>
        let data = res.expect("Error fetching schema");
        let data = data.expect("No data in response when trying to fetch schema");
        
        // now that we have the unwrapped response data, we can get the schema
        dbg!(serde_json::to_string(&data.service.unwrap().schema));
        // dbg!(data.json());

        Ok("schema {}".to_string())
    }