use anyhow::anyhow;
use reqwest::blocking::Client;
use rover_std::Style;

use crate::command::dev::protocol::{SubgraphSdl, SubgraphUrl};
use crate::command::graph::Introspect as GraphIntrospect;
use crate::command::subgraph::Introspect as SubgraphIntrospect;
use crate::options::IntrospectOpts;
use crate::{RoverError, RoverErrorSuggestion, RoverResult};

#[derive(Clone, Debug)]
pub struct UnknownIntrospectRunner {
    endpoint: SubgraphUrl,
    client: Client,
}

impl UnknownIntrospectRunner {
    pub fn new(endpoint: SubgraphUrl, client: Client) -> Self {
        Self { endpoint, client }
    }

    pub fn run(&self) -> RoverResult<(SubgraphSdl, IntrospectRunnerKind)> {
        let subgraph_runner = SubgraphIntrospectRunner {
            endpoint: self.endpoint.clone(),
            client: self.client.clone(),
        };

        let graph_runner = GraphIntrospectRunner {
            endpoint: self.endpoint.clone(),
            client: self.client.clone(),
        };

        // we _could_ run these in parallel
        // but we could run into race conditions where
        // the regular introspection query runs a bit after
        // the federated introspection query
        // in which case we may incorrectly assume
        // they do not support federated introspection
        // so, run the graph query first and _then_ the subgraph query
        let graph_result = graph_runner.run();
        let subgraph_result = subgraph_runner.run();

        match (subgraph_result, graph_result) {
            (Ok(s), _) => {
                tracing::info!("fetching federated SDL succeeded");
                Ok((s, IntrospectRunnerKind::Subgraph(subgraph_runner)))
            }
            (Err(_), Ok(s)) => {
                let warn_prefix = Style::WarningPrefix.paint("WARN:");
                eprintln!("{} could not fetch federated SDL, using introspection schema without directives. you should convert this monograph to a federated subgraph. see https://www.apollographql.com/docs/federation/subgraphs/ for more information.", warn_prefix);
                Ok((s, IntrospectRunnerKind::Graph(graph_runner)))
            }
            (Err(se), Err(ge)) => {
                let message = anyhow!(
                    "could not run `rover graph introspect {0}` or `rover subgraph introspect {0}`",
                    &self.endpoint
                );
                let mut err = RoverError::new(message);
                let (ge, se) = (ge.to_string(), se.to_string());
                if ge == se {
                    err.set_suggestion(RoverErrorSuggestion::Adhoc(ge))
                } else {
                    err.set_suggestion(RoverErrorSuggestion::Adhoc(format!("`rover subgraph introspect {0}` failed with:\n{1}\n`rover graph introspect {0}` failed with:\n{2}", &self.endpoint, &se, &ge)));
                };
                Err(err)
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum IntrospectRunnerKind {
    Unknown(UnknownIntrospectRunner),
    Subgraph(SubgraphIntrospectRunner),
    Graph(GraphIntrospectRunner),
}

impl IntrospectRunnerKind {
    pub fn endpoint(&self) -> SubgraphUrl {
        match &self {
            Self::Unknown(u) => u.endpoint.clone(),
            Self::Subgraph(s) => s.endpoint.clone(),
            Self::Graph(g) => g.endpoint.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SubgraphIntrospectRunner {
    endpoint: SubgraphUrl,
    client: Client,
}

impl SubgraphIntrospectRunner {
    pub fn run(&self) -> RoverResult<String> {
        tracing::debug!(
            "running `rover subgraph introspect --endpoint {}`",
            &self.endpoint
        );
        SubgraphIntrospect {
            opts: IntrospectOpts {
                endpoint: self.endpoint.clone(),
                headers: None,
                watch: false,
            },
        }
        .exec(&self.client, false)
    }
}

#[derive(Debug, Clone)]
pub struct GraphIntrospectRunner {
    endpoint: SubgraphUrl,
    client: Client,
}

impl GraphIntrospectRunner {
    pub fn run(&self) -> RoverResult<String> {
        tracing::debug!(
            "running `rover graph introspect --endpoint {}`",
            &self.endpoint
        );
        GraphIntrospect {
            opts: IntrospectOpts {
                endpoint: self.endpoint.clone(),
                headers: None,
                watch: false,
            },
        }
        .exec(&self.client, false)
    }
}
