use std::{
    fmt::Debug,
    io::{self, BufRead, BufReader, Write},
};

use anyhow::{anyhow, Context, Error};
use interprocess::local_socket::Stream;
use serde::{de::DeserializeOwned, Serialize};

use crate::RoverResult;

pub(crate) fn handle_socket_error(conn: io::Result<Stream>) -> Option<Stream> {
    match conn {
        Ok(val) => Some(val),
        Err(error) => {
            eprintln!("incoming connection failed: {}", error);
            None
        }
    }
}

pub(crate) fn socket_read<B>(stream: &mut BufReader<Stream>) -> std::result::Result<B, Error>
where
    B: Serialize + DeserializeOwned + Debug,
{
    let mut incoming_message = String::new();

    match stream.read_line(&mut incoming_message) {
        Ok(_) => {
            if incoming_message.is_empty() {
                Err(anyhow!("incoming message was empty"))
            } else {
                let incoming_message: B =
                    serde_json::from_str(&incoming_message).with_context(|| {
                        format!(
                            "incoming message '{}' was not valid JSON",
                            &incoming_message
                        )
                    })?;
                Ok(incoming_message)
            }
        }
        Err(e) => Err(Error::new(e).context("could not read incoming message")),
    }
}

pub(crate) fn socket_write<A>(message: &A, stream: &mut BufReader<Stream>) -> RoverResult<()>
where
    A: Serialize + DeserializeOwned + Debug,
{
    let outgoing_json = serde_json::to_string(message)
        .with_context(|| format!("could not convert outgoing message {:?} to json", &message))?;
    let outgoing_string = format!("{}\n", &outgoing_json);
    stream
        .get_mut()
        .write_all(outgoing_string.as_bytes())
        .with_context(|| {
            format!(
                "could not write outgoing message {:?} to socket",
                &outgoing_json
            )
        })?;
    Ok(())
}
