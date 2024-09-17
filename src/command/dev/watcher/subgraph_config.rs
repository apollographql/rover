use std::fs::OpenOptions;
use std::{marker::Send, pin::Pin};

use anyhow::{anyhow, Error};
use apollo_federation_types::config::{SchemaSource, SubgraphConfig};
use camino::Utf8PathBuf;
use futures::stream::BoxStream;
use futures::{Stream, StreamExt};
use rover_std::errln;
use tap::TapFallible;
use tokio::{sync::mpsc::UnboundedSender, task::AbortHandle};

use crate::cli::RoverOutputFormatKind;
use crate::command::dev::{subtask::SubtaskHandleUnit, types::SubgraphUrl};
use crate::command::subgraph::introspect::Introspect as SubgraphIntrospect;
use crate::options::{IntrospectOpts, OutputOpts};

use super::file::FileWatcher;

#[derive(Debug, Clone)]
pub enum SubgraphConfigWatcherKind {
    /// Watch a file on disk.
    File(FileWatcher),
    /// Poll an endpoint via introspection.
    Introspect(SubgraphIntrospection),
    /// Don't ever update, schema is only pulled once.
    _Once(String),
}

#[derive(Debug, Clone)]
pub struct SubgraphIntrospection {
    endpoint: SubgraphUrl,
    // TODO: ticket using a hashmap, not a tuple, in introspect opts as eventual cleanup
    headers: Option<Vec<(String, String)>>,
}

//TODO: impl retry (needed at least for dev)
impl SubgraphIntrospection {
    fn new(endpoint: SubgraphUrl, headers: Option<Vec<(String, String)>>) -> Self {
        Self { endpoint, headers }
    }

    async fn watch(&self, subgraph_name: &str) -> FileWatcher {
        let client = reqwest::Client::new();

        //FIXME: unwrap removed
        // TODO: does this re-use tmp dirs? or, what? don't want errors second time we run
        // TODO: clean up after?
        let tmp_dir = tempfile::Builder::new().tempdir().unwrap();
        let tmp_config_dir_path = Utf8PathBuf::try_from(tmp_dir.into_path()).unwrap();

        // NOTE: this assumes subgraph names are unique; are they?
        let tmp_introspection_file = tmp_config_dir_path.join(subgraph_name);

        let _ = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(tmp_introspection_file.clone())
            // FIXME: unwrap
            .unwrap();

        let output_opts = OutputOpts {
            format_kind: RoverOutputFormatKind::default(),
            output_file: Some(tmp_introspection_file.clone()),
        };

        let endpoint = self.endpoint.clone();
        let headers = self.headers.clone();

        tokio::spawn(async move {
            let _ = SubgraphIntrospect {
                opts: IntrospectOpts {
                    endpoint,
                    headers,
                    // TODO impl retries (at least for dev from cli flag)
                    watch: true,
                },
            }
            .run(client, &output_opts, None)
            .await
            .map_err(|err| anyhow!(err));
        });

        FileWatcher::new(tmp_introspection_file)
    }
}

impl SubgraphConfigWatcherKind {
    async fn watch(&self, subgraph_name: &str) -> Pin<Box<dyn Stream<Item = String> + Send>> {
        match self {
            Self::File(file_watcher) => file_watcher.clone().watch(),
            Self::Introspect(introspection) => {
                let watcher = introspection.watch(subgraph_name).await;
                println!("watcher: {watcher:?}");

                watcher.watch()
            }
            Self::_Once(_) => todo!(),
        }
    }
}

impl TryFrom<SchemaSource> for SubgraphConfigWatcherKind {
    // FIXME: anyhow error -> bespoke error with impl From to rovererror or whatever
    type Error = anyhow::Error;
    fn try_from(schema_source: SchemaSource) -> Result<Self, Self::Error> {
        match schema_source {
            SchemaSource::File { file } => Ok(Self::File(FileWatcher::new(file))),
            SchemaSource::SubgraphIntrospection {
                subgraph_url,
                introspection_headers,
            } => Ok(Self::Introspect(SubgraphIntrospection {
                endpoint: subgraph_url,
                headers: introspection_headers.map(|header_map| header_map.into_iter().collect()),
            })),
            // SDL (stdin? not sure) / Subgraph (ie, from graph-ref)
            unsupported_source => Err(anyhow!(
                "unsupported subgraph introspection source: {unsupported_source:?}"
            )),
        }
    }
}

pub struct SubgraphConfigWatcher {
    subgraph_name: String,
    watcher: SubgraphConfigWatcherKind,
}

impl SubgraphConfigWatcher {
    // not sure we need the subgraph config?
    pub fn new(watcher: SubgraphConfigWatcherKind, subgraph_name: &str) -> Self {
        Self {
            watcher,
            subgraph_name: subgraph_name.to_string(),
        }
    }
}

/// A unit struct denoting a change to a subgraph, used by composition to know whether to recompose
pub struct SubgraphChanged;

impl SubtaskHandleUnit for SubgraphConfigWatcher {
    type Output = SubgraphChanged;

    fn handle(self, sender: UnboundedSender<Self::Output>) -> AbortHandle {
        tokio::spawn(async move {
            while let Some(content) = self.watcher.watch(&self.subgraph_name).await.next().await {
                // TODO: fix parsing; see wtf is up
                //let parsed_config: Result<SubgraphConfig, serde_yaml::Error> =
                //    serde_yaml::from_str(&content);
                let _ = sender
                    .send(SubgraphChanged)
                    .tap_err(|err| tracing::error!("{:?}", err));

                // We're only looking at whether a subgraph has changed, but we won't emit events
                // if the subgraph config can't be parsed to fail early for composition
                //match parsed_config {
                //    Ok(_subgraph_config) => {
                //        let _ = sender
                //            .send(SubgraphChanged)
                //            .tap_err(|err| tracing::error!("{:?}", err));
                //    }
                //    Err(err) => {
                //        tracing::error!("Could not parse subgraph config file: {:?}", err);
                //        errln!("could not parse subgraph config file");
                //    }
                //}
            }
        })
        .abort_handle()
    }
}
