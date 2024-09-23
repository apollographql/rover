use core::panic;

use apollo_federation_types::{
    config::FederationVersion, javascript::SubgraphDefinition, rover::BuildErrors,
};
use apollo_language_server::{ApolloLanguageServer, Config};
use clap::Parser;
use futures::{channel::mpsc::Receiver, StreamExt};
use serde::Serialize;
use tower_lsp::Server;

use super::supergraph::compose::Compose;
use crate::federation::supergraph_config::{
    get_supergraph_config, ResolvedSubgraphConfig, ResolvedSupergraphConfig,
};
use crate::{
    options::PluginOpts,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
    RoverOutput, RoverResult,
};

#[derive(Debug, Serialize, Parser)]
pub struct Lsp {
    #[clap(flatten)]
    pub(crate) opts: LspOpts,
}

#[derive(Clone, Debug, Serialize, Parser)]
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

        run_lsp(client_config, &self.opts).await;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig, lsp_opts: &LspOpts) {
    let (service, socket, receiver) = ApolloLanguageServer::build_service(Config {
        root_uri: "".into(),
        enable_auto_composition: true,
        force_federation: false,
        disable_telemetry: false,
    });

    let language_server = service.inner().clone();

    tokio::spawn(run_composer_in_thread(
        receiver,
        lsp_opts.clone(),
        client_config,
        language_server,
    ));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
}

async fn run_composer_in_thread(
    mut receiver: Receiver<Vec<SubgraphDefinition>>,
    lsp_opts: LspOpts,
    client_config: StudioClientConfig,
    language_server: ApolloLanguageServer,
) {
    let composer = Compose::new(lsp_opts.plugin_opts.clone());
    let federation_version =
        dbg!(get_federation_version(lsp_opts.clone(), client_config.clone()).await);
    match composer
        .maybe_install_supergraph(None, client_config.clone(), federation_version.clone())
        .await
    {
        Ok(_) => {}
        Err(err) => {
            panic!("Failed to install supergraph plugin: {:?}", err);
        }
    };

    while let Some(mut definitions) = receiver.next().await {
        while let Some(next_definitions) = receiver.try_next().ok().flatten() {
            definitions = next_definitions
        }
        tracing::info!("Received message: {:?}", definitions);
        dbg!(&definitions);

        let subgraphs = definitions
            .into_iter()
            .map(|def| (def.name, ResolvedSubgraphConfig::new(def.url, def.sdl)))
            .collect();
        let supergraph_config = ResolvedSupergraphConfig {
            subgraphs,
            federation_version: federation_version.clone(),
        };

        match composer
            .compose(None, client_config.clone(), supergraph_config, None)
            .await
        {
            Ok(RoverOutput::CompositionResult(composition_output)) => {
                dbg!(&composition_output);
                language_server
                    .composition_did_update(
                        Some(composition_output.supergraph_sdl),
                        composition_output
                            .hints
                            .into_iter()
                            .map(Into::into)
                            .collect(),
                        None,
                    )
                    .await
            }
            Err(rover_error) => {
                dbg!(&rover_error);
                let build_errors: BuildErrors = rover_error.into();
                dbg!(&build_errors);
                language_server
                    .composition_did_update(
                        None,
                        build_errors.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            _ => panic!("Expected CompositionResult"),
        }
    }
}

async fn get_federation_version(
    lsp_opts: LspOpts,
    client_config: StudioClientConfig,
) -> FederationVersion {
    if let Some(supergraph_yaml) = &lsp_opts.supergraph_yaml {
        if let Ok(Some(supergraph_config)) = get_supergraph_config(
            &None,
            Some(supergraph_yaml),
            None,
            client_config.clone(),
            &lsp_opts.plugin_opts.profile,
        )
        .await
        {
            supergraph_config
                .get_federation_version()
                .unwrap_or(FederationVersion::LatestFedTwo)
        } else {
            tracing::warn!("Failed to resolve supergraph yaml");
            FederationVersion::LatestFedTwo
        }
    } else {
        FederationVersion::LatestFedTwo
    }
}
