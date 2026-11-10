//! A bare-bones HTTP server for tests that need to see the `traceparent` each request carried.
//!
//! `httpmock` can match on a header but can't report back what a request sent, and these tests
//! need to compare the values sent across attempts.

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

/// Records the `traceparent` of every request it receives.
///
/// It answers the Nth request with the Nth status it was given, and the last status to every
/// request after that. A `200` carries an empty GraphQL `data` object.
pub(crate) struct TraceRecorder {
    address: SocketAddr,
    traceparents: Arc<Mutex<Vec<Option<String>>>>,
}

impl TraceRecorder {
    pub(crate) async fn start(statuses: Vec<u16>) -> TraceRecorder {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let traceparents = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&traceparents);
        tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let request = read_request(&mut stream).await;
                let status = {
                    let mut recorded = recorded.lock().unwrap();
                    recorded.push(header(&request, "traceparent"));
                    statuses[(recorded.len() - 1).min(statuses.len() - 1)]
                };
                let body = if status == 200 { r#"{"data":{}}"# } else { "" };
                let response = format!(
                    "HTTP/1.1 {status} Test\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
                let _ = stream.shutdown().await;
            }
        });
        TraceRecorder {
            address,
            traceparents,
        }
    }

    pub(crate) fn url(&self) -> String {
        format!("http://{}/graphql", self.address)
    }

    /// The `traceparent` of each request received so far, in order, `None` for a request
    /// that had none
    pub(crate) fn traceparents(&self) -> Vec<Option<String>> {
        self.traceparents.lock().unwrap().clone()
    }
}

/// The text of a request's head, with its body read off the connection
async fn read_request(stream: &mut tokio::net::TcpStream) -> String {
    let mut received = Vec::new();
    let mut buffer = [0u8; 4096];
    let head_end = loop {
        let read = stream.read(&mut buffer).await.unwrap_or(0);
        if read == 0 {
            return String::from_utf8_lossy(&received).into_owned();
        }
        received.extend_from_slice(&buffer[..read]);
        if let Some(end) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break end + 4;
        }
    };
    let head = String::from_utf8_lossy(&received[..head_end]).into_owned();
    let body_length: usize = header(&head, "content-length")
        .and_then(|length| length.parse().ok())
        .unwrap_or(0);
    while received.len() < head_end + body_length {
        let read = stream.read(&mut buffer).await.unwrap_or(0);
        if read == 0 {
            break;
        }
        received.extend_from_slice(&buffer[..read]);
    }
    head
}

fn header(request: &str, name: &str) -> Option<String> {
    request.lines().skip(1).find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim()
            .eq_ignore_ascii_case(name)
            .then(|| value.trim().to_string())
    })
}
