use crate::blocking::StudioClient;
use crate::RoverClientError;
use houston::CredentialOrigin;

use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/query/config/whoami.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. who_am_i_query
pub struct WhoAmIQuery;

#[derive(Debug, PartialEq)]
pub struct RegistryIdentity {
    pub id: String,
    pub graph_title: Option<String>,
    pub key_actor_type: Actor,
    pub credential_origin: CredentialOrigin,
}

#[derive(Debug, PartialEq)]
pub enum Actor {
    GRAPH,
    USER,
    OTHER,
}

/// Get info from the registry about an API key, i.e. the name/id of the
/// user/graph and what kind of key it is (GRAPH/USER/Other)
pub fn run(
    variables: who_am_i_query::Variables,
    client: &StudioClient,
) -> Result<RegistryIdentity, RoverClientError> {
    let response_data = client.post::<WhoAmIQuery>(variables)?;
    get_identity_from_response_data(response_data, client.credential.origin.clone())
}

fn get_identity_from_response_data(
    response_data: who_am_i_query::ResponseData,
    credential_origin: CredentialOrigin,
) -> Result<RegistryIdentity, RoverClientError> {
    if let Some(me) = response_data.me {
        // I believe for the purposes of the CLI, we only care about users and
        // graphs as api key actors, since that's all we _should_ get.
        // I think it's safe to only include those two kinds of actors in the enum
        // more here: https://studio-staging.apollographql.com/graph/engine/schema/reference/enums/ActorType?variant=prod

        let key_actor_type = match me.as_actor.type_ {
            who_am_i_query::ActorType::GRAPH => Actor::GRAPH,
            who_am_i_query::ActorType::USER => Actor::USER,
            _ => Actor::OTHER,
        };

        let graph_title = match me.on {
            who_am_i_query::WhoAmIQueryMeOn::Service(s) => Some(s.title),
            _ => None,
        };

        Ok(RegistryIdentity {
            id: me.id,
            graph_title,
            key_actor_type,
            credential_origin,
        })
    } else {
        Err(RoverClientError::InvalidKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn get_identity_from_response_data_works_for_users() {
        let json_response = json!({
            "me": {
              "__typename": "User",
              "title": "SearchForTunaService",
              "id": "gh.nobodydefinitelyhasthisusernamelol",
              "asActor": {
                "type": "USER"
              },
            }
        });
        let data: who_am_i_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_identity_from_response_data(data, CredentialOrigin::EnvVar);

        let expected_identity = RegistryIdentity {
            id: "gh.nobodydefinitelyhasthisusernamelol".to_string(),
            graph_title: None,
            key_actor_type: Actor::USER,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_identity);
    }

    #[test]
    fn get_identity_from_response_data_works_for_services() {
        let json_response = json!({
            "me": {
              "__typename": "Service",
              "title": "GraphKeyService",
              "id": "big-ol-graph-key-lolol",
              "asActor": {
                "type": "GRAPH"
              },
            }
        });
        let data: who_am_i_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_identity_from_response_data(data, CredentialOrigin::EnvVar);

        let expected_identity = RegistryIdentity {
            id: "big-ol-graph-key-lolol".to_string(),
            graph_title: Some("GraphKeyService".to_string()),
            key_actor_type: Actor::GRAPH,
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_identity);
    }
}
