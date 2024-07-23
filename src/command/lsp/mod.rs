use std::sync::Arc;

use apollo_federation_types::config::{FederationVersion, SupergraphConfig};
use apollo_language_server_core::server::ApolloLanguageServer;
use clap::Parser;
use futures::{channel::mpsc::channel, StreamExt};
use serde::Serialize;
use tokio::runtime::Runtime;
use tower_lsp::{LspService, Server};

use super::supergraph::compose::Compose;
use crate::{
    options::{LicenseAccepter, PluginOpts, ProfileOpt},
    utils::client::StudioClientConfig,
    RoverOutput, RoverResult,
};

#[derive(Debug, Parser, Serialize)]
pub struct Lsp;

impl Lsp {
    pub fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        let runtime = Runtime::new().expect("Failed to create Tokio runtime");
        runtime.block_on(run_lsp(client_config));
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (sender, mut receiver) = channel(1);
    let (service, socket) =
        LspService::new(|client| Arc::new(ApolloLanguageServer::new(client, sender)));

    let language_server = service.inner().clone();

    let server = Server::new(stdin, stdout, socket);

    let composer = Compose::new(PluginOpts {
        profile: ProfileOpt {
            profile_name: "default".to_string(),
        },
        elv2_license_accepter: LicenseAccepter {
            elv2_license_accepted: Some(true),
        },
        skip_update: true,
    });

    tokio::spawn(async move {
        while let Some(definitions) = receiver.next().await {
            tracing::info!("Received message: {:?}", definitions);

            let mut supergraph_config = SupergraphConfig::from(definitions);
            supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);

            match composer.exec(None, client_config.clone(), &mut supergraph_config) {
                Ok(composition_output) => {
                    dbg!(&composition_output);
                    language_server
                        .composition_did_update(Some(composition_output.supergraph_sdl), vec![])
                        .await
                }
                Err(errors) => {
                    dbg!(&errors);
                    tracing::error!("Error composing supergraph: {:?}", errors);
                }
            }
        }
    });

    server.serve(service).await;
}
