use crate::{
    command::dev::{
        command::CommandRunner, compose::ComposeRunner, do_dev::log_err_and_continue,
        router::RouterRunner,
    },
    error::RoverError,
    Result,
};
use apollo_federation_types::{
    build::SubgraphDefinition,
    config::{FederationVersion, SupergraphConfig},
};
use interprocess::local_socket::{LocalSocketListener, LocalSocketStream};
use reqwest::Url;
use saucer::{anyhow, Context};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt::Debug,
    io::{self, BufRead, BufReader, Write},
    sync::mpsc::SyncSender,
};
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};

#[derive(Serialize, Deserialize, Debug)]
#[non_exhaustive]
pub enum MessageKind {
    AddSubgraph {
        subgraph_entry: SubgraphEntry,
    },
    UpdateSubgraph {
        subgraph_entry: SubgraphEntry,
    },
    RemoveSubgraph {
        subgraph_name: SubgraphName,
    },
    RestartProcess {
        subgraph_name: SubgraphName,
        process_id: u32,
    },
    GetSubgraphs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageSender {
    socket_addr: String,
}

impl MessageSender {
    pub fn new(socket_addr: &str) -> Self {
        Self {
            socket_addr: socket_addr.to_string(),
        }
    }

    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.try_send(MessageKind::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        self.try_send(MessageKind::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> Result<()> {
        self.try_send(MessageKind::RemoveSubgraph {
            subgraph_name: subgraph_name.to_string(),
        })
    }

    // TODO: perhaps watch the entire project directory
    // and restart the process when any file changes
    pub fn _restart_process(&self, subgraph: &SubgraphDefinition, process_id: u32) -> Result<()> {
        self.try_send(MessageKind::RestartProcess {
            subgraph_name: name_from_definition(subgraph),
            process_id,
        })
    }

    pub fn get_subgraphs(&self) -> Result<Vec<SubgraphKey>> {
        let maybe_subgraphs = self.try_send_and_receive(MessageKind::GetSubgraphs)?;
        if let Some(subgraphs) = maybe_subgraphs {
            Ok(subgraphs)
        } else {
            Ok(Vec::new())
        }
    }

    pub fn try_send(&self, message: MessageKind) -> Result<()> {
        match self.connect() {
            Ok(mut stream) => Ok(try_send(&message, &mut stream)?),
            Err(e) => Err(e),
        }
    }

    pub fn try_send_and_receive<T>(&self, message: MessageKind) -> Result<Option<T>>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        match self.connect() {
            Ok(mut stream) => Ok(try_send_and_receive(&message, &mut stream)?),
            Err(e) => Err(e),
        }
    }

    fn connect(&self) -> Result<LocalSocketStream> {
        LocalSocketStream::connect(&*self.socket_addr).map_err(|_| {
            RoverError::new(anyhow!(
                "main `rover dev` session has been killed, shutting down"
            ))
        })
    }
}

pub type SubgraphName = String;
pub type SubgraphUrl = Url;
pub type SubgraphSdl = String;
pub type SubgraphKey = (SubgraphName, SubgraphUrl);
pub type SubgraphEntry = (SubgraphKey, SubgraphSdl);

fn sdl_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphSdl {
    subgraph_definition.sdl.to_string()
}

fn name_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphName {
    subgraph_definition.name.to_string()
}

fn url_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphUrl> {
    Ok(subgraph_definition.url.parse()?)
}

fn key_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphKey> {
    Ok((
        name_from_definition(subgraph_definition),
        url_from_definition(subgraph_definition)?,
    ))
}

fn entry_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphEntry> {
    Ok((
        key_from_definition(subgraph_definition)?,
        sdl_from_definition(subgraph_definition),
    ))
}

#[derive(Debug)]
pub struct MessageReceiver {
    subgraphs: HashMap<SubgraphKey, SubgraphSdl>,
    socket_addr: String,
    compose_runner: ComposeRunner,
    router_runner: RouterRunner,
}

impl MessageReceiver {
    pub fn new(
        socket_addr: &str,
        compose_runner: ComposeRunner,
        router_runner: RouterRunner,
    ) -> Result<Self> {
        if LocalSocketStream::connect(socket_addr).is_ok() {
            Err(RoverError::new(anyhow!("a composer is already running")))
        } else {
            Ok(Self {
                subgraphs: HashMap::new(),
                socket_addr: socket_addr.to_string(),
                compose_runner,
                router_runner,
            })
        }
    }

    pub fn supergraph_config(&self) -> SupergraphConfig {
        let mut supergraph_config: SupergraphConfig = self
            .subgraphs
            .iter()
            .map(|((name, url), sdl)| SubgraphDefinition::new(name, url.to_string(), sdl))
            .collect::<Vec<SubgraphDefinition>>()
            .into();
        supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);
        supergraph_config
    }

    pub fn add_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> Result<()> {
        let ((name, url), sdl) = subgraph_entry;
        if self
            .subgraphs
            .get(&(name.to_string(), url.clone()))
            .is_some()
        {
            Err(RoverError::new(anyhow!(
                "subgraph with name '{}' and url '{}' already exists",
                &name,
                &url
            )))
        } else {
            self.subgraphs
                .insert((name.to_string(), url.clone()), sdl.to_string());
            Ok(())
        }
    }

    pub fn update_subgraph(&mut self, subgraph_entry: &SubgraphEntry) -> Result<()> {
        let ((name, url), sdl) = subgraph_entry;
        if let Some(prev_sdl) = self.subgraphs.get_mut(&(name.to_string(), url.clone())) {
            if prev_sdl != sdl {
                *prev_sdl = sdl.to_string();
            }
            Ok(())
        } else {
            self.add_subgraph(subgraph_entry)
        }
    }

    pub fn remove_subgraph(&mut self, subgraph_name: &SubgraphName) -> Result<()> {
        let mut found = None;
        for (name, url) in self.subgraphs.keys() {
            if name == subgraph_name {
                found = Some((name, url));
                break;
            }
        }
        if let Some((name, url)) = found {
            self.subgraphs.remove(&(name.to_string(), url.clone()));
            Ok(())
        } else {
            Err(RoverError::new(anyhow!(
                "subgraph with name '{}' does not exist",
                &subgraph_name,
            )))
        }
    }

    pub fn receive_messages(
        &mut self,
        command_runner: &mut CommandRunner,
        ready_sender: SyncSender<()>,
    ) -> Result<()> {
        let listener = LocalSocketListener::bind(&*self.socket_addr).with_context(|| {
            format!(
                "could not start local socket server at {}",
                &self.socket_addr
            )
        })?;
        ready_sender.send(()).unwrap();
        listener.incoming().filter_map(handle_socket_error).for_each(|mut stream| {
            tracing::info!("received incoming socket connection");
            let was_composed = self.compose_runner.has_composed();
            match try_receive::<MessageKind>(&mut stream) {
                Ok(Some(message)) => {
                    tracing::info!("successfully parsed message");
                    match message {
                        MessageKind::AddSubgraph { subgraph_entry } => {
                            let _ = self
                                .add_subgraph(&subgraph_entry)
                                .map(|_| {
                                    let _ = self.compose_runner.run(self).map_err(log_err_and_continue);
                                })
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::UpdateSubgraph { subgraph_entry } => {
                            let _ = self
                                .update_subgraph(&subgraph_entry)
                                .map(|_| {
                                    let _ = self.compose_runner.run(self).map_err(log_err_and_continue);
                                })
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::RemoveSubgraph { subgraph_name } => {
                            let _ = self
                                .remove_subgraph(&subgraph_name)
                                .map(|_| {
                                    let _ = self.compose_runner.run(self).map_err(log_err_and_continue);
                                });
                        }
                        MessageKind::RestartProcess {
                            subgraph_name,
                            process_id,
                        } => {
                            let _ = self.remove_subgraph(&subgraph_name).map(|_| {
                                self.compose_runner.run(self).map(|_| {
                                    let system = System::new();
                                    if let Some(process) = system.process(Pid::from_u32(process_id)) {
                                        if !process.kill() {
                                            eprintln!(
                                                "couldn't kill process for subgraph '{}' with pid {}",
                                                &subgraph_name, process_id
                                            );
                                        }
                                    } else {
                                        eprintln!(
                                            "no process found for subgraph '{}' with pid {}",
                                            &subgraph_name, process_id
                                        );
                                    }
                                }).map_err(log_err_and_continue)
                            }).map_err(log_err_and_continue);
                        }
                        MessageKind::GetSubgraphs => {
                            let _ = try_send(&self.get_subgraphs(), &mut stream).map_err(log_err_and_continue);
                        }
                    }
                },
                Ok(None) => {},
                Err(e) => {
                    log_err_and_continue(e)
                }
            }
            match (was_composed, self.compose_runner.has_composed()) {
                (false, true) => {
                    self.router_runner.spawn(command_runner).expect("could not spawn router");
                },
                (true, false) => {
                    eprintln!("router no longer composes, spinning down");
                    command_runner.remove_subgraph_message(&self.router_runner.reserved_subgraph_name());
                }
                _ => {}
            };
        });
        Ok(())
    }

    pub fn get_subgraphs(&self) -> Vec<SubgraphKey> {
        let mut endpoints = self.subgraphs.keys().cloned().collect::<Vec<SubgraphKey>>();

        endpoints.extend(self.router_runner.reserved_subgraph_keys());
        endpoints
    }
}

fn handle_socket_error(conn: io::Result<LocalSocketStream>) -> Option<LocalSocketStream> {
    match conn {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!("incoming connection failed: {}", error);
            None
        }
    }
}

fn try_send_and_receive<A, B>(message: &A, stream: &mut LocalSocketStream) -> Result<Option<B>>
where
    A: Serialize + DeserializeOwned + Debug,
    B: Serialize + DeserializeOwned + Debug,
{
    tracing::debug!("\n---- SEND & RECEIVE ----\n");
    try_send(message, stream)?;
    let result = try_receive(stream)?;
    tracing::debug!("\n== END SEND & RECEIVE ==\n");
    Ok(result)
}

fn try_receive<B>(stream: &mut LocalSocketStream) -> Result<Option<B>>
where
    B: Serialize + DeserializeOwned + Debug,
{
    tracing::debug!("\n----    RECEIVE     ----\n");
    let mut stream_reader = BufReader::new(stream);

    let maybe_buf = stream_reader.fill_buf();
    if let Ok(buf) = maybe_buf {
        if buf.is_empty() {
            Ok(None)
        } else {
            let mut incoming_message = String::new();
            stream_reader
                .read_line(&mut incoming_message)
                .context("could not read incoming message")?;
            let incoming_message: B = serde_json::from_str(&incoming_message)
                .context("incoming message was not valid")?;
            tracing::debug!("\n{:?}\n", &incoming_message);
            tracing::debug!("\n====   END RECEIVE    ====\n");
            Ok(Some(incoming_message))
        }
    } else {
        Err(RoverError::new(anyhow!(
            "something went wrong while receiving a message over the socket"
        )))
    }
}

fn try_send<A>(message: &A, stream: &mut LocalSocketStream) -> Result<()>
where
    A: Serialize + DeserializeOwned + Debug,
{
    tracing::debug!("\n----      SEND      ----\n");
    tracing::debug!("\n{:?}\n", &message);
    let outgoing_json = serde_json::to_string(message)
        .with_context(|| format!("could not convert outgoing message {:?} to json", &message))?;
    let outgoing_string = format!("{}\n", &outgoing_json);
    stream
        .write_all(outgoing_string.as_bytes())
        .context("could not write outgoing message to socket")?;
    tracing::debug!("\n====    END SEND     ====\n");
    Ok(())
}
