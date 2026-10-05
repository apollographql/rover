use std::{io::stdin, str::FromStr};

use anyhow::anyhow;
use apollo_federation_types::config::FederationVersion;
use camino::Utf8PathBuf;
use futures::StreamExt;
use rover_client::RoverClientError;
use rover_print::{print::Print, style::StyledText};
use rover_std::{errln, infoln};
use semver::Version;
use timber::Level;
use tower::ServiceExt;

use crate::{
    RoverError, RoverOutput, RoverResult,
    command::{
        Dev,
        dev::{
            mcp::{binary::RunMcpServerBinaryError, run::RunMcpServer},
            router::{
                binary::RunRouterBinaryError,
                config::{RouterAddress, RouterHost, RouterPort},
                run::RunRouter,
            },
        },
        install::Plugin,
    },
    composition::{
        CompositionError, FederationUpdaterConfig,
        events::CompositionEvent,
        pipeline::CompositionPipeline,
        supergraph::config::{
            full::introspect::MakeResolveIntrospectSubgraph,
            resolver::{
                DefaultSubgraphDefinition, SubgraphPrompt,
                fetch_remote_subgraph::MakeFetchRemoteSubgraph,
                fetch_remote_subgraphs::MakeFetchRemoteSubgraphs,
            },
        },
    },
    options::ProfileOpt,
    plugin::{
        discovery::ManifestDirs,
        error::RequestOrigin,
        precedence::{self, RequestInputs, declarations_in_scope},
        version::{PluginName, VersionRequest},
    },
    utils::{
        client::StudioClientConfig,
        effect::{
            exec::{TokioCommand, TokioSpawn},
            read_file::FsReadFile,
            write_file::FsWriteFile,
        },
        env::RoverEnvKey,
    },
};

impl Dev {
    /// Runs rover dev
    pub async fn run(
        &self,
        override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
        log_level: Option<Level>,
        profile: &ProfileOpt,
        stderr: &impl Print,
        graph_ref_setting: Option<String>,
    ) -> RoverResult<RoverOutput> {
        // `.env` is loaded by the caller (`cli.rs`'s `Command::Dev` dispatch),
        // before `graph_ref_setting` above was resolved - not here, which
        // would be too late for a `.env`-supplied `APOLLO_GRAPH_REF` to be
        // seen by that resolution.
        let elv2_license_accepter = self.opts.plugin_opts.elv2_license_accepter;
        let skip_update = self.opts.plugin_opts.skip_update;
        let read_file_impl = FsReadFile::default();
        let write_file_impl = FsWriteFile::default();
        let exec_command_impl = TokioCommand::default();

        let tmp_dir = tempfile::Builder::new().prefix("supergraph").tempdir()?;
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.keep())?;

        let router_config_path = self.opts.supergraph_opts.router_config_path.clone();

        let graph_ref = &self.opts.supergraph_opts.graph_ref;
        if let Some(graph_ref) = graph_ref {
            eprintln!("retrieving subgraphs remotely from {graph_ref}")
        }
        let supergraph_config_path = &self.opts.supergraph_opts.clone().supergraph_config_path;

        let fetch_remote_subgraphs_factory = MakeFetchRemoteSubgraphs::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build();
        let fetch_remote_subgraph_factory = MakeFetchRemoteSubgraph::builder()
            .studio_client_config(client_config.clone())
            .profile(profile.clone())
            .build()
            .boxed_clone();
        let resolve_introspect_subgraph_factory =
            MakeResolveIntrospectSubgraph::new(client_config.service()?).boxed_clone();

        let plugin_levels = ManifestDirs::in_scope(override_install_path.as_deref());
        let federation_version = self.federation_version_override();

        let subgraph_definition = self
            .opts
            .subgraph_opts
            .subgraph_name
            .as_ref()
            .and_then(|subgraph_name| {
                self.opts
                    .subgraph_opts
                    .subgraph_url
                    .as_ref()
                    .map(|subgraph_url| DefaultSubgraphDefinition::Args {
                        name: subgraph_name.to_string(),
                        url: subgraph_url.clone(),
                        schema_path: self.opts.subgraph_opts.subgraph_schema_path.clone(),
                    })
            })
            .unwrap_or_else(|| {
                DefaultSubgraphDefinition::Prompt(Box::new(SubgraphPrompt::default()))
            });
        let composition_pipeline = CompositionPipeline::default()
            .init(
                &mut stdin(),
                fetch_remote_subgraphs_factory,
                supergraph_config_path.clone(),
                graph_ref.clone(),
                Some(subgraph_definition),
            )
            .await?
            .resolve_federation_version(
                resolve_introspect_subgraph_factory.clone(),
                fetch_remote_subgraph_factory.clone(),
                federation_version.clone(),
                &plugin_levels,
                false,
            )
            .await?
            .install_supergraph_binary(
                client_config.clone(),
                override_install_path.clone(),
                elv2_license_accepter,
                skip_update,
            )
            .await?;

        // The chain above only succeeds once the supergraph binary is resolved, so this is
        // always `Ok` here (FR54).
        if let Ok(binary) = &composition_pipeline.state.supergraph_binary {
            stderr.print(&StyledText::plain(binary.provenance().to_string()));
        }

        let (router_version, router_version_origin) = match dev_plugin_request(
            PluginName::Router,
            self.router_version_override()?,
            VersionRequest::Major(2),
            &plugin_levels,
        )? {
            (Plugin::Router(version), origin) => (version, origin),
            _ => unreachable!("a `router` request converts to a router version"),
        };

        let api_key_override = std::env::var(RoverEnvKey::Key.to_string()).ok();
        let home_override = std::env::var(RoverEnvKey::Home.to_string()).ok();

        // Set up an updater config, but only if we're not overriding the version ourselves: a
        // flag or its environment variable outranks `supergraph.yaml`, so a mid-session change
        // there must not replace it.
        let federation_updater_config = match federation_version {
            Some(_) => None,
            None => Some(FederationUpdaterConfig {
                studio_client_config: client_config.clone(),
                elv2_licence_accepter: elv2_license_accepter,
                skip_update,
            }),
        };

        let composition_runner = composition_pipeline
            .runner(
                exec_command_impl,
                write_file_impl.clone(),
                client_config.service()?,
                fetch_remote_subgraph_factory.boxed_clone(),
                self.opts.subgraph_opts.subgraph_polling_interval,
                tmp_config_dir_path.clone(),
                true,
                federation_updater_config,
            )
            .await?;

        let mut composition_messages = composition_runner.run();

        // Sit in a loop and wait for the composition to actually succeed, once it does then
        // we can progress
        let supergraph_schema;
        loop {
            match composition_messages.next().await {
                Some(CompositionEvent::Started) => {
                    if let Ok(ref binary) = composition_pipeline.state.supergraph_binary {
                        eprintln!("composing supergraph with Federation {}", binary.version());
                    }
                }
                Some(CompositionEvent::Success(success)) => {
                    supergraph_schema = success.supergraph_sdl;
                    break;
                }
                Some(CompositionEvent::Error(CompositionError::Build { source, .. })) => {
                    let number_of_subgraphs = source.len();
                    let error_to_output = RoverError::from(RoverClientError::BuildErrors {
                        source,
                        num_subgraphs: number_of_subgraphs,
                    });
                    eprintln!("{error_to_output}")
                }
                Some(CompositionEvent::Error(err)) => {
                    errln!(
                        "Error occurred when composing supergraph\n{}",
                        rover_std::format_error_chain(&err)
                    )
                }
                Some(_) => {}
                None => {
                    return Err(RoverError::new(anyhow!(
                        "Composition Events Stream closed before supergraph schema could successfully compose"
                    )));
                }
            }
        }

        // This RouterAddress hasn't been fully processed. It only represents the CLI option or
        // default, but we still have to reckon with the config-set address (if one exists). See
        // the reassignment of the variable below for details
        let router_address = RouterAddress::new(
            self.opts
                .supergraph_opts
                .supergraph_address
                .map(RouterHost::CliOption),
            self.opts
                .supergraph_opts
                .supergraph_port
                .map(RouterPort::CliOption),
        );

        let run_router = RunRouter::default()
            .install(
                router_version,
                router_version_origin,
                client_config.clone(),
                override_install_path.clone(),
                elv2_license_accepter,
                skip_update,
            )
            .await?;
        stderr.print(&StyledText::plain(
            run_router.state.binary.provenance().to_string(),
        ));
        let run_router = run_router
            .load_config(&read_file_impl, router_address, router_config_path)
            .await?
            .load_remote_config(
                client_config.clone(),
                profile.clone(),
                graph_ref.clone(),
                home_override.clone(),
                api_key_override.clone(),
            )
            .await;
        // This RouterAddress has some logic figuring out _which_ of the potentially multiple
        // address options we should use (eg, CLI, config, env var, or default). It will be used in
        // the cli arguments for the router, but also as a message to the user for
        // where to find their router
        let router_address = *run_router.state.config.address();
        // Extract the router's listen path from the config to construct the full endpoint URL for MCP
        let router_url_path = run_router.state.config.listen_path();
        infoln!(
            "Attempting to start router at {}.",
            router_address.pretty_string()
        );

        let supergraph_output = self.opts.supergraph_opts.supergraph_output.clone();
        if let Some(ref path) = supergraph_output {
            infoln!(
                "writing the composed supergraph schema to {path} (updated on every recomposition)"
            );
        }

        let mut run_router = run_router
            .run(
                FsWriteFile::default(),
                TokioSpawn::default(),
                &tmp_config_dir_path,
                client_config.clone(),
                &supergraph_schema,
                profile.clone(),
                home_override,
                api_key_override,
                log_level,
                supergraph_output,
                self.opts.supergraph_opts.license.clone(),
                graph_ref_setting,
            )
            .await?
            .watch_for_changes(write_file_impl, composition_messages)
            .await;

        if let Some(ref config) = self.opts.mcp.config {
            let (mcp_version, mcp_version_origin) = match dev_plugin_request(
                PluginName::ApolloMcpServer,
                self.mcp_version_override(),
                VersionRequest::Latest,
                &plugin_levels,
            )? {
                (Plugin::McpServer(version), origin) => (version, origin),
                _ => unreachable!("an `apollo-mcp-server` request converts to its version"),
            };

            let run_mcp_server = RunMcpServer::default()
                .install(
                    mcp_version,
                    mcp_version_origin,
                    client_config.clone(),
                    override_install_path,
                    elv2_license_accepter,
                    skip_update,
                )
                .await?;
            stderr.print(&StyledText::plain(
                run_mcp_server.state.binary.provenance().to_string(),
            ));

            let mut run_mcp_server = run_mcp_server
                .run(
                    TokioSpawn::default(),
                    run_router.state.hot_reload_schema_path.clone(),
                    router_address,
                    router_url_path,
                    config.clone(),
                    run_router.state.env.clone(),
                )
                .await?;

            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        eprintln!("\nreceived shutdown signal, stopping `rover dev` processes...");

                        // Note that these calls aren't strictly necessary. The OS will send the
                        // SIGINT signal to forked child processes, so they would exit anyway.
                        run_router.shutdown();
                        run_mcp_server.shutdown();
                        break
                    },

                    Some(router_log) = run_router.router_logs().next() => {
                        match router_log {
                            Ok(router_log) => {
                                if !router_log.to_string().is_empty() {
                                    eprintln!("{router_log}");
                                }
                            }
                            Err(RunRouterBinaryError::BinaryExited(res)) => {
                                match res {
                                    Ok(status) => {
                                        match status.code() {
                                            None => {
                                                eprintln!("Router process terminal by signal");
                                            }
                                            Some(code) => {
                                                eprintln!("Router process exited with status code: {code}");
                                            }
                                        }

                                    }
                                    Err(err) => {
                                        tracing::error!("Router process exited without status code. Error: {err}")
                                    }
                                }
                                eprintln!("\nRouter binary exited, stopping `rover dev` processes...");
                                run_mcp_server.shutdown();
                                break;
                            }
                            Err(err) => {
                                tracing::error!("{:?}", err);
                            }
                        }
                    },

                    Some(mcp_server_logs) = run_mcp_server.mcp_server_logs().next() => {
                        match mcp_server_logs {
                            Ok(mcp_server_logs) => {
                                if !mcp_server_logs.to_string().is_empty() {
                                    eprintln!("{mcp_server_logs}");
                                }
                            }
                            Err(RunMcpServerBinaryError::BinaryExited(res)) => {
                                match res {
                                    Ok(status) => {
                                        match status.code() {
                                            None => {
                                                eprintln!("MCP Server process terminal by signal");
                                            }
                                            Some(code) => {
                                                eprintln!("MCP Server process exited with status code: {code}");
                                            }
                                        }

                                    }
                                    Err(err) => {
                                        tracing::error!("MCP Server process exited without status code. Error: {err}")
                                    }
                                }
                                eprintln!("\nMCP Server binary exited, stopping `rover dev` processes...");
                                run_router.shutdown();
                                break;
                            }
                            Err(err) => {
                                tracing::error!("{:?}", err);
                            }
                        }
                    },

                    else => break,
                }
            }
        } else {
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        eprintln!("\nreceived shutdown signal, stopping `rover dev` processes...");
                        run_router.shutdown();
                        break
                    },
                    Some(router_log) = run_router.router_logs().next() => {
                        match router_log {
                            Ok(router_log) => {
                                if !router_log.to_string().is_empty() {
                                    eprintln!("{router_log}");
                                }
                            }
                            Err(RunRouterBinaryError::BinaryExited(res)) => {
                                match res {
                                    Ok(status) => {
                                        match status.code() {
                                            None => {
                                                eprintln!("Router process terminal by signal");
                                            }
                                            Some(code) => {
                                                eprintln!("Router process exited with status code: {code}");
                                            }
                                        }

                                    }
                                    Err(err) => {
                                        tracing::error!("Router process exited without status code. Error: {err}")
                                    }
                                }
                                eprintln!("\nRouter binary exited, stopping `rover dev` processes...");
                                break;
                            }
                            Err(err) => {
                                tracing::error!("{:?}", err);
                            }
                        }
                    },
                    else => break,
                }
            }
        };
        Ok(RoverOutput::EmptySuccess)
    }
}

impl Dev {
    /// The `supergraph` version `rover dev` was given, and where:
    /// `--federation-version`, then `--composition-version` or its
    /// environment variable. Where that ranks against `supergraph.yaml` and
    /// the manifests is shared with every other plugin-using command.
    fn federation_version_override(&self) -> Option<(FederationVersion, RequestOrigin)> {
        self.opts
            .supergraph_opts
            .federation_version
            .clone()
            .map(|version| (version, RequestOrigin::Flag("--federation-version")))
            .or_else(|| {
                let version = &self
                    .opts
                    .supergraph_opts
                    .composition_version
                    .clone()
                    .and_then(|version| {
                        match FederationVersion::from_str(&format!("={version}")) {
                            Ok(version) => Some((
                                version,
                                RequestOrigin::flag_or_env(
                                    "--composition-version",
                                    "APOLLO_ROVER_DEV_COMPOSITION_VERSION",
                                ),
                            )),
                            Err(err) => {
                                errln!("{err}");
                                tracing::error!("{:?}", err);
                                None
                            }
                        }
                    });

                version.clone()
            })
    }

    /// The `router` version `rover dev` was given, and where.
    fn router_version_override(&self) -> RoverResult<Option<(VersionRequest, RequestOrigin)>> {
        let Some(version) = &self.opts.supergraph_opts.router_version else {
            return Ok(None);
        };
        Ok(Some((
            VersionRequest::Exact(Version::parse(version)?),
            RequestOrigin::flag_or_env("--router-version", "APOLLO_ROVER_DEV_ROUTER_VERSION"),
        )))
    }

    /// The `apollo-mcp-server` version `rover dev` was given, and where.
    fn mcp_version_override(&self) -> Option<(VersionRequest, RequestOrigin)> {
        let version = self.opts.mcp.version.clone()?;
        let origin = self.opts.mcp.version_origin()?;
        Some((Plugin::McpServer(version).request(), origin))
    }
}

/// The plugin `rover dev` runs for `plugin`, and where its version came from:
/// what it was `given`, ranked against the manifests at `levels` exactly as
/// every other plugin-using command ranks them, or `default`.
fn dev_plugin_request(
    plugin: PluginName,
    given: Option<(VersionRequest, RequestOrigin)>,
    default: VersionRequest,
    levels: &ManifestDirs,
) -> RoverResult<(Plugin, Option<RequestOrigin>)> {
    let declarations = declarations_in_scope(levels)?;
    let inputs = RequestInputs::new(default).with_override(given);
    let request = precedence::request(plugin, inputs, levels, &declarations)?;
    Ok((
        Plugin::from_request(plugin, &request.request)?,
        request.origin(),
    ))
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    /// `rover dev`'s router and MCP server installs download only when the
    /// client config says the run is opted in. The registry fails every request, so an install
    /// that may download fails resolving (E048) after asking it, and one
    /// that may not fails as a missing plugin (E058) without asking.
    #[cfg(not(target_env = "musl"))]
    #[tokio::test]
    #[rstest]
    #[case::the_router_not_opted_in(PluginName::Router, false)]
    #[case::the_router_opted_in(PluginName::Router, true)]
    #[case::the_mcp_server_not_opted_in(PluginName::ApolloMcpServer, false)]
    #[case::the_mcp_server_opted_in(PluginName::ApolloMcpServer, true)]
    async fn the_router_and_mcp_server_follow_the_runs_opt_in(
        #[case] plugin: PluginName,
        #[case] allow_automatic_download: bool,
    ) {
        use crate::{
            RoverErrorCode,
            command::{
                dev::{mcp::run::RunMcpServer, router::run::RunRouter},
                install::McpServerVersion,
            },
            options::LicenseAccepter,
            utils::client::{ClientBuilder, ClientTimeout},
        };

        let server = httpmock::MockServer::start();
        let registry = server.mock(|_, then| {
            then.status(500);
        });
        let host = format!("http://{}", server.address());
        let home = tempfile::tempdir().unwrap();
        let install_path = Utf8PathBuf::try_from(home.path().to_path_buf()).unwrap();
        let client_config = StudioClientConfig::new(
            Some(host.clone()),
            houston::Config {
                home: install_path.join("config"),
                override_api_key: Some("api-key".to_string()),
                override_client_credentials_token: None,
            },
            false,
            ClientBuilder::default(),
            ClientTimeout::new(1),
        )
        .with_download_host(host)
        .with_allow_automatic_download(allow_automatic_download);
        let license = LicenseAccepter {
            elv2_license_accepted: Some(true),
        };

        let code = temp_env::async_with_vars(
            [
                ("APOLLO_ROVER_SKIP_UPDATE", None::<&str>),
                ("APOLLO_ROVER_ALLOW_AUTOMATIC_DOWNLOAD", None),
                ("APOLLO_NODE_MODULES_BIN_DIR", None),
            ],
            async {
                let path = Some(install_path.clone());
                match plugin {
                    PluginName::Router => RunRouter::default()
                        .install(
                            apollo_federation_types::config::RouterVersion::LatestTwo,
                            None,
                            client_config,
                            path,
                            license,
                            false,
                        )
                        .await
                        .err()
                        .map(|err| RoverError::new(err).code()),
                    _ => RunMcpServer::default()
                        .install(
                            McpServerVersion::Latest,
                            None,
                            client_config,
                            path,
                            license,
                            false,
                        )
                        .await
                        .err()
                        .map(|err| RoverError::new(err).code()),
                }
            },
        )
        .await;

        let expected = if allow_automatic_download {
            (Some(Some(RoverErrorCode::E048)), true)
        } else {
            (Some(Some(RoverErrorCode::E058)), false)
        };
        assert_that!((code, registry.calls() > 0)).is_equal_to(expected);
    }

    const MANIFEST: &str = "plugins:\n  router: \"=2.1.0\"\n  apollo-mcp-server: latest\n";
    const LOCK: &str = "version = 1\n\n[[plugins]]\nname = \"router\"\nrequested = \"=2.1.0\"\nresolved = \"2.1.0\"\n\n[[plugins]]\nname = \"apollo-mcp-server\"\nrequested = \"latest\"\nresolved = \"1.0.3\"\n";

    /// The router and the MCP server take their versions by the same ladder
    /// as the `supergraph` plugin: what `rover dev` was given, then the
    /// manifests (pinned by their lockfile), then the default.
    #[rstest]
    #[case::the_router_flag(
        PluginName::Router,
        Some(("=2.3.0", RequestOrigin::Flag("--router-version"))),
        "2",
        ("=2.3.0", Some(RequestOrigin::Flag("--router-version")))
    )]
    #[case::the_router_variable(
        PluginName::Router,
        Some(("=2.3.0", RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION"))),
        "2",
        ("=2.3.0", Some(RequestOrigin::EnvVar("APOLLO_ROVER_DEV_ROUTER_VERSION")))
    )]
    #[case::the_router_declaration(
        PluginName::Router,
        None,
        "2",
        ("=2.1.0", Some(RequestOrigin::Manifest))
    )]
    #[case::the_locked_mcp_server(
        PluginName::ApolloMcpServer,
        None,
        "latest",
        ("=1.0.3", Some(RequestOrigin::Lockfile))
    )]
    fn each_plugin_takes_its_version_by_the_shared_ladder(
        #[case] plugin: PluginName,
        #[case] given: Option<(&str, RequestOrigin)>,
        #[case] default: &str,
        #[case] expected: (&str, Option<RequestOrigin>),
    ) {
        let temp = tempfile::tempdir().unwrap();
        let project = Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap();
        std::fs::write(project.join("rover.yaml"), MANIFEST).unwrap();
        std::fs::write(project.join("plugin-versions.lock"), LOCK).unwrap();
        let levels = ManifestDirs {
            global: None,
            project: Some(project),
        };
        let given = given.map(|(request, origin)| (request.parse().unwrap(), origin));

        let (converted, origin) =
            dev_plugin_request(plugin, given, default.parse().unwrap(), &levels).unwrap();

        assert_that!((converted.request().to_string(), origin))
            .is_equal_to((expected.0.to_string(), expected.1));
    }

    #[rstest]
    fn nothing_declared_takes_the_default() {
        let levels = ManifestDirs {
            global: None,
            project: None,
        };

        let (converted, origin) =
            dev_plugin_request(PluginName::Router, None, VersionRequest::Major(2), &levels)
                .unwrap();

        assert_that!((converted.request(), origin)).is_equal_to((VersionRequest::Major(2), None));
    }
}
