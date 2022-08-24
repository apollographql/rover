use crate::{
    command::dev::{compose::ComposeRunner, do_dev::log_err_and_continue, router::RouterRunner},
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[non_exhaustive]
pub enum MessageKind {
    AddSubgraph { subgraph_entry: SubgraphEntry },
    UpdateSubgraph { subgraph_entry: SubgraphEntry },
    RemoveSubgraph { subgraph_name: SubgraphName },
    GetSubgraphs,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageSender {
    socket_addr: String,
    is_main_session: bool,
}

impl MessageSender {
    pub fn new(socket_addr: &str, is_main_session: bool) -> Self {
        Self {
            socket_addr: socket_addr.to_string(),
            is_main_session,
        }
    }

    fn should_message(&self, subgraph_name: &SubgraphName) -> bool {
        subgraph_name != &RouterRunner::reserved_subgraph_name()
    }

    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        if self.should_message(&subgraph.name) {
            eprintln!(
                "notifying main `rover dev` session about new subgraph '{}'",
                &subgraph.name
            );
        }
        self.try_send(&MessageKind::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        if self.should_message(&subgraph.name) {
            eprintln!(
                "notifying main `rover dev` session about updated subgraph '{}'",
                &subgraph.name
            );
        }
        self.try_send(&MessageKind::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })
    }

    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> Result<()> {
        if self.should_message(subgraph_name) {
            eprintln!(
                "notifying main `rover dev` session about removed subgraph '{}'",
                &subgraph_name
            );
        }
        if !self.is_main_session {
            self.try_send(&MessageKind::RemoveSubgraph {
                subgraph_name: subgraph_name.to_string(),
            })
        } else {
            Ok(())
        }
    }

    pub fn get_subgraphs(&self) -> Vec<SubgraphKey> {
        let mut all_subgraphs = Vec::from_iter(RouterRunner::reserved_subgraph_keys());
        if let Ok(Some(subgraphs)) =
            self.try_send_and_receive::<Vec<SubgraphKey>>(&MessageKind::GetSubgraphs)
        {
            all_subgraphs.extend(subgraphs);
        }
        all_subgraphs
    }

    pub fn try_send_and_receive<T>(&self, message: &MessageKind) -> Result<Option<T>>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        match self.connect() {
            Ok(mut stream) => {
                // send our message over the socket
                socket_write(message, &mut stream)?;

                // wait for our message to be read by the other socket handler
                // then read the response that was written back to the socket
                socket_read(&mut stream)
            }
            Err(e) => Err(e),
        }
    }

    pub fn try_send(&self, message: &MessageKind) -> Result<()> {
        match self.connect() {
            Ok(mut stream) => Ok(socket_write(message, &mut stream)?),
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
}

impl MessageReceiver {
    pub fn new(socket_addr: &str, compose_runner: ComposeRunner) -> Result<Self> {
        if LocalSocketStream::connect(socket_addr).is_ok() {
            Err(RoverError::new(anyhow!(
                "there is already a main `rover dev` session"
            )))
        } else {
            Ok(Self {
                subgraphs: HashMap::new(),
                socket_addr: socket_addr.to_string(),
                compose_runner,
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
        ready_sender: SyncSender<()>,
        compose_sender: SyncSender<ComposeResult>,
    ) -> Result<()> {
        let listener = LocalSocketListener::bind(&*self.socket_addr).with_context(|| {
            format!(
                "could not start local socket server at {}",
                &self.socket_addr
            )
        })?;
        ready_sender.send(()).unwrap();
        tracing::info!(
            "connected to socket {}, waiting for messages",
            &self.socket_addr
        );
        listener
            .incoming()
            .filter_map(handle_socket_error)
            .for_each(|mut stream| {
                tracing::info!("received incoming socket connection");
                let was_composed = self.compose_runner.has_composed();
                match socket_read::<MessageKind>(&mut stream) {
                    Ok(Some(message)) => match message {
                        MessageKind::AddSubgraph { subgraph_entry } => {
                            eprintln!(
                                "adding subgraph with name '{}' to the main `rover dev` session",
                                &subgraph_entry.0 .0
                            );
                            let _ = self
                                .add_subgraph(&subgraph_entry)
                                .map(|_| {
                                    let _ =
                                        self.compose_runner.run(self).map_err(log_err_and_continue);
                                })
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::UpdateSubgraph { subgraph_entry } => {
                            eprintln!(
                                "updating subgraph with name '{}' from the main `rover dev` session",
                                &subgraph_entry.0 .0
                            );
                            let _ = self
                                .update_subgraph(&subgraph_entry)
                                .map(|_| {
                                    let _ =
                                        self.compose_runner.run(self).map_err(log_err_and_continue);
                                })
                                .map_err(log_err_and_continue);
                        }
                        MessageKind::RemoveSubgraph { subgraph_name } => {
                            eprintln!(
                                "removing subgraph with name '{}' from the main `rover dev` session",
                                &subgraph_name
                            );
                            let _ = self.remove_subgraph(&subgraph_name).map(|_| {
                                let _ = self.compose_runner.run(self).map_err(log_err_and_continue);
                            });
                        }
                        MessageKind::GetSubgraphs => {
                            eprintln!("notifying new `rover dev` session about existing subgraphs");
                            let _ = socket_write(&self.get_subgraphs(), &mut stream)
                                .map_err(log_err_and_continue);
                        }
                    },
                    Ok(None) => {}
                    Err(e) => log_err_and_continue(e),
                }

                let has_composed = self.compose_runner.has_composed();

                if has_composed && !was_composed {
                    compose_sender.send(ComposeResult::Succeed).unwrap();
                } else if !has_composed && was_composed {
                    compose_sender.send(ComposeResult::Fail).unwrap();
                }
            });
        Ok(())
    }

    pub fn get_subgraphs(&self) -> Vec<SubgraphKey> {
        self.subgraphs.keys().cloned().collect()
    }
}

pub enum ComposeResult {
    Succeed,
    Fail,
    Kill,
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

fn socket_read<B>(stream: &mut LocalSocketStream) -> Result<Option<B>>
where
    B: Serialize + DeserializeOwned + Debug,
{
    tracing::info!("\n----    RECEIVE     ----\n");
    let mut incoming_message = String::new();
    let mut stream_reader = BufReader::new(stream);
    stream_reader
        .read_line(&mut incoming_message)
        .context("could not read incoming message")?;

    let res = if incoming_message.is_empty() {
        None
    } else {
        let incoming_message: B =
            serde_json::from_str(&incoming_message).context("incoming message was not valid")?;
        tracing::info!("\n{:?}\n", &incoming_message);
        Some(incoming_message)
    };

    tracing::info!("\n====   END RECEIVE    ====\n");
    Ok(res)
}

fn socket_write<A>(message: &A, stream: &mut LocalSocketStream) -> Result<()>
where
    A: Serialize + DeserializeOwned + Debug,
{
    tracing::info!("\n----      SEND      ----\n");
    tracing::info!("\n{:?}\n", &message);
    let outgoing_json = serde_json::to_string(message)
        .with_context(|| format!("could not convert outgoing message {:?} to json", &message))?;
    let outgoing_string = format!("{}\n", &outgoing_json);
    stream
        .write_all(outgoing_string.as_bytes())
        .context("could not write outgoing message to socket")?;
    tracing::info!("\n====    END SEND     ====\n");
    Ok(())
}
