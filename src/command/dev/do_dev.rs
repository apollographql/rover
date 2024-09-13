use camino::Utf8PathBuf;
use rover_std::{infoln, warnln};
use tokio::sync::mpsc::unbounded_channel;

use crate::command::dev::subtask::SubtaskHandleUnit;
use crate::command::dev::watcher::file::FileWatcher;
use crate::command::dev::{runner::Runner, watcher::supergraph_config::SupergraphConfigWatcher};
use crate::utils::{client::StudioClientConfig, supergraph_config::get_supergraph_config};
use crate::{RoverError, RoverOutput, RoverResult};

use super::Dev;

pub fn log_err_and_continue(err: RoverError) -> RoverError {
    let _ = err.print();
    err
}

impl Dev {
    pub async fn run(
        &self,
        _override_install_path: Option<Utf8PathBuf>,
        client_config: StudioClientConfig,
    ) -> RoverResult<RoverOutput> {
        // Check for license acceptance.
        self.opts
            .plugin_opts
            .prompt_for_license_accept(&client_config)?;

        warnln!(
            "Do not run this command in production! It is intended for local development only."
        );

        // let mut dev_runner = Runner::new(&client_config, &self.opts.supergraph_opts);

        // infoln!("Starting main `rover dev` process");
        // dev_runner.run(&self.opts.plugin_opts.profile).await?;

        let supergraph_config = get_supergraph_config(
            &self.opts.supergraph_opts.graph_ref,
            &self.opts.supergraph_opts.supergraph_config_path,
            self.opts.supergraph_opts.federation_version.as_ref(),
            client_config.clone(),
            &self.opts.plugin_opts.profile,
            false,
        )
        .await
        .unwrap()
        .unwrap();

        let f = FileWatcher::new(Utf8PathBuf::from(
            "./examples/supergraph-demo/supergraph.yaml",
        ));
        let supergraph_config_watcher = SupergraphConfigWatcher::new(f, supergraph_config);

        let (tx, mut rx) = unbounded_channel();
        supergraph_config_watcher.handle(tx);

        loop {
            rx.recv().await;
            eprintln!("supergraph update");
        }

        unreachable!("todo");
        // Ok(RoverOutput::EmptySuccess)
    }
}
