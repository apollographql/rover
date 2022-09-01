use crate::{error::RoverError, Result};
use apollo_federation_types::build::SubgraphDefinition;
use interprocess::local_socket::LocalSocketStream;
use saucer::{anyhow, Context};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fmt::Debug, io::BufReader, time::Duration};

use crate::command::dev::protocol::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageSender {
    ipc_socket_addr: String,
    is_main_session: bool,
}

impl MessageSender {
    pub fn new(ipc_socket_addr: &str, is_main_session: bool) -> Self {
        Self {
            ipc_socket_addr: ipc_socket_addr.to_string(),
            is_main_session,
        }
    }

    pub fn new_subgraph(ipc_socket_addr: &str) -> Self {
        Self::new(ipc_socket_addr, false)
    }

    fn should_message(&self) -> bool {
        !self.is_main_session
    }

    pub fn add_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        if self.should_message() {
            eprintln!(
                "notifying main `rover dev` session about new subgraph '{}'",
                &subgraph.name
            );
        }
        let result = self.socket_message::<CompositionResult>(&MessageKind::AddSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })?;

        if let Some(result) = result {
            match result {
                Ok(_) => eprintln!(
                    "successfully re-composed after adding the '{}' subgraph.",
                    &subgraph.name
                ),
                Err(e) => eprintln!("{}", e),
            }
        }
        Ok(())
    }

    pub fn update_subgraph(&self, subgraph: &SubgraphDefinition) -> Result<()> {
        if self.should_message() {
            eprintln!(
                "notifying main `rover dev` session about updated subgraph '{}'",
                &subgraph.name
            );
        }
        let result = self.socket_message::<CompositionResult>(&MessageKind::UpdateSubgraph {
            subgraph_entry: entry_from_definition(subgraph)?,
        })?;

        if let Some(result) = result {
            match result {
                Ok(_) => eprintln!(
                    "successfully re-composed after updating the '{}' subgraph.",
                    &subgraph.name
                ),
                Err(e) => eprintln!("{}", e),
            }
        }
        Ok(())
    }

    pub fn remove_subgraph(&self, subgraph_name: &SubgraphName) -> Result<()> {
        if self.should_message() {
            eprintln!(
                "notifying main `rover dev` session about removed subgraph '{}'",
                &subgraph_name
            );
            let result =
                self.socket_message::<CompositionResult>(&MessageKind::RemoveSubgraph {
                    subgraph_name: subgraph_name.to_string(),
                })?;

            if let Some(result) = result {
                match result {
                    Ok(_) => eprintln!(
                        "successfully re-composed after removing the '{}' subgraph.",
                        &subgraph_name
                    ),
                    Err(e) => eprintln!("{}", e),
                }
            }
        }

        Ok(())
    }

    pub fn kill_router(&self) -> Result<Option<()>> {
        self.socket_message::<()>(&MessageKind::KillRouter)
    }

    pub fn session_subgraphs(&self) -> Option<Vec<SubgraphKey>> {
        if let Ok(Some(subgraphs)) =
            self.socket_message::<Vec<SubgraphKey>>(&MessageKind::GetSubgraphs)
        {
            tracing::info!(
                "the main `rover dev` session currently has {} subgraphs",
                subgraphs.len()
            );
            Some(subgraphs)
        } else {
            tracing::info!("initializing the main `rover dev` session",);
            None
        }
    }

    pub fn health_check(&self) -> Result<()> {
        loop {
            if let Err(e) = self.socket_message::<()>(&MessageKind::HealthCheck) {
                break Err(e);
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }

    pub fn socket_message<T>(&self, message: &MessageKind) -> Result<Option<T>>
    where
        T: Serialize + DeserializeOwned + Debug,
    {
        match self.connect() {
            Ok(stream) => {
                stream
                    .set_nonblocking(true)
                    .context("could not set socket to non-blocking mode")?;
                let mut stream = BufReader::new(stream);

                // send our message over the socket
                socket_write(message, &mut stream)?;

                // wait for our message to be read by the other socket handler
                // then read the response that was written back to the socket
                socket_read(&mut stream)
            }
            Err(e) => Err(e),
        }
    }

    fn connect(&self) -> Result<LocalSocketStream> {
        LocalSocketStream::connect(&*self.ipc_socket_addr).map_err(|_| {
            RoverError::new(anyhow!(
                "the main `rover dev` session has been killed, shutting down"
            ))
        })
    }

    pub fn is_main_session(&self) -> bool {
        self.is_main_session
    }
}
