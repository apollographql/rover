use anyhow::anyhow;
use apollo_federation_types::javascript::SubgraphDefinition;
use apollo_language_server::{ApolloLanguageServer, Config};
use clap::Parser;
use futures::{channel::mpsc::Receiver, StreamExt};
use serde::Serialize;
use tower_lsp::Server;

use crate::federation::supergraph_config::{get_supergraph_config, resolve_supergraph_config};
use crate::federation::Composer;
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
) -> RoverResult<()> {
    let initial_config = get_supergraph_config(
        &None,
        lsp_opts.supergraph_yaml.as_ref(),
        None,
        client_config.clone(),
        &lsp_opts.plugin_opts.profile,
    )
    .await?
    .ok_or_else(|| anyhow!("error getting supergraph config"))?;
    let resolved_config = resolve_supergraph_config(
        initial_config,
        client_config.clone(),
        &lsp_opts.plugin_opts.profile,
    )
    .await?;
    let mut composer = Composer::new(
        resolved_config,
        None,
        client_config,
        lsp_opts.plugin_opts.elv2_license_accepter,
        lsp_opts.plugin_opts.skip_update,
    )
    .await?;

    while let Some(mut definitions) = receiver.next().await {
        while let Some(next_definitions) = receiver.try_next().ok().flatten() {
            definitions = next_definitions
        }
        tracing::info!("Received message: {:?}", definitions);
        dbg!(&definitions);

        for subgraph in definitions {
            if let Some(subgraph_config) =
                composer.supergraph_config.subgraphs.get_mut(&subgraph.name)
            {
                subgraph_config.schema.sdl = subgraph.sdl;
            }
        }

        match composer.compose(None).await {
            Ok(Ok(output)) => {
                dbg!(&output);
                language_server
                    .composition_did_update(
                        Some(output.supergraph_sdl),
                        output.hints.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            Ok(Err(build_errors)) => {
                dbg!(&build_errors);
                language_server
                    .composition_did_update(
                        None,
                        build_errors.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    Ok(())
}
