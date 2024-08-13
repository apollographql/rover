use std::sync::Arc;

use apollo_federation_types::{
    config::{FederationVersion, SupergraphConfig},
    rover::BuildErrors,
};
use apollo_language_server_core::server::ApolloLanguageServer;
use clap::Parser;
use futures::{channel::mpsc::channel, StreamExt};
use serde::Serialize;
use tower_lsp::{LspService, Server};

use super::supergraph::compose::Compose;
use crate::{
    options::PluginOpts,
    utils::{
        client::StudioClientConfig, parsers::FileDescriptorType,
        supergraph_config::resolve_supergraph_yaml,
    },
    RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Lsp {
    #[clap(flatten)]
    pub(crate) opts: LspOpts,
}

#[derive(Debug, Serialize, Parser)]
pub struct LspOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    /// The relative path to the supergraph configuration file. You can pass `-` to use stdin instead of a file.
    #[serde(skip_serializing)]
    #[arg(long = "supergraph-config")]
    supergraph_yaml: Option<FileDescriptorType>,
}

impl Lsp {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        let composer = Compose::new(self.opts.plugin_opts.clone());

        let mut federation_version = FederationVersion::LatestFedTwo;
        if let Some(supergraph_yaml) = &self.opts.supergraph_yaml {
            if let Some(supergraph_config) = resolve_supergraph_yaml(
                &supergraph_yaml,
                client_config.clone(),
                &self.opts.plugin_opts.profile,
            )
            .await
            .ok()
            {
                federation_version = supergraph_config
                    .get_federation_version()
                    .unwrap_or(FederationVersion::LatestFedTwo);
            }
        }

        composer
            .maybe_install_supergraph(None, client_config.clone(), federation_version)
            .await?;

        run_lsp(client_config, self.opts.plugin_opts.clone()).await;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig, plugin_opts: PluginOpts) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (sender, mut receiver) = channel(10);
    let (service, socket) =
        LspService::new(|client| Arc::new(ApolloLanguageServer::new(client, sender)));

    let language_server = service.inner().clone();

    let server = Server::new(stdin, stdout, socket);

    let composer = Compose::new(PluginOpts {
        skip_update: true,
        ..plugin_opts
    });

    tokio::spawn(async move {
        while let Some(definitions) = receiver.next().await {
            tracing::info!("Received message: {:?}", definitions);
            dbg!(&definitions);

            let mut supergraph_config = SupergraphConfig::from(definitions);
            supergraph_config.set_federation_version(FederationVersion::LatestFedTwo);

            match composer
                .exec(None, client_config.clone(), &mut supergraph_config)
                .await
            {
                Ok(composition_output) => {
                    language_server
                        .composition_did_update(
                            Some(composition_output.supergraph_sdl),
                            composition_output
                                .hints
                                .into_iter()
                                .map(Into::into)
                                .collect(),
                        )
                        .await
                }
                Err(rover_error) => {
                    let build_errors: BuildErrors = rover_error.into();
                    dbg!(&build_errors);
                    // tracing::error!("Error composing supergraph: {:?}", errors);
                    language_server
                        .composition_did_update(
                            None,
                            build_errors.into_iter().map(Into::into).collect(),
                        )
                        .await
                }
            }
        }
    });

    server.serve(service).await;
}
