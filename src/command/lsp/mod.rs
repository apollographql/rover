use futures::StreamExt;
mod errors;

use crate::command::lsp::errors::SupergraphConfigLazyResolutionError;
use crate::command::lsp::errors::SupergraphConfigLazyResolutionError::PathDoesNotPointToAFile;
use crate::composition::events::CompositionEvent;
use crate::composition::runner::Runner;
use crate::composition::supergraph::binary::OutputTarget;
use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::composition::supergraph::config::resolver::{
    ResolveSupergraphConfigError, SupergraphConfigResolver,
};
use crate::composition::supergraph::install::InstallSupergraph;
use crate::composition::{
    CompositionError, CompositionSubgraphAdded, CompositionSubgraphRemoved, CompositionSuccess,
};
use crate::utils::effect::exec::TokioCommand;
use crate::utils::effect::install::InstallBinary;
use crate::utils::effect::read_file::FsReadFile;
use crate::utils::effect::write_file::FsWriteFile;
use crate::{
    options::PluginOpts,
    utils::{client::StudioClientConfig, parsers::FileDescriptorType},
    RoverOutput, RoverResult,
};
use anyhow::{anyhow, Error};
use apollo_federation_types::config::FederationVersion;
use apollo_language_server::{ApolloLanguageServer, Config};
use camino::Utf8PathBuf;
use clap::Parser;
use rover_client::blocking::StudioClient;
use serde::Serialize;
use std::collections::HashMap;
use std::env::temp_dir;
use std::io::stdin;
use tower_lsp::lsp_types::{Diagnostic, Range};
use tower_lsp::Server;
use tracing::debug;
use url::Url;

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

        run_lsp(client_config, self.opts.clone()).await?;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig, lsp_opts: LspOpts) -> RoverResult<()> {
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

    // Early return if there is no `supergraph.yaml` given as there is no further need to construct
    // anything
    let (service, socket) = match supergraph_yaml_path {
        None => {
            let (service, socket, _receiver) = ApolloLanguageServer::build_service(
                Config {
                    // TODO Do we need to worry about these now?
                    root_uri: String::default(),
                    enable_auto_composition: false,
                    force_federation: false,
                    disable_telemetry: false,
                },
                HashMap::new(),
            );
            (service, socket)
        }
        Some(supergraph_yaml_path) => {
            let studio_client =
                client_config.get_authenticated_client(&lsp_opts.plugin_opts.profile)?;
            // Resolve Supergraph Config -> Lazy
            let (lazily_resolved_supergraph_config, supergraph_content_root) =
                generate_lazily_resolved_supergraph_config(
                    &studio_client,
                    supergraph_yaml_path.clone(),
                )
                .await?;
            // Generate the config needed to spin up the Language Server
            let (service, socket, _receiver) = ApolloLanguageServer::build_service(
                Config {
                    root_uri: supergraph_content_root,
                    enable_auto_composition: false,
                    force_federation: false,
                    disable_telemetry: false,
                },
                HashMap::from_iter(
                    lazily_resolved_supergraph_config
                        .subgraphs()
                        .iter()
                        .map(|(a, b)| (a.to_string(), b.schema().clone())),
                ),
            );
            let supergraph_yaml_url = Url::from_file_path(supergraph_yaml_path)
                .map_err(|_| anyhow!("Failed to convert supergraph yaml path to url"))?;
            // Start running composition
            start_composition(
                lazily_resolved_supergraph_config,
                supergraph_yaml_url,
                client_config,
                studio_client,
                lsp_opts,
                service.inner().to_owned(),
            )
            .await;
            (service, socket)
        }
    };

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
    Ok(())
}

async fn generate_lazily_resolved_supergraph_config(
    studio_client: &StudioClient,
    supergraph_yaml_path: Utf8PathBuf,
) -> Result<(LazilyResolvedSupergraphConfig, String), SupergraphConfigLazyResolutionError> {
    // Get the SupergraphConfig in a form we can use
    let supergraph_config = SupergraphConfigResolver::default()
        .load_remote_subgraphs(studio_client, None)
        .await?
        .load_from_file_descriptor(
            &mut stdin(),
            Some(&FileDescriptorType::File(supergraph_yaml_path.clone())),
        )?;
    if let Some(parent) = supergraph_yaml_path.parent() {
        Ok((
            supergraph_config
                .lazily_resolve_subgraphs(&parent.to_owned())
                .await?,
            parent.to_string(),
        ))
    } else {
        Err(PathDoesNotPointToAFile(
            supergraph_yaml_path.into_std_path_buf(),
        ))
    }
}

async fn start_composition(
    lazily_resolved_supergraph_config: LazilyResolvedSupergraphConfig,
    supergraph_yaml_url: Url,
    client_config: StudioClientConfig,
    studio_client: StudioClient,
    lsp_opts: LspOpts,
    language_server: ApolloLanguageServer,
) {
    // Spawn a separate thread to handle composition and passing that data to the language server
    tokio::spawn(async move {
        // Create a supergraph binary
        // TODO: Check defaulting behaviour here and see if we need to centralise
        let federation_version = lazily_resolved_supergraph_config
            .federation_version()
            .clone()
            .unwrap_or(FederationVersion::LatestFedTwo);

        // TODO: Let the supergraph binary exist inside its own task that can respond to being re-installed etc.
        let supergraph_binary =
            InstallSupergraph::new(federation_version.clone(), client_config.clone())
                .install(
                    None,
                    lsp_opts.plugin_opts.elv2_license_accepter,
                    lsp_opts.plugin_opts.skip_update,
                )
                .await?;

        // Spin up Runner
        let mut stream = Runner::default()
            .setup_subgraph_watchers(
                lazily_resolved_supergraph_config.subgraphs().clone(),
                &lsp_opts.plugin_opts.profile,
                &client_config,
                500,
            )
            .setup_supergraph_config_watcher(lazily_resolved_supergraph_config.clone())
            .setup_composition_watcher(
                lazily_resolved_supergraph_config
                    .extract_subgraphs_as_sdls(&client_config, &studio_client)
                    .await
                    .map_err(ResolveSupergraphConfigError::ResolveSubgraphs)?,
                supergraph_binary,
                TokioCommand::default(),
                FsReadFile::default(),
                FsWriteFile::default(),
                OutputTarget::Stdout,
                Utf8PathBuf::try_from(temp_dir())?,
            )
            .run();

        while let Some(event) = stream.next().await {
            match event {
                CompositionEvent::Started => {
                    // Even though it's hidden by library calls, this function emits a WorkDoneProgressBegin event,
                    // which is paired with a WorkDoneProgressEnd event, sent by the `composition_did_update` function.
                    // Any refactoring needs to ensure that we don't break this ordering, otherwise the LSP may well
                    // cease to function in a useful way.
                    language_server.composition_did_start().await;
                }
                CompositionEvent::Success(CompositionSuccess {
                    supergraph_sdl,
                    federation_version,
                    hints,
                }) => {
                    debug!("Successfully composed with version {}", federation_version);
                    // Clear any previous errors on `supergraph.yaml`
                    language_server
                        .publish_diagnostics(supergraph_yaml_url.clone(), vec![])
                        .await;
                    language_server
                        .composition_did_update(
                            Some(supergraph_sdl),
                            hints.into_iter().map(Into::into).collect(),
                            None,
                        )
                        .await;
                }
                CompositionEvent::Error(CompositionError::Build { source: errors }) => {
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
                CompositionEvent::Error(err) => {
                    debug!("Composition {federation_version} failed: {err}");
                    let message = format!("Failed run composition {federation_version}: {err}",);
                    let diagnostic = Diagnostic::new_simple(Range::default(), message);
                    language_server
                        .publish_diagnostics(supergraph_yaml_url.clone(), vec![diagnostic])
                        .await;
                }
                CompositionEvent::SubgraphAdded(CompositionSubgraphAdded {
                    name,
                    schema_source,
                }) => {
                    debug!("Subgraph {} added", name);
                    language_server.add_subgraph(name, schema_source).await;
                }
                CompositionEvent::SubgraphRemoved(CompositionSubgraphRemoved { name }) => {
                    debug!("Subgraph {} removed", name);
                    language_server.remove_subgraph(&name).await;
                }
            }
        }
        Ok::<(), Error>(())
    });
}
