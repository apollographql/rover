use apollo_language_server_core::server::ApolloLanguageServer;
use clap::Parser;
use futures::{channel::mpsc::channel, StreamExt};
use serde::Serialize;
use tokio::runtime::Runtime;
use tower_lsp::{LspService, Server};

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser, Serialize)]
pub struct Lsp;

impl Lsp {
    pub async fn run(&self) -> RoverResult<RoverOutput> {
        run_lsp().await;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (sender, mut receiver) = channel(1);
    let (service, socket) = LspService::new(|client| ApolloLanguageServer::new(client, sender));
    let server = Server::new(stdin, stdout, socket);
    tokio::spawn(async move {
        while let Some(definitions) = receiver.next().await {
            // TODO: run composition
            tracing::info!("Received message: {:?}", definitions);
        }
    });
    server.serve(service).await;
}
