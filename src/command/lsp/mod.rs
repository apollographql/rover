use anyhow::anyhow;
use apollo_federation_types::config::SupergraphConfig;
use apollo_language_server::{ApolloLanguageServer, Config};
use clap::Parser;
use serde::Serialize;
use tower_lsp::Server;

use crate::federation::supergraph_config::get_supergraph_config;
use crate::federation::{Event, Watcher};
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

        run_lsp(client_config, &self.opts).await?;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig, lsp_opts: &LspOpts) -> RoverResult<()> {
    let initial_config = get_supergraph_config(
        &None,
        lsp_opts.supergraph_yaml.as_ref(),
        None,
        client_config.clone(),
        &lsp_opts.plugin_opts.profile,
    )
    .await?
    .ok_or_else(|| anyhow!("error getting supergraph config"))?;

    let (service, socket, _receiver) = ApolloLanguageServer::build_service(
        Config {
            root_uri: "".into(),
            enable_auto_composition: false,
            force_federation: false,
            disable_telemetry: false,
        },
        initial_config
            .clone()
            .into_iter()
            .map(|(subgraph_name, subgraph_definition)| (subgraph_name, subgraph_definition.schema))
            .collect(),
    );

    let language_server = service.inner().clone();

    tokio::spawn(run_composer_in_thread(
        initial_config,
        lsp_opts.clone(),
        client_config,
        language_server,
    ));

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
    Ok(())
}

async fn run_composer_in_thread(
    initial_config: SupergraphConfig,
    lsp_opts: LspOpts,
    client_config: StudioClientConfig,
    language_server: ApolloLanguageServer,
) -> RoverResult<()> {
    let watcher = Watcher::new(
        initial_config,
        None,
        client_config,
        lsp_opts.plugin_opts.elv2_license_accepter,
        lsp_opts.plugin_opts.skip_update,
        &lsp_opts.plugin_opts.profile,
        1,
    )
    .await?;

    let mut events = watcher.watch().await;

    while let Some(event) = events.recv().await {
        match event {
            Event::SubgraphUpdated { .. } => {
                language_server.composition_did_start().await;
            }
            Event::InitialComposition(output)
            | Event::ComposedAfterSubgraphUpdated { output, .. } => {
                dbg!(&output);
                language_server
                    .composition_did_update(
                        Some(output.supergraph_sdl),
                        output.hints.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            Event::CompositionErrors(build_errors) => {
                dbg!(&build_errors);
                language_server
                    .composition_did_update(
                        None,
                        build_errors.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            Event::CompositionFailed(err) => {
                return Err(err);
            }
        }
    }
    Ok(())
}
