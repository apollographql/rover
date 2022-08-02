use std::sync::mpsc::{sync_channel, SyncSender};

use reqwest::blocking::Client;
use saucer::{anyhow, ParallelSaucer, Saucer};

use crate::command::subgraph::Introspect;
use crate::command::RoverOutput;
use crate::Result;

#[derive(Clone, Debug)]
pub struct IntrospectRunner {
    endpoint: reqwest::Url,
    sdl_sender: SyncSender<saucer::Result<String>>,
    client: Client,
}

impl IntrospectRunner {
    pub fn new(
        endpoint: reqwest::Url,
        sdl_sender: SyncSender<saucer::Result<String>>,
        client: Client,
    ) -> Self {
        Self {
            endpoint,
            sdl_sender,
            client,
        }
    }
    pub fn introspect(
        &self,
        subgraph_sender: SyncSender<Result<String>>,
        graph_sender: SyncSender<Result<String>>,
        current_stage: usize,
        total_stages: usize,
    ) -> ParallelSaucer<SubgraphIntrospectSaucer, GraphIntrospectSaucer> {
        ParallelSaucer::new(
            SubgraphIntrospectSaucer {
                sender: subgraph_sender.clone(),
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            GraphIntrospectSaucer {
                sender: graph_sender.clone(),
                endpoint: self.endpoint.clone(),
                client: self.client.clone(),
            },
            &self.prefix(),
            current_stage,
            total_stages,
        )
    }
}

impl Saucer for IntrospectRunner {
    fn description(&self) -> String {
        "introspect".to_string()
    }

    fn beam(&self) -> saucer::Result<()> {
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
                self.sdl_sender.send(Ok(s))?;
            }
            (Err(_), Ok(s)) => {
                eprintln!("warn: could not fetch federated SDL, using introspection schema without directives. you should convert this monograph to a federated subgraph. see https://www.apollographql.com/docs/federation/subgraphs/ for more information.");
                self.sdl_sender.send(Ok(s))?;
            }
            (Err(se), Err(ge)) => {
                self.sdl_sender
                    .send(Err(anyhow!("could not introspect {}", &self.endpoint)
                        .context(se)
                        .context(ge)))?;
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct SubgraphIntrospectSaucer {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl Saucer for SubgraphIntrospectSaucer {
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
struct GraphIntrospectSaucer {
    endpoint: reqwest::Url,
    sender: SyncSender<Result<String>>,
    client: Client,
}

impl Saucer for GraphIntrospectSaucer {
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
