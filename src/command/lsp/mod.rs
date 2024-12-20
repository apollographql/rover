mod errors;

use std::collections::HashMap;
use std::env::temp_dir;
use std::io::stdin;

use apollo_federation_types::config::FederationVersion;
use apollo_language_server::{ApolloLanguageServer, Config, MaxSpecVersions};
use camino::Utf8PathBuf;
use clap::Parser;
use futures::stream::BoxStream;
use futures::StreamExt;
use serde::Serialize;
use tower::ServiceExt;
use tower_lsp::lsp_types::{Diagnostic, Range};
use tower_lsp::Server;
use tracing::debug;
use url::Url;

use crate::command::lsp::errors::StartCompositionError;
use crate::command::lsp::errors::StartCompositionError::SupergraphYamlUrlConversionFailed;
use crate::composition::events::CompositionEvent;
use crate::composition::pipeline::CompositionPipeline;
use crate::composition::supergraph::binary::OutputTarget;
use crate::composition::supergraph::config::full::introspect::MakeResolveIntrospectSubgraph;
use crate::composition::supergraph::config::lazy::LazilyResolvedSupergraphConfig;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraph::MakeFetchRemoteSubgraph;
use crate::composition::supergraph::config::resolver::fetch_remote_subgraphs::MakeFetchRemoteSubgraphs;
use crate::composition::supergraph::config::resolver::{SubgraphPrompt, SupergraphConfigResolver};
use crate::composition::SupergraphConfigResolutionError::PathDoesNotPointToAFile;
use crate::composition::{
    CompositionError, CompositionSubgraphAdded, CompositionSubgraphRemoved, CompositionSuccess,
    SupergraphConfigResolutionError,
};
use crate::options::ProfileOpt;
use crate::utils::effect::exec::TokioCommand;
use crate::utils::effect::read_file::FsReadFile;
use crate::utils::effect::write_file::FsWriteFile;
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

    /// The absolute path to the supergraph configuration file.
    #[serde(skip_serializing)]
    #[arg(long = "supergraph-config")]
    supergraph_yaml: Option<Utf8PathBuf>,
}

impl Lsp {
    pub async fn run(&self, client_config: StudioClientConfig) -> RoverResult<RoverOutput> {
        self.opts
            .plugin_opts
            .elv2_license_accepter
            .require_elv2_license(&client_config)?;

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
                    max_spec_versions: MaxSpecVersions {
                        connect: None,
                        federation: None,
                    },
                },
                HashMap::new(),
            );
            (service, socket)
        }
        Some(supergraph_yaml_path) => {
            // Resolve Supergraph Config -> Lazy
            let lazily_resolved_supergraph_config = generate_lazily_resolved_supergraph_config(
                supergraph_yaml_path.clone(),
                client_config.clone(),
                lsp_opts.plugin_opts.profile.clone(),
            )
            .await?;
            let supergraph_yaml_url = Url::from_file_path(supergraph_yaml_path.clone())
                .map_err(|_| SupergraphYamlUrlConversionFailed(supergraph_yaml_path.clone()))?;
            debug!("Supergraph Config Root: {:?}", supergraph_yaml_url);
            // Generate the config needed to spin up the Language Server
            let (service, socket, _receiver) = ApolloLanguageServer::build_service(
                Config {
                    root_uri: String::from(supergraph_yaml_url.clone()),
                    enable_auto_composition: false,
                    force_federation: false,
                    disable_telemetry: false,
                    max_spec_versions: MaxSpecVersions {
                        connect: None,
                        federation: None,
                    },
                },
                HashMap::from_iter(
                    lazily_resolved_supergraph_config
                        .subgraphs()
                        .iter()
                        .map(|(a, b)| (a.to_string(), b.schema().clone())),
                ),
            );
            // Start running composition
            start_composition(
                supergraph_yaml_path,
                supergraph_yaml_url,
                lazily_resolved_supergraph_config
                    .federation_version()
                    .clone(),
                client_config,
                lsp_opts,
                service.inner().to_owned(),
            )
            .await?;
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
    supergraph_yaml_path: Utf8PathBuf,
    client_config: StudioClientConfig,
    profile: ProfileOpt,
) -> Result<LazilyResolvedSupergraphConfig, SupergraphConfigResolutionError> {
    let make_fetch_remote_subgraphs = MakeFetchRemoteSubgraphs::builder()
        .studio_client_config(client_config)
        .profile(profile)
        .build();
    // Get the SupergraphConfig in a form we can use
    let supergraph_config = SupergraphConfigResolver::default()
        .load_remote_subgraphs(make_fetch_remote_subgraphs, None)
        .await?
        .load_from_file_descriptor(
            &mut stdin(),
            Some(&FileDescriptorType::File(supergraph_yaml_path.clone())),
        )?;
    if let Some(parent) = supergraph_yaml_path.parent() {
        Ok(supergraph_config
            .lazily_resolve_subgraphs(&parent.to_owned(), &SubgraphPrompt::default())
            .await?)
    } else {
        Err(PathDoesNotPointToAFile(
            supergraph_yaml_path.into_std_path_buf(),
        ))
    }
}

async fn start_composition(
    supergraph_config_path: Utf8PathBuf,
    supergraph_yaml_url: Url,
    federation_version: Option<FederationVersion>,
    client_config: StudioClientConfig,
    lsp_opts: LspOpts,
    language_server: ApolloLanguageServer,
) -> Result<(), StartCompositionError> {
    let federation_version = federation_version.clone();
    let mut stream = match create_composition_stream(
        supergraph_config_path,
        federation_version,
        client_config,
        lsp_opts,
    )
    .await
    {
        Ok(stream) => stream,
        Err(e) => {
            let message = format!("Could not initialise composition process: {e}");
            let diagnostic = Diagnostic::new_simple(Range::default(), message);
            language_server
                .publish_diagnostics(supergraph_yaml_url.clone(), vec![diagnostic])
                .await;
            return Err(e);
        }
    };

    // Spawn a separate thread to handle composition and passing that data to the language server
    tokio::spawn(async move {
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
                CompositionEvent::Error(CompositionError::Build {
                    source: errors,
                    federation_version,
                }) => {
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
                    debug!("Composition failed: {err}");
                    let message = format!("Failed run composition: {err}",);
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
        Ok::<(), StartCompositionError>(())
    });
    Ok(())
}

async fn create_composition_stream(
    supergraph_config_path: Utf8PathBuf,
    federation_version: Option<FederationVersion>,
    client_config: StudioClientConfig,
    lsp_opts: LspOpts,
) -> Result<BoxStream<'static, CompositionEvent>, StartCompositionError> {
    let fetch_remote_subgraphs_factory = MakeFetchRemoteSubgraphs::builder()
        .studio_client_config(client_config.clone())
        .profile(lsp_opts.plugin_opts.profile.clone())
        .build();
    let fetch_remote_subgraph_factory = MakeFetchRemoteSubgraph::builder()
        .studio_client_config(client_config.clone())
        .profile(lsp_opts.plugin_opts.profile.clone())
        .build()
        .boxed_clone();
    let resolve_introspect_subgraph_factory =
        MakeResolveIntrospectSubgraph::new(client_config.service()?).boxed_clone();

    let composition_pipeline = CompositionPipeline::default()
        .init(
            &mut stdin(),
            fetch_remote_subgraphs_factory,
            Some(FileDescriptorType::File(supergraph_config_path.clone())),
            None,
        )
        .await?
        .resolve_federation_version(
            resolve_introspect_subgraph_factory.clone(),
            fetch_remote_subgraph_factory.clone(),
            federation_version,
        )
        .await?
        .install_supergraph_binary(
            client_config.clone(),
            None,
            lsp_opts.plugin_opts.elv2_license_accepter,
            lsp_opts.plugin_opts.skip_update,
        )
        .await?;

    // Spin up Runner
    Ok(composition_pipeline
        .runner(
            TokioCommand::default(),
            FsReadFile::default(),
            FsWriteFile::default(),
            client_config.service()?,
            fetch_remote_subgraph_factory.boxed_clone(),
            500,
            Utf8PathBuf::try_from(temp_dir())?,
            OutputTarget::InMemory,
            true,
        )
        .await?
        .run())
}
