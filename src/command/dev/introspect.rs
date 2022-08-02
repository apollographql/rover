use std::sync::mpsc::{sync_channel, SyncSender};

use reqwest::blocking::Client;
use saucer::{anyhow, ParallelSaucer, Saucer};

use crate::command::subgraph::Introspect;
use crate::command::RoverOutput;
use crate::Result;

#[derive(Clone, Debug)]
pub struct IntrospectRunner {
    endpoint: reqwest::Url,
    client: Client,
}

impl IntrospectRunner {
    pub fn new(endpoint: reqwest::Url, client: Client) -> Self {
        Self { endpoint, client }
    }

    pub fn run(&self) -> Result<String> {
        let (subgraph_sender, subgraph_receiver) = sync_channel(1);
        let (graph_sender, graph_receiver) = sync_channel(1);
        // stage 1 of 1
        self.introspect(subgraph_sender, graph_sender, 1, 1)
            .beam()?;

        let graph_result = graph_receiver.recv()?;
        let subgraph_result = subgraph_receiver.recv()?;

        match (subgraph_result, graph_result) {
            (Ok(s), _) => {
                eprintln!("fetching federated SDL succeeded");
                Ok(s)
            }
            (Err(_), Ok(s)) => {
                eprintln!("warn: could not fetch federated SDL, using introspection schema without directives. you should convert this monograph to a federated subgraph. see https://www.apollographql.com/docs/federation/subgraphs/ for more information.");
                Ok(s)
            }
            (Err(se), Err(ge)) => Err(anyhow!("could not introspect {}", &self.endpoint)
                .context(se)
                .context(ge)
                .into()),
        }
    }

    fn introspect(
        &self,
        subgraph_sender: SyncSender<Result<String>>,
        graph_sender: SyncSender<Result<String>>,
        current_stage: usize,
        total_stages: usize,
    ) -> ParallelSaucer<SubgraphIntrospectRunner, GraphIntrospectRunner> {
        ParallelSaucer::new(
            SubgraphIntrospectRunner {
                sender: subgraph_sender,
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            GraphIntrospectRunner {
                sender: graph_sender,
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            "",
            current_stage,
            total_stages,
        )
    }
}

#[derive(Debug, Clone)]
struct SubgraphIntrospectRunner {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl Saucer for SubgraphIntrospectRunner {
    fn description(&self) -> String {
        "subgraph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        eprintln!("running subgraph introspect");
        let output = Introspect {
            endpoint: self.endpoint.clone(),
            headers: None,
        }
        .run(self.client.clone());
        match output {
            Ok(rover_output) => match rover_output {
                RoverOutput::Introspection(sdl) => {
                    self.sender.send(Ok(sdl))?;
                }
                _ => {
                    self.sender.send(Err(
                        anyhow!("invalid result from subgraph introspect").into()
                    ))?;
                }
            },
            Err(e) => {
                self.sender.send(Err(e))?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct GraphIntrospectRunner {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl Saucer for GraphIntrospectRunner {
    fn description(&self) -> String {
        "graph introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
        eprintln!("running graph introspect");
        let output = Introspect {
            endpoint: self.endpoint.clone(),
            headers: None,
        }
        .run(self.client.clone());
        match output {
            Ok(rover_output) => match rover_output {
                RoverOutput::Introspection(sdl) => {
                    self.sender.send(Ok(sdl))?;
                }
                _ => {
                    self.sender
                        .send(Err(anyhow!("invalid result from graph introspect").into()))?;
                }
            },
            Err(e) => {
                self.sender.send(Err(e))?;
            }
        }
        Ok(())
    }
}
