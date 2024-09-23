use std::net::SocketAddr;

use crate::{
    command::dev::netstat::normalize_loopback_urls, options::OptionalSubgraphOpts, RoverError,
    RoverErrorSuggestion, RoverResult,
};
use anyhow::anyhow;
use apollo_federation_types::config::{SchemaSource, SubgraphConfig, SupergraphConfig};
use reqwest::Url;

impl OptionalSubgraphOpts {
    pub fn get_single_subgraph_from_opts(
        &self,
        router_socket_addr: SocketAddr,
    ) -> RoverResult<SupergraphConfig> {
        let url = self.prompt_for_url()?;
        let normalized_user_urls = normalize_loopback_urls(&url);
        let normalized_supergraph_urls =
            normalize_loopback_urls(&Url::parse(&format!("http://{}", router_socket_addr))?);

        for normalized_user_url in &normalized_user_urls {
            for normalized_supergraph_url in &normalized_supergraph_urls {
                if normalized_supergraph_url == normalized_user_url {
                    let mut err = RoverError::new(anyhow!("The subgraph argument `--url {}` conflicts with the supergraph argument `--supergraph-port {}`", &url, normalized_supergraph_url.port().unwrap()));
                    err.set_suggestion(RoverErrorSuggestion::Adhoc("Set the `--supergraph-port` flag to a different port to start the local supergraph.".to_string()));
                    return Err(err);
                }
            }
        }

        let name = self.prompt_for_name()?;
        let schema = self.prompt_for_schema()?;
        let routing_url = Some(url.to_string());

        let schema = if let Some(schema) = schema {
            SchemaSource::File { file: schema }
        } else {
            SchemaSource::SubgraphIntrospection {
                subgraph_url: url,
                introspection_headers: None,
            }
        };
        let subgraph_config = SubgraphConfig {
            routing_url,
            schema,
        };
        Ok(SupergraphConfig::new(
            [(name, subgraph_config)].into_iter().collect(),
            None,
        ))
    }
}
