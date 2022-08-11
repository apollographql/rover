use std::{
    collections::HashMap,
    net::SocketAddr,
    process::{Command, Stdio},
    sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender},
    time::{Duration, Instant},
};

use command_group::{CommandGroup, GroupChild};
use dialoguer::Select;
use rayon::{iter::ParallelIterator, prelude::IntoParallelRefIterator};
use reqwest::blocking::Client;
use saucer::{anyhow, Context};

use crate::{
    command::dev::{
        do_dev::log_err_and_continue,
        netstat::{get_all_local_graphql_endpoints_except, get_all_local_sockets_except},
        socket::{MessageSender, SubgraphName, SubgraphUrl},
    },
    error::RoverError,
    Result, Suggestion,
};

pub struct CommandRunner {
    background_task_runner: BackgroundTaskRunner,
    command_message_receiver: Receiver<CommandRunnerMessage>,
}

impl CommandRunner {
    pub fn new(
        socket_addr: &str,
        command_message_receiver: Receiver<CommandRunnerMessage>,
    ) -> Self {
        Self {
            background_task_runner: BackgroundTaskRunner::new(socket_addr),
            command_message_receiver,
        }
    }

    pub fn message_channel() -> (Sender<CommandRunnerMessage>, Receiver<CommandRunnerMessage>) {
        channel()
    }

    pub fn url_channel() -> (
        SyncSender<Result<SubgraphUrl>>,
        Receiver<Result<SubgraphUrl>>,
    ) {
        sync_channel(1)
    }

    pub fn ready_channel() -> (SyncSender<()>, Receiver<()>) {
        sync_channel(1)
    }

    pub fn handle_command_runner_messages(&mut self) -> ! {
        loop {
            match self.command_message_receiver.recv().unwrap() {
                CommandRunnerMessage::SpawnTaskAndFindUrl {
                    subgraph_name,
                    command,
                    client,
                    preexisting_socket_addrs,
                    url_sender,
                } => {
                    let url_result = self.background_task_runner.spawn_and_find_url(
                        subgraph_name,
                        command,
                        client,
                        &preexisting_socket_addrs,
                    );
                    url_sender.send(url_result).unwrap();
                }
                CommandRunnerMessage::SpawnTask {
                    subgraph_name,
                    command,
                    ready_sender,
                } => {
                    let _ = self
                        .background_task_runner
                        .spawn(&subgraph_name, &command)
                        .map_err(log_err_and_continue);
                    ready_sender.send(()).unwrap()
                }
                CommandRunnerMessage::KillTask {
                    subgraph_name,
                    ready_sender,
                } => {
                    self.background_task_runner.kill_task(&subgraph_name);
                    ready_sender.send(()).unwrap();
                }
                CommandRunnerMessage::KillTasks { ready_sender } => {
                    self.background_task_runner.kill_tasks();
                    ready_sender.send(()).unwrap();
                }
            };
        }
    }
}

pub enum CommandRunnerMessage {
    SpawnTaskAndFindUrl {
        subgraph_name: SubgraphName,
        command: String,
        client: Client,
        preexisting_socket_addrs: Vec<SocketAddr>,
        url_sender: SyncSender<Result<SubgraphUrl>>,
    },
    SpawnTask {
        subgraph_name: SubgraphName,
        command: String,
        ready_sender: SyncSender<()>,
    },
    KillTask {
        subgraph_name: SubgraphName,
        ready_sender: SyncSender<()>,
    },
    KillTasks {
        ready_sender: SyncSender<()>,
    },
}

#[derive(Debug)]
struct BackgroundTaskRunner {
    message_sender: MessageSender,
    tasks: HashMap<SubgraphName, BackgroundTask>,
}

impl BackgroundTaskRunner {
    fn new(socket_addr: &str) -> Self {
        Self {
            message_sender: MessageSender::new(socket_addr),
            tasks: HashMap::new(),
        }
    }

    fn spawn(&mut self, subgraph_name: &SubgraphName, command: &str) -> Result<()> {
        tracing::info!(
            "spawning command '{}' for subgraph '{}'",
            command,
            subgraph_name
        );
        for existing_name in self.tasks.keys() {
            if subgraph_name == existing_name {
                return Err(RoverError::new(anyhow!(
                    "subgraph with name '{}' already has a running process",
                    subgraph_name
                )));
            }
        }
        let args: Vec<&str> = command.split(' ').collect();
        let (bin, args) = match args.len() {
            0 => Err(anyhow!("the command you passed is empty")),
            1 => Ok((args[0], Vec::new())),
            _ => Ok((args[0], Vec::from_iter(args[1..].iter()))),
        }?;
        tracing::info!("starting `{}`", &command);
        if which::which(bin).is_ok() {
            let mut command = Command::new(bin);
            command.args(args);
            let process = BackgroundTask::new(command)?;
            self.tasks.insert(subgraph_name.to_string(), process);
            Ok(())
        } else {
            Err(anyhow!("{} is not installed on this machine", &bin).into())
        }
    }

    fn spawn_and_find_url(
        &mut self,
        subgraph_name: SubgraphName,
        command: String,
        client: Client,
        session_socket_addrs: &[SocketAddr],
    ) -> Result<SubgraphUrl> {
        let mut preexisting_socket_addrs = get_all_local_sockets_except(session_socket_addrs);
        preexisting_socket_addrs.extend(session_socket_addrs);
        self.spawn(&subgraph_name, &command)?;
        let mut new_graphql_endpoint = None;
        let now = Instant::now();
        let timeout_secs = 120;
        let mut err = RoverError::new(anyhow!(
            "could not find a new GraphQL endpoint for this `rover dev` session after {} seconds",
            timeout_secs
        ));
        eprintln!("searching for running GraphQL servers... to skip this step, pass the `--url <SUBGRAPH_URL>` argument");
        while new_graphql_endpoint.is_none() && now.elapsed() < Duration::from_secs(timeout_secs) {
            let graphql_endpoints =
                get_all_local_graphql_endpoints_except(client.clone(), &preexisting_socket_addrs);
            match graphql_endpoints.len() {
                0 => {}
                1 => new_graphql_endpoint = Some(graphql_endpoints[0].clone()),
                _ => {
                    if atty::is(atty::Stream::Stderr) {
                        if let Ok(endpoint_index) = Select::new()
                            .items(&graphql_endpoints)
                            .default(0)
                            .interact()
                        {
                            new_graphql_endpoint = Some(graphql_endpoints[endpoint_index].clone());
                        }
                    } else {
                        let strs = graphql_endpoints
                            .iter()
                            .map(|u| u.to_string())
                            .collect::<Vec<String>>();
                        err = RoverError::new(anyhow!(
                            "detected multiple GraphQL endpoints: {:?}",
                            &strs
                        ));
                        err.set_suggestion(Suggestion::Adhoc("select the correct endpoint and re-run this command with the `--url` argument.".to_string()));
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if let Some(graphql_endpoint) = new_graphql_endpoint {
            eprintln!("detected GraphQL server at {}", &graphql_endpoint);
            Ok(graphql_endpoint)
        } else {
            self.kill_task(&subgraph_name);
            if err.suggestion().is_none() {
                err.set_suggestion(Suggestion::Adhoc("this is either a problem with your subgraph server, introspection is disabled, or it is being served from an endpoint other than the root, `/graphql` or `/query`. if you think this subgraph is running correctly, try re-running this command, and pass the endpoint via the `--url` argument.".to_string()))
            }
            Err(err)
        }
    }

    fn remove_subgraph_message(&self, subgraph_name: &SubgraphName) {
        let _ = self.message_sender.remove_subgraph(subgraph_name);
    }

    fn kill_task(&mut self, subgraph_name: &SubgraphName) {
        tracing::info!("killing spawned task for subgraph '{}'", subgraph_name);
        self.remove_subgraph_message(subgraph_name);
        self.tasks.remove(subgraph_name);
    }

    fn kill_tasks(&mut self) {
        tracing::info!("killing all tasks");
        let subgraphs: Vec<&SubgraphName> = self.tasks.keys().collect();
        subgraphs
            .par_iter()
            .for_each(|name| self.remove_subgraph_message(name));
        self.tasks = HashMap::new();
        tracing::info!("done killing tasks");
    }
}

#[derive(Debug)]
struct BackgroundTask {
    child: GroupChild,
}

impl BackgroundTask {
    fn new(mut command: Command) -> Result<Self> {
        if cfg!(windows) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let child = command
            .group_spawn()
            .with_context(|| "could not spawn child process")?;
        Ok(Self { child })
    }

    fn kill(&mut self) {
        let pgid = self.child.id();
        tracing::info!("killing child with pgid {}", &pgid);
        let _ = self.child.kill().map_err(|_| {
            log_err_and_continue(RoverError::new(anyhow!(
                "could not kill child group with pgid {}",
                &pgid
            )));
        });
    }
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        tracing::info!("background task with pgid {} was dropped", &self.child.id());
        self.kill()
    }
}
