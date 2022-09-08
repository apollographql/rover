use crate::Result;
use apollo_federation_types::build::SubgraphDefinition;
use interprocess::local_socket::LocalSocketStream;
use reqwest::Url;
use saucer::{anyhow, Context, Error};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    fmt::Debug,
    io::{self, BufRead, BufReader, Write},
    time::{Duration, Instant},
};

pub type SubgraphName = String;
pub type SubgraphUrl = Url;
pub type SubgraphSdl = String;
pub type SubgraphKey = (SubgraphName, SubgraphUrl);
pub type SubgraphEntry = (SubgraphKey, SubgraphSdl);
pub type CompositionResult = std::result::Result<SubgraphSdl, String>;

pub(crate) fn sdl_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphSdl {
    subgraph_definition.sdl.to_string()
}

pub(crate) fn name_from_definition(subgraph_definition: &SubgraphDefinition) -> SubgraphName {
    subgraph_definition.name.to_string()
}

pub(crate) fn url_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphUrl> {
    Ok(subgraph_definition.url.parse()?)
}

pub(crate) fn key_from_definition(subgraph_definition: &SubgraphDefinition) -> Result<SubgraphKey> {
    Ok((
        name_from_definition(subgraph_definition),
        url_from_definition(subgraph_definition)?,
    ))
}

pub(crate) fn entry_from_definition(
    subgraph_definition: &SubgraphDefinition,
) -> Result<SubgraphEntry> {
    Ok((
        key_from_definition(subgraph_definition)?,
        sdl_from_definition(subgraph_definition),
    ))
}

pub(crate) fn handle_socket_error(
    conn: io::Result<LocalSocketStream>,
) -> Option<LocalSocketStream> {
    match conn {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!("incoming connection failed: {}", error);
            None
        }
    }
}

pub(crate) fn socket_read<B>(stream: &mut BufReader<LocalSocketStream>) -> Result<Option<B>>
where
    B: Serialize + DeserializeOwned + Debug,
{
    tracing::debug!("\n----    RECEIVE     ----\n");
    let mut incoming_message = String::new();

    let now = Instant::now();

    let result = loop {
        if now.elapsed() > Duration::from_secs(5) {
            break Err(anyhow!(
                "could not read incoming message from the main `rover dev` session after 5 seconds"
            ));
        }

        match stream.read_line(&mut incoming_message) {
            Ok(_) => {
                break Ok(
                    if incoming_message.is_empty() || &incoming_message == "null\n" {
                        None
                    } else {
                        let incoming_message: B = serde_json::from_str(&incoming_message)
                            .with_context(|| {
                                format!(
                                    "incoming message '{}' was not valid JSON",
                                    &incoming_message
                                )
                            })?;
                        tracing::debug!("\n{:?}\n", &incoming_message);
                        Some(incoming_message)
                    },
                )
            }
            Err(e) => {
                if !matches!(e.kind(), io::ErrorKind::WouldBlock) {
                    break Err(Error::new(e).context("could not read incoming message"));
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }?;

    tracing::debug!("\n====   END RECEIVE    ====\n");
    Ok(result)
}

pub(crate) fn socket_write<A>(message: &A, stream: &mut BufReader<LocalSocketStream>) -> Result<()>
where
    A: Serialize + DeserializeOwned + Debug,
{
    tracing::debug!("\n----      SEND      ----\n");
    tracing::debug!("\n{:?}\n", &message);
    let outgoing_json = serde_json::to_string(message)
        .with_context(|| format!("could not convert outgoing message {:?} to json", &message))?;
    let outgoing_string = format!("{}\n", &outgoing_json);
    stream
        .get_mut()
        .write_all(outgoing_string.as_bytes())
        .context("could not write outgoing message to socket")?;
    tracing::debug!("\n====    END SEND     ====\n");
    Ok(())
}
