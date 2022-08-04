use std::sync::mpsc::{sync_channel, SyncSender};

use reqwest::blocking::Client;
use saucer::{anyhow, ParallelSaucer, Saucer};

use crate::command::graph::Introspect as GraphIntrospect;
use crate::command::subgraph::Introspect as SubgraphIntrospect;
use crate::options::IntrospectOpts;
use crate::Result;

#[derive(Clone, Debug)]
pub struct UnknownIntrospectRunner {
    endpoint: reqwest::Url,
    client: Client,
}

impl UnknownIntrospectRunner {
    pub fn new(endpoint: reqwest::Url, client: Client) -> Self {
        Self { endpoint, client }
    }

    pub fn run(&self) -> Result<(String, IntrospectRunnerKind)> {
        let (subgraph_sender, subgraph_receiver) = sync_channel(1);
        let subgraph_runner = SubgraphIntrospectRunner {
            sender: subgraph_sender,
            endpoint: self.endpoint.clone(),
            client: self.client.clone(),
        };

        let (graph_sender, graph_receiver) = sync_channel(1);
        let graph_runner = GraphIntrospectRunner {
            sender: graph_sender,
            endpoint: self.endpoint.clone(),
            client: self.client.clone(),
        };

        // stage 1 of 1
        self.introspect(subgraph_runner.clone(), graph_runner.clone(), 1, 1)
            .beam()?;

        let graph_result = graph_receiver.recv()?;
        let subgraph_result = subgraph_receiver.recv()?;

        match (subgraph_result, graph_result) {
            (Ok(s), _) => {
                tracing::info!("fetching federated SDL succeeded");
                Ok((s, IntrospectRunnerKind::Subgraph(subgraph_runner)))
            }
            (Err(_), Ok(s)) => {
                eprintln!("warn: could not fetch federated SDL, using introspection schema without directives. you should convert this monograph to a federated subgraph. see https://www.apollographql.com/docs/federation/subgraphs/ for more information.");
                Ok((s, IntrospectRunnerKind::Graph(graph_runner)))
            }
            (Err(se), Err(ge)) => Err(anyhow!("could not introspect {}", &self.endpoint)
                .context(se)
                .context(ge)
                .into()),
        }
    }

    fn introspect(
        &self,
        subgraph_runner: SubgraphIntrospectRunner,
        graph_runner: GraphIntrospectRunner,
        current_stage: usize,
        total_stages: usize,
    ) -> ParallelSaucer<SubgraphIntrospectRunner, GraphIntrospectRunner> {
        ParallelSaucer::new(
            subgraph_runner,
            graph_runner,
            "",
            current_stage,
            total_stages,
        )
    }
}

#[derive(Debug, Clone)]
pub enum IntrospectRunnerKind {
    Unknown(UnknownIntrospectRunner),
    Subgraph(SubgraphIntrospectRunner),
    Graph(GraphIntrospectRunner),
}

#[derive(Debug, Clone)]
pub struct SubgraphIntrospectRunner {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl SubgraphIntrospectRunner {
    pub fn run(&self) -> Result<String> {
        tracing::info!("running subgraph introspect");
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

impl Saucer for SubgraphIntrospectRunner {
    fn description(&self) -> String {
        "subgraph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        let sdl_or_error = self.run();
        self.sender.send(sdl_or_error)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct GraphIntrospectRunner {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl GraphIntrospectRunner {
    pub fn run(&self) -> Result<String> {
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

impl Saucer for GraphIntrospectRunner {
    fn description(&self) -> String {
        "graph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        tracing::info!("running graph introspect");
        let sdl_or_error = self.run();
        self.sender.send(sdl_or_error)?;
        Ok(())
    }
}
