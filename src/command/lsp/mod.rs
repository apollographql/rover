use crate::federation::supergraph_config::{get_supergraph_config, HybridSupergraphConfig};
use crate::federation::{Event, Watcher};
use crate::{
    options::PluginOpts,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
    RoverOutput, RoverResult,
};
use anyhow::anyhow;
use apollo_language_server::{ApolloLanguageServer, Config};
use camino::Utf8PathBuf;
use clap::Parser;
use serde::Serialize;
use tower_lsp::lsp_types::{Diagnostic, Range};
use tower_lsp::Server;
use tracing::debug;

#[derive(Debug, Serialize, Parser)]
pub struct Lsp {
    #[clap(flatten)]
    pub(crate) opts: LspOpts,
}

#[derive(Clone, Debug, Serialize, Parser)]
pub struct LspOpts {
    #[clap(flatten)]
    pub plugin_opts: PluginOpts,

    /// The absolute path to the supergraph configuration file.
    #[serde(skip_serializing)]
    #[arg(long = "supergraph-config")]
    supergraph_yaml: Option<Utf8PathBuf>,
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
    let supergraph_yaml_path = lsp_opts.supergraph_yaml.as_ref().and_then(|path| {
        if path.is_relative() {
            Some(
                Utf8PathBuf::try_from(std::env::current_dir().ok()?)
                    .ok()?
                    .join(path),
            )
        } else {
            Some(path.clone())
        }
    });
    let root_uri = supergraph_yaml_path
        .as_ref()
        .and_then(|path| path.parent())
        .map(|path| path.to_string())
        .unwrap_or_default();
    let supergraph_config = if let Some(supergraph_yaml) = supergraph_yaml_path.as_ref() {
        let initial_config = get_supergraph_config(
            &None,
            Some(&FileDescriptorType::File(supergraph_yaml.clone())),
            None,
            client_config.clone(),
            &lsp_opts.plugin_opts.profile,
        )
        /* TODO: don't fail on startup for this, somehow.
           Instead start watching the files like normal and report diagnostics
           Today, if this fails at startup, the users have no recourse
        */
        .await?
        .ok_or_else(|| anyhow!("error getting supergraph config"))?;
        Some((supergraph_yaml.clone(), initial_config))
    } else {
        None
    };

    let (service, socket, _receiver) = ApolloLanguageServer::build_service(
        Config {
            root_uri,
            enable_auto_composition: false,
            force_federation: false,
            disable_telemetry: false,
        },
        supergraph_config
            .as_ref()
            .map(|(_, config)| config.merged_config.clone())
            .into_iter()
            .flatten()
            .map(|(subgraph_name, subgraph_definition)| (subgraph_name, subgraph_definition.schema))
            .collect(),
    );

    let language_server = service.inner().clone();

    if let Some(supergraph_config) = supergraph_config {
        tokio::spawn(run_composer_in_thread(
            supergraph_config.0,
            supergraph_config.1,
            lsp_opts.clone(),
            client_config,
            language_server,
        ));
    }

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
    Ok(())
}

async fn run_composer_in_thread(
    supergraph_yaml_path: Utf8PathBuf,
    initial_config: HybridSupergraphConfig,
    lsp_opts: LspOpts,
    client_config: StudioClientConfig,
    language_server: ApolloLanguageServer,
) -> RoverResult<()> {
    let supergraph_yaml_url = tower_lsp::lsp_types::Url::from_file_path(supergraph_yaml_path)
        .map_err(|_| anyhow!("Failed to convert supergraph yaml path to url"))?;
    let watcher = Watcher::new(
        initial_config,
        None,
        client_config,
        lsp_opts.plugin_opts.elv2_license_accepter,
        lsp_opts.plugin_opts.skip_update,
        lsp_opts.plugin_opts.profile,
        1,
    )
    .await?;

    let mut events = watcher.watch().await;

    while let Some(event) = events.recv().await {
        match event {
            Event::SubgraphUpdated { subgraph_name } => {
                debug!("Subgraph {} updated", subgraph_name);
                language_server.composition_did_start().await;
            }
            Event::CompositionSucceeded {
                output,
                federation_version,
                ..
            } => {
                debug!("Successfully composed with version {}", federation_version);
                // Clear any previous errors on `supergraph.yaml`
                language_server
                    .publish_diagnostics(supergraph_yaml_url.clone(), vec![])
                    .await;
                language_server
                    .composition_did_update(
                        Some(output.supergraph_sdl),
                        output.hints.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await;
            }
            Event::CompositionErrors {
                errors,
                federation_version,
            } => {
                debug!(
                    ?errors,
                    "Composition {federation_version} completed with errors"
                );
                // Clear any previous errors on `supergraph.yaml`
                language_server
                    .publish_diagnostics(supergraph_yaml_url.clone(), vec![])
                    .await;
                language_server
                    .composition_did_update(
                        None,
                        errors.into_iter().map(Into::into).collect(),
                        None,
                    )
                    .await
            }
            Event::CompositionFailed {
                err,
                federation_version,
            } => {
                debug!("Composition {federation_version} failed: {err}");
                // TODO: we could highlight the version of federation, since it failed.
                let message = format!(
                    "Failed run composition {federation_version}: {err}",
                    err = err.message()
                );
                let diagnostic = Diagnostic::new_simple(Range::default(), message);
                language_server
                    .publish_diagnostics(supergraph_yaml_url.clone(), vec![diagnostic])
                    .await;
            }
            Event::SubgraphAdded {
                subgraph_name,
                schema_source,
            } => {
                debug!("Subgraph {} added", subgraph_name);
                language_server
                    .add_subgraph(subgraph_name, schema_source)
                    .await;
            }
            Event::SubgraphRemoved { subgraph_name } => {
                debug!("Subgraph {} removed", subgraph_name);
                language_server.remove_subgraph(&subgraph_name).await;
            }
            Event::StartedWatchingSubgraph(watcher) => {
                debug!("Started watching subgraph {watcher:?}"); // TODO: hand off between real-time and on-save
            }
        }
    }
    Ok(())
}
