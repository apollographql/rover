use std::{fmt, future::Future, pin::Pin};

use graphql_client::GraphQLQuery;
use houston::CredentialOrigin;
use rover_graphql::{GraphQLRequest, GraphQLServiceError};
use tower::Service;

use super::types::{InitMembershipsInput, InitMembershipsResponse, Organization, QueryVariables};
use crate::RoverClientError;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/init/memberships/init_memberships_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "Eq, PartialEq, Debug, Serialize, Deserialize",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. init_memberships_query
pub struct InitMembershipsQuery;

impl fmt::Debug for init_memberships_query::Variables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Variables").finish()
    }
}

impl PartialEq for init_memberships_query::Variables {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

#[derive(thiserror::Error, Debug)]
pub enum MembershipsError {
    #[error("Invalid key")]
    InvalidKey,
    #[error(transparent)]
    GraphQL(#[from] GraphQLServiceError<<InitMembershipsQuery as GraphQLQuery>::ResponseData>),
}

impl From<MembershipsError> for RoverClientError {
    fn from(value: MembershipsError) -> Self {
        match value {
            MembershipsError::InvalidKey => RoverClientError::InvalidKey,
            MembershipsError::GraphQL(err) => err.into(),
        }
    }
}

pub struct MembershipsRequest {
    input: InitMembershipsInput,
    credential_origin: CredentialOrigin,
}

impl MembershipsRequest {
    pub const fn new(credential_origin: CredentialOrigin) -> MembershipsRequest {
        MembershipsRequest {
            input: InitMembershipsInput {},
            credential_origin,
        }
    }
}

pub struct Memberships<S> {
    inner: S,
}

impl<S> Memberships<S> {
    pub const fn new(inner: S) -> Memberships<S> {
        Memberships { inner }
    }
}

impl<S, Fut> Service<MembershipsRequest> for Memberships<S>
where
    S: Service<
            GraphQLRequest<InitMembershipsQuery>,
            Response = init_memberships_query::ResponseData,
            Error = GraphQLServiceError<init_memberships_query::ResponseData>,
            Future = Fut,
        > + Clone
        + Send
        + 'static,
    Fut: Future<Output = Result<S::Response, S::Error>> + Send,
{
    type Response = InitMembershipsResponse;
    type Error = MembershipsError;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        tower::Service::<GraphQLRequest<InitMembershipsQuery>>::poll_ready(&mut self.inner, cx)
            .map_err(MembershipsError::from)
    }

    fn call(&mut self, req: MembershipsRequest) -> Self::Future {
        let cloned = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, cloned);
        let fut = async move {
            inner
                .call(GraphQLRequest::<InitMembershipsQuery>::new(
                    QueryVariables::from(req.input),
                ))
                .await
                .map_err(MembershipsError::from)
                .and_then(|response_data: init_memberships_query::ResponseData| {
                    if let Some(me) = response_data.me {
                        let memberships = match me.on {
                            init_memberships_query::InitMembershipsQueryMeOn::User(m) => Some(
                                m.memberships
                                    .iter()
                                    .map(|o| Organization {
                                        id: o.account.id.to_string(),
                                        name: o.account.name.to_string(),
                                    })
                                    .collect::<Vec<_>>(),
                            ),
                            _ => None,
                        };

                        Ok(InitMembershipsResponse {
                            id: me.id,
                            memberships: memberships.unwrap_or_default(),
                            credential_origin: req.credential_origin,
                        })
                    } else {
                        Err(MembershipsError::InvalidKey)
                    }
                })
        };
        Box::pin(fut)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use speculoos::prelude::*;
    use tokio::task;
    use tower::{ServiceBuilder, ServiceExt};
    use tower_test::mock;

    use super::*;

    #[tokio::test]
    async fn get_memberships_from_response_data_works_for_users() {
        let (service, mut handle) = mock::spawn::<
            GraphQLRequest<InitMembershipsQuery>,
            init_memberships_query::ResponseData,
        >();

        let inner = ServiceBuilder::new()
            .map_err(GraphQLServiceError::UpstreamService)
            .service(service.into_inner());
        let mut memberships = Memberships::new(inner);
        let memberships = memberships.ready().await.unwrap();

        let response = memberships.call(MembershipsRequest::new(CredentialOrigin::EnvVar));

        let json_response = json!({
            "me": {
              "__typename": "User",
              "memberships": [{ "account": { "id": "thisisfake", "name": "This is Fake"}}],
              "id": "gh.nobodydefinitelyhasthisusernamelol",
            }
        });

        let response_data: init_memberships_query::ResponseData =
            serde_json::from_value(json_response).unwrap();

        let resp_task = task::spawn(async move {
            let (req, send_response) = handle.next_request().await.unwrap();
            assert_that!(req)
                .is_equal_to(GraphQLRequest::new(init_memberships_query::Variables {}));
            send_response.send_response(response_data);
        });

        let output = response.await;

        let expected_identity = InitMembershipsResponse {
            id: "gh.nobodydefinitelyhasthisusernamelol".to_string(),
            memberships: vec![Organization {
                id: "thisisfake".to_string(),
                name: "This is Fake".to_string(),
            }],
            credential_origin: CredentialOrigin::EnvVar,
        };
        assert_that!(output).is_ok().is_equal_to(expected_identity);
        resp_task.await.unwrap()
    }
}
