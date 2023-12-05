use crate::blocking::StudioClient;
use crate::RoverClientError;

use crate::operations::license::fetch::types::LicenseFetchInput;
use crate::shared::GraphRef;
use graphql_client::*;

#[derive(GraphQLQuery)]
// The paths are relative to the directory where your `Cargo.toml` is located.
// Both json and the GraphQL schema language are supported as sources for the schema
#[graphql(
    query_path = "src/operations/license/fetch/fetch_query.graphql",
    schema_path = ".schema/schema.graphql",
    response_derives = "PartialEq, Eq, Debug, Serialize, Deserialize, Clone",
    deprecated = "warn"
)]
/// This struct is used to generate the module containing `Variables` and
/// `ResponseData` structs.
/// Snake case of this name is the mod name. i.e. license_fetch_query
pub(crate) struct LicenseFetchQuery;

/// The main function to be used from this module. This function fetches an offline license if permitted to do so.
pub fn run(input: LicenseFetchInput, client: &StudioClient) -> Result<String, RoverClientError> {
    let graph_ref = input.graph_ref.clone();
    let response_data = client.post::<LicenseFetchQuery>(input.into())?;
    let license = get_license_response_from_data(response_data, &graph_ref)?;
    Ok(license)
}

fn get_license_response_from_data(
    data: license_fetch_query::ResponseData,
    graph_ref: &GraphRef,
) -> Result<String, RoverClientError> {
    let graph = data.graph.ok_or(RoverClientError::GraphNotFound {
        graph_ref: graph_ref.clone(),
    })?;
    // Yes, account is optional in the platform api schema.
    let account = graph
        .account
        .ok_or(RoverClientError::OrganizationNotFound {
            graph_ref: graph_ref.clone(),
        })?;
    let license = account
        .offline_license
        .ok_or(RoverClientError::OfflineLicenseNotEnabled)?;

    Ok(license.jwt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::license::fetch::runner::license_fetch_query::{
        LicenseFetchQueryGraph, LicenseFetchQueryGraphAccount,
        LicenseFetchQueryGraphAccountOfflineLicense, ResponseData,
    };
    use crate::shared::GraphRef;

    #[test]
    fn gets_license_when_data_is_valid() {
        let data = ResponseData {
            graph: Some(LicenseFetchQueryGraph {
                account: Some(LicenseFetchQueryGraphAccount {
                    offline_license: Some(LicenseFetchQueryGraphAccountOfflineLicense {
                        jwt: "valid_license".to_string(),
                    }),
                }),
            }),
        };
        let graph_ref = GraphRef::new("graph".to_string(), Some("variant".to_string())).unwrap();

        let result = get_license_response_from_data(data, &graph_ref);

        assert_eq!(result.unwrap(), "valid_license");
    }

    #[test]
    fn returns_error_when_graph_is_missing() {
        let data = ResponseData { graph: None };
        let graph_ref = GraphRef::new("graph".to_string(), Some("variant".to_string())).unwrap();
        let result = get_license_response_from_data(data, &graph_ref);

        assert!(matches!(
            result.unwrap_err(),
            RoverClientError::GraphNotFound { .. }
        ));
    }

    #[test]
    fn returns_error_when_account_is_missing() {
        let data = ResponseData {
            graph: Some(LicenseFetchQueryGraph { account: None }),
        };
        let graph_ref = GraphRef::new("graph".to_string(), Some("variant".to_string())).unwrap();
        let result = get_license_response_from_data(data, &graph_ref);

        assert!(matches!(
            result.unwrap_err(),
            RoverClientError::OrganizationNotFound { .. }
        ));
    }

    #[test]
    fn returns_error_when_offline_license_is_missing() {
        let data = ResponseData {
            graph: Some(LicenseFetchQueryGraph {
                account: Some(LicenseFetchQueryGraphAccount {
                    offline_license: None,
                }),
            }),
        };
        let graph_ref = GraphRef::new("graph".to_string(), Some("variant".to_string())).unwrap();
        let result = get_license_response_from_data(data, &graph_ref);

        assert!(matches!(
            result.unwrap_err(),
            RoverClientError::OfflineLicenseNotEnabled
        ));
    }
}
