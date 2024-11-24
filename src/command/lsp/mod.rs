use crate::composition::events::CompositionEvent;
use crate::composition::runner::Runner;
use crate::composition::supergraph::binary::OutputTarget;
use crate::composition::supergraph::config::{
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
use futures::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::env::temp_dir;
use std::io::stdin;
use tokio::task::JoinHandle;
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

        run_lsp(client_config, self.opts.clone()).await?;
        Ok(RoverOutput::EmptySuccess)
    }
}

async fn run_lsp(client_config: StudioClientConfig, lsp_opts: LspOpts) -> RoverResult<()> {
    //TODO: Check this error handling is right i.e. if we don't get a supergraph.yaml passed
    // this should fail rather than doing something else.
    let supergraph_yaml_path = lsp_opts
        .supergraph_yaml
        .as_ref()
        .and_then(|path| {
            if path.is_relative() {
                Some(
                    Utf8PathBuf::try_from(std::env::current_dir().ok()?)
                        .ok()?
                        .join(path),
                )
            } else {
                Some(path.clone())
            }
        })
        .ok_or_else(|| anyhow!("Could not find supergraph.yaml file."))?;

    let studio_client = client_config.get_authenticated_client(&lsp_opts.plugin_opts.profile)?;

    // Get the SupergraphConfig in a form we can use
    let supergraph_config = SupergraphConfigResolver::default()
        .load_remote_subgraphs(&studio_client, None)
        .await?
        .load_from_file_descriptor(
            &mut stdin(),
            Some(&FileDescriptorType::File(supergraph_yaml_path.clone())),
        )?;
    let lazily_resolved_supergraph_config = supergraph_config
        .lazily_resolve_subgraphs(&supergraph_yaml_path)
        .await?;

    let root_uri = supergraph_yaml_path
        .parent()
        .map(|path| path.to_string())
        .unwrap_or_default();

    // Build the service to spin up the language server
    let (service, socket, _receiver) = ApolloLanguageServer::build_service(
        Config {
            root_uri,
            enable_auto_composition: false,
            force_federation: false,
            disable_telemetry: false,
        },
        HashMap::from_iter(
            lazily_resolved_supergraph_config
                .subgraphs()
                .into_iter()
                .map(|(a, b)| (a.to_string(), b.schema().clone())),
        ),
    );

    let language_server = service.inner().clone();

    // Spawn a separate thread to handle composition and passing that data to the language server
    let _: JoinHandle<Result<(), Error>> = tokio::spawn(async move {
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
                    .fully_resolve_subgraphs(&client_config, &studio_client)
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

        let supergraph_yaml_url =
            tower_lsp::lsp_types::Url::from_file_path(supergraph_yaml_path)
                .map_err(|_| anyhow!("Failed to convert supergraph yaml path to url"))?;

        while let Some(event) = stream.next().await {
            match event {
                CompositionEvent::Started => {
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
                    // TODO: we could highlight the version of federation, since it failed.
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
        Ok(())
    });

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let server = Server::new(stdin, stdout, socket);
    server.serve(service).await;
    Ok(())
}
