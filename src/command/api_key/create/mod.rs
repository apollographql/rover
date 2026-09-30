mod output;

use std::{collections::HashMap, fs::canonicalize, io::IsTerminal, path::PathBuf};

use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::operations::api_key::{
    create::{ApiKeyResourceInput, CreateKeyInput, SubgraphIdentifierInput, run},
    pair_create::{CreatePair, CreatePairInput, service::CREATE_PAIR_ATTEMPT_TIMEOUT},
};
use serde::Serialize;
use tower::{Service, ServiceExt};

use crate::{
    RoverError, RoverOutput, RoverResult,
    command::api_key::{
        ApiKeyType, OrganizationOpt, create::output::CreateClientCredentialsOutput,
    },
    options::ProfileOpt,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
};

#[derive(Debug, Serialize, Parser)]
pub(crate) struct Create {
    #[clap(flatten)]
    organization_opt: OrganizationOpt,
    #[clap(name = "TYPE", value_enum, help = "The type of the API key")]
    key_type: ApiKeyType,
    #[clap(help = "The name of the key to be created")]
    name: String,
    #[clap(long)]
    subgraph_config: Option<PathBuf>,

    // FR2: required (validated in `validate_arguments`, not here - clap can't express "required
    // only for one enum value of another argument"), repeatable.
    #[clap(
        long = "graph-id",
        help = "A graph the pair may act on (client-credentials only)"
    )]
    graph_ids: Vec<String>,

    // FR3: optional; omitting it uses the Platform API's own default lifetime.
    #[clap(
        long,
        value_parser = clap::value_parser!(i64).range(0..),
        help = "How many days the pair's secret stays valid (client-credentials only)"
    )]
    secret_lifetime_days: Option<i64>,
}

impl Create {
    pub(crate) async fn run(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        self.validate_arguments()?;

        if let ApiKeyType::ClientCredentials = self.key_type {
            return self.create_client_credentials(client_config, profile).await;
        }

        let client = client_config.get_authenticated_client(profile)?;
        let resources = match self.key_type {
            ApiKeyType::Operator => None,
            ApiKeyType::Subgraph => {
                let file_descriptor = self
                    .subgraph_config
                    .clone()
                    .map(canonicalize)
                    .transpose()?
                    .map(Utf8PathBuf::from_path_buf)
                    .transpose()
                    .map_err(|p| anyhow::anyhow!("Unable to convert {:?} to Utf8PathBuf", p))?
                    .map(FileDescriptorType::File)
                    .unwrap_or_else(|| FileDescriptorType::Stdin);
                let mut stdin = std::io::stdin();
                if let FileDescriptorType::Stdin = file_descriptor
                    && stdin.is_terminal()
                {
                    return Err(RoverError::new(
                        anyhow::anyhow!("Expected subgraph config from stdin, received none")
                    ).with_suggestion(
                        crate::RoverErrorSuggestion::Adhoc("Pipe supergraph config to stdin or provide a file path via the --subgraph-config flag".to_string()))
                    );
                }
                let content =
                    file_descriptor.read_file_descriptor("subgraph config", &mut stdin)?;
                let config: SubgraphKeyConfig = serde_yaml::from_str(&content)?;
                let mut subgraphs_input = Vec::new();
                for (graph_id, variants) in config.iter() {
                    for (variant_name, subgraphs) in variants.iter() {
                        for subgraph_name in subgraphs {
                            subgraphs_input.push(SubgraphIdentifierInput {
                                graph_id: graph_id.clone(),
                                variant_name: variant_name.clone(),
                                subgraph_name: subgraph_name.to_string(),
                            })
                        }
                    }
                }
                let resources = ApiKeyResourceInput {
                    subgraphs: Some(subgraphs_input),
                    gateways: None,
                    variants: None,
                };
                Some(resources)
            }
            ApiKeyType::ClientCredentials => unreachable!("handled above"),
        };
        let resp = run(
            CreateKeyInput {
                organization_id: self.organization_opt.organization_id.clone(),
                name: self.name.clone(),
                key_type: self.key_type.into_query_enum(),
                resources,
            },
            &client,
        )
        .await?;
        Ok(RoverOutput::CreateKeyResponse {
            api_key: resp.token,
            key_type: self.key_type.to_string(),
            id: resp.key_id,
            name: resp.key_name,
        })
    }

    /// FR2-FR4: `--graph-id`/`--secret-lifetime-days` only make sense for a pair, and
    /// `--subgraph-config` only for a `subgraph` key - no clap-declarative way exists to gate an
    /// argument on another argument's specific enum value, so this runs first, before any
    /// request, and rejects every other combination as a usage error.
    fn validate_arguments(&self) -> RoverResult<()> {
        match self.key_type {
            ApiKeyType::ClientCredentials => {
                if self.graph_ids.is_empty() {
                    return Err(usage_error(
                        "--graph-id is required when creating a client-credentials pair",
                    ));
                }
                if self.subgraph_config.is_some() {
                    return Err(usage_error(
                        "--subgraph-config isn't accepted with `client-credentials`",
                    ));
                }
            }
            ApiKeyType::Operator | ApiKeyType::Subgraph => {
                if !self.graph_ids.is_empty() {
                    return Err(usage_error(
                        "--graph-id is only accepted with `client-credentials`",
                    ));
                }
                if self.secret_lifetime_days.is_some() {
                    return Err(usage_error(
                        "--secret-lifetime-days is only accepted with `client-credentials`",
                    ));
                }
            }
        }
        Ok(())
    }

    async fn create_client_credentials(
        &self,
        client_config: StudioClientConfig,
        profile: &ProfileOpt,
    ) -> RoverResult<RoverOutput> {
        let client = client_config.get_authenticated_client(profile)?;
        let service = client.studio_graphql_service_with_timeout(CREATE_PAIR_ATTEMPT_TIMEOUT)?;
        let mut create_pair = CreatePair::new(service);
        let create_pair = create_pair.ready().await?;
        let pair = create_pair
            .call(
                CreatePairInput::builder()
                    .organization_id(self.organization_opt.organization_id.clone())
                    .name(self.name.clone())
                    .graph_ids(self.graph_ids.clone())
                    .maybe_secret_lifetime_days(self.secret_lifetime_days)
                    .build(),
            )
            .await?;

        Ok(RoverOutput::CliOutput(Box::new(
            CreateClientCredentialsOutput { pair },
        )))
    }
}

fn usage_error(message: &str) -> RoverError {
    RoverError::new(anyhow::anyhow!("{message}"))
}

pub type GraphId = String;
pub type VariantName = String;
pub type SubgraphName = String;
pub type SubgraphIdentifier = HashMap<VariantName, Vec<SubgraphName>>;
pub type SubgraphKeyConfig = HashMap<GraphId, SubgraphIdentifier>;

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    fn organization_opt() -> OrganizationOpt {
        OrganizationOpt {
            organization_id: "acme".to_string(),
        }
    }

    fn client_credentials_create() -> Create {
        Create {
            organization_opt: organization_opt(),
            key_type: ApiKeyType::ClientCredentials,
            name: "ci-deploy".to_string(),
            subgraph_config: None,
            graph_ids: vec!["inventory".to_string()],
            secret_lifetime_days: None,
        }
    }

    fn operator_create() -> Create {
        Create {
            organization_opt: organization_opt(),
            key_type: ApiKeyType::Operator,
            name: "router-prod".to_string(),
            subgraph_config: None,
            graph_ids: Vec::new(),
            secret_lifetime_days: None,
        }
    }

    #[test]
    fn a_valid_client_credentials_create_passes_validation() {
        assert_that!(client_credentials_create().validate_arguments()).is_ok();
    }

    #[test]
    fn a_valid_operator_create_passes_validation() {
        assert_that!(operator_create().validate_arguments()).is_ok();
    }

    #[test]
    fn client_credentials_without_a_graph_id_is_rejected() {
        let create = Create {
            graph_ids: Vec::new(),
            ..client_credentials_create()
        };

        let error = create
            .validate_arguments()
            .expect_err("expected a missing --graph-id to be rejected");

        assert_that!(error.to_string()).contains("--graph-id is required");
    }

    #[test]
    fn client_credentials_with_subgraph_config_is_rejected() {
        let create = Create {
            subgraph_config: Some(PathBuf::from("subgraphs.yaml")),
            ..client_credentials_create()
        };

        let error = create
            .validate_arguments()
            .expect_err("expected --subgraph-config to be rejected for client-credentials");

        assert_that!(error.to_string()).contains("--subgraph-config isn't accepted");
    }

    #[test]
    fn a_non_pair_type_with_a_graph_id_is_rejected() {
        let create = Create {
            graph_ids: vec!["inventory".to_string()],
            ..operator_create()
        };

        let error = create
            .validate_arguments()
            .expect_err("expected --graph-id to be rejected for a non-pair type");

        assert_that!(error.to_string()).contains("--graph-id is only accepted");
    }

    #[test]
    fn a_non_pair_type_with_a_secret_lifetime_is_rejected() {
        let create = Create {
            secret_lifetime_days: Some(30),
            ..operator_create()
        };

        let error = create
            .validate_arguments()
            .expect_err("expected --secret-lifetime-days to be rejected for a non-pair type");

        assert_that!(error.to_string()).contains("--secret-lifetime-days is only accepted");
    }

    #[test]
    fn client_credentials_is_a_recognized_clap_type_value() {
        let create = Create::try_parse_from([
            "api-key create",
            "acme",
            "client-credentials",
            "ci-deploy",
            "--graph-id",
            "inventory",
            "--graph-id",
            "checkout",
        ])
        .expect("expected `client-credentials` to parse as a valid TYPE");

        assert_that!(matches!(create.key_type, ApiKeyType::ClientCredentials)).is_true();
        assert_that!(create.graph_ids)
            .is_equal_to(vec!["inventory".to_string(), "checkout".to_string()]);
    }

    #[test]
    fn a_negative_secret_lifetime_is_rejected_by_clap() {
        let error = Create::try_parse_from([
            "api-key create",
            "acme",
            "client-credentials",
            "ci-deploy",
            "--graph-id",
            "inventory",
            "--secret-lifetime-days=-1",
        ])
        .expect_err("expected a negative secret lifetime to be rejected");

        assert_that!(error.to_string()).contains("secret-lifetime-days");
    }
}
