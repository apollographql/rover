use crate::blocking::StudioClient;
use crate::RoverClientError;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/metadata/frontend.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. frontend_url_query
pub struct FrontendUrlQuery;

/// Fetch the url for apollo studio's frontend from the api
/// this should allow staging/local/envs to also use the `graph open` command
pub fn run(
    variables: frontend_url_query::Variables,
    client: &StudioClient,
) -> Result<String, RoverClientError> {
    let response_data = client.post::<FrontendUrlQuery>(variables)?;
    Ok(response_data.frontend_url_root)
    // get_identity_from_response_data(response_data, client.credential.origin.clone())
}

// fn get_identity_from_response_data(
//     response_data: frontend_url_query::ResponseData,
//     credential_origin: CredentialOrigin,
// ) -> Result<RegistryIdentity, RoverClientError> {
//     if let Some(me) = response_data.me {
//         // I believe for the purposes of the CLI, we only care about users and
//         // graphs as api key actors, since that's all we _should_ get.
//         // I think it's safe to only include those two kinds of actors in the enum
//         // more here: https://studio-staging.apollographql.com/graph/engine/schema/reference/enums/ActorType?variant=prod

//         let key_actor_type = match me.as_actor.type_ {
//             frontend_url_query::ActorType::GRAPH => Actor::GRAPH,
//             frontend_url_query::ActorType::USER => Actor::USER,
//             _ => Actor::OTHER,
//         };

//         let graph_title = match me.on {
//             frontend_url_query::FrontendUrlQueryMeOn::Service(s) => Some(s.title),
//             _ => None,
//         };

//         Ok(RegistryIdentity {
//             id: me.id,
//             graph_title,
//             key_actor_type,
//             credential_origin,
//         })
//     } else {
//         Err(RoverClientError::InvalidKey)
//     }
// }

