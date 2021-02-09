use crate::blocking::StudioClient;
use crate::RoverClientError;
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
    pub name: String,
    pub id: String,
    pub key_actor_type: Actor,
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
    get_identity_from_response_data(response_data)
}

fn get_identity_from_response_data(
    response_data: who_am_i_query::ResponseData,
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
        Ok(RegistryIdentity {
            id: me.id,
            name: me.name,
            key_actor_type,
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
              "name": "Yaboi",
              "id": "gh.nobodydefinitelyhasthisusernamelol",
              "asActor": {
                "type": "USER"
              },
            }
        });
        let data: who_am_i_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_identity_from_response_data(data);

        let expected_identity = RegistryIdentity {
            name: "Yaboi".to_string(),
            id: "gh.nobodydefinitelyhasthisusernamelol".to_string(),
            key_actor_type: Actor::USER,
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_identity);
    }

    #[test]
    fn get_identity_from_response_data_works_for_services() {
        let json_response = json!({
            "me": {
              "__typename": "Service",
              "name": "big-ol-graph",
              "id": "big-ol-graph-key-lolol",
              "asActor": {
                "type": "GRAPH"
              },
            }
        });
        let data: who_am_i_query::ResponseData = serde_json::from_value(json_response).unwrap();
        let output = get_identity_from_response_data(data);

        let expected_identity = RegistryIdentity {
            name: "big-ol-graph".to_string(),
            id: "big-ol-graph-key-lolol".to_string(),
            key_actor_type: Actor::GRAPH,
        };
        assert!(output.is_ok());
        assert_eq!(output.unwrap(), expected_identity);
    }
}
