use std::collections::HashMap;

use camino::Utf8PathBuf;
use tokio::process::Child;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tokio_util::sync::CancellationToken;
use tower::Service;

use super::binary::{McpServerLog, RunMcpServerBinary, RunMcpServerBinaryError};
use super::install::{InstallMcpServer, InstallMcpServerError};
use crate::command::dev::router::config::RouterAddress;
use crate::command::install::McpServerVersion;
use crate::options::LicenseAccepter;
use crate::subtask::{Subtask, SubtaskRunUnit};
use crate::utils::client::StudioClientConfig;
use crate::utils::effect::exec::ExecCommandConfig;
use crate::utils::effect::install::InstallBinary;

pub struct RunMcpServer<S> {
    pub(crate) state: S,
}

impl Default for RunMcpServer<state::Install> {
    fn default() -> Self {
        RunMcpServer {
            state: state::Install::default(),
        }
    }
}

impl RunMcpServer<state::Install> {
    pub async fn install(
        self,
        mcp_server_version: McpServerVersion,
        studio_client_config: StudioClientConfig,
        override_install_path: Option<Utf8PathBuf>,
        elv2_license_accepter: LicenseAccepter,
        skip_update: bool,
    ) -> Result<RunMcpServer<state::Run>, InstallMcpServerError> {
        let install_binary = InstallMcpServer::new(mcp_server_version, studio_client_config);
        let binary = install_binary
            .install(override_install_path, elv2_license_accepter, skip_update)
            .await?;
        Ok(RunMcpServer {
            state: state::Run { binary },
        })
    }
}

impl RunMcpServer<state::Run> {
    pub async fn run<Spawn>(
        self,
        spawn: Spawn,
        supergraph_schema_path: Utf8PathBuf,
        router_address: RouterAddress,
        mcp_config_path: Option<Utf8PathBuf>,
        env: HashMap<String, String>,
    ) -> Result<RunMcpServer<state::Abort>, RunMcpServerBinaryError>
    where
        Spawn: Service<ExecCommandConfig, Response = Child> + Send + Clone + 'static,
        Spawn::Error: std::error::Error + Send + Sync,
        Spawn::Future: Send,
    {
        let run_mcp_server_binary = RunMcpServerBinary::builder()
            .mcp_server_binary(self.state.binary.clone())
            .supergraph_schema_path(supergraph_schema_path.clone())
            .spawn(spawn)
            .router_address(router_address)
            .and_mcp_config_path(mcp_config_path)
            .env(env)
            .build();

        let (mcp_server_logs, run_mcp_server_binary_subtask): (
            UnboundedReceiverStream<Result<McpServerLog, RunMcpServerBinaryError>>,
            _,
        ) = Subtask::new(run_mcp_server_binary);

        let cancellation_token = CancellationToken::new();
        SubtaskRunUnit::run(
            run_mcp_server_binary_subtask,
            Some(cancellation_token.clone()),
        );

        Ok(RunMcpServer {
            state: state::Abort {
                cancellation_token: cancellation_token.clone(),
                mcp_server_logs,
                supergraph_schema_path,
            },
        })
    }
}

impl RunMcpServer<state::Abort> {
    pub fn mcp_server_logs(
        &mut self,
    ) -> &mut UnboundedReceiverStream<Result<McpServerLog, RunMcpServerBinaryError>> {
        &mut self.state.mcp_server_logs
    }

    pub fn shutdown(&mut self) {
        self.state.cancellation_token.cancel();
    }
}

pub mod state {
    use camino::Utf8PathBuf;
    use tokio_stream::wrappers::UnboundedReceiverStream;
    use tokio_util::sync::CancellationToken;

    use crate::command::dev::mcp::binary::{
        McpServerBinary, McpServerLog, RunMcpServerBinaryError,
    };

    #[derive(Default)]
    pub struct Install {}
    pub struct Run {
        pub binary: McpServerBinary,
    }

    pub struct Abort {
        pub mcp_server_logs: UnboundedReceiverStream<Result<McpServerLog, RunMcpServerBinaryError>>,
        #[allow(unused)]
        pub cancellation_token: CancellationToken,
        #[allow(unused)]
        pub supergraph_schema_path: Utf8PathBuf,
    }
}
