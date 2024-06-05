use apollo_language_server_core::server::ApolloLanguageServer;
use clap::Parser;
use serde::Serialize;
use tokio::runtime::Runtime;
use tower_lsp::{LspService, Server};

use crate::{RoverOutput, RoverResult};

#[derive(Debug, Parser, Serialize)]
pub struct Lsp;

impl Lsp {
    pub fn run(&self) -> RoverResult<RoverOutput> {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        runtime.block_on(run_lsp());
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(ApolloLanguageServer::new);
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
}
