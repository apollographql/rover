use std::{
    collections::HashMap,
    net::SocketAddr,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use dialoguer::Select;
use rayon::{iter::ParallelIterator, prelude::IntoParallelRefIterator};
use reqwest::blocking::Client;
use saucer::{anyhow, Context};
use sysinfo::{Pid, PidExt, ProcessExt, System, SystemExt};

use crate::{
    command::dev::{
        netstat::{get_all_local_endpoints_except, get_all_local_graphql_endpoints_except},
        socket::{MessageSender, SubgraphName, SubgraphUrl},
    },
    error::RoverError,
    Result, Suggestion,
};

#[derive(Debug)]
pub struct CommandRunner {
    message_sender: MessageSender,
    tasks: HashMap<SubgraphName, BackgroundTask>,
    system: System,
}

impl CommandRunner {
    pub fn new(socket_addr: &str) -> Self {
        Self {
            message_sender: MessageSender::new(socket_addr),
            tasks: HashMap::new(),
            system: System::new(),
        }
    }

    pub fn spawn(&mut self, subgraph_name: &SubgraphName, command: &str) -> Result<()> {
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
            let pid = process.pid();
            self.tasks.insert(subgraph_name.to_string(), process);
            self.system.refresh_process(pid);
            Ok(())
        } else {
            Err(anyhow!("{} is not installed on this machine", &bin).into())
        }
    }

    pub fn spawn_and_find_url(
        &mut self,
        subgraph_name: SubgraphName,
        command: String,
        client: Client,
        preexisting_endpoints: &[SocketAddr],
    ) -> Result<SubgraphUrl> {
        let preexisting_endpoints = get_all_local_endpoints_except(preexisting_endpoints);
        self.spawn(&subgraph_name, &command)?;
        let mut new_graphql_endpoint = None;
        let now = Instant::now();
        while new_graphql_endpoint.is_none() && now.elapsed() < Duration::from_secs(5) {
            let graphql_endpoints =
                get_all_local_graphql_endpoints_except(client.clone(), &preexisting_endpoints);
            match graphql_endpoints.len() {
                0 => {}
                1 => new_graphql_endpoint = Some(graphql_endpoints[0].clone()),
                _ => {
                    if !atty::is(atty::Stream::Stdin) {
                        if let Ok(endpoint_index) = Select::new()
                            .items(&graphql_endpoints)
                            .default(0)
                            .interact()
                        {
                            new_graphql_endpoint = Some(graphql_endpoints[endpoint_index].clone());
                        }
                    } else {
                        eprintln!("detected multiple GraphQL endpoints: {:?}. select the correct endpoint and re-run this command with the `--url` argument.", &graphql_endpoints);
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        if let Some(graphql_endpoint) = new_graphql_endpoint {
            Ok(graphql_endpoint)
        } else {
            self.system.refresh_all();
            self.kill_subgraph(&subgraph_name);
            let mut err = RoverError::new(anyhow!(
                "could not find a new GraphQL endpoint for this `rover dev` session after 5 seconds"
            ));
            err.set_suggestion(Suggestion::Adhoc(format!("if '{}' seems to be working properly, try re-running this command with the `--url <ROUTING_URL>` argument. otherwise, fix up your GraphQL server before trying this command again", &command)));
            Err(err)
        }
    }

    pub fn kill_subgraph(&self, subgraph_name: &SubgraphName) {
        let background_task = self.tasks.get(subgraph_name);
        if let Some(background_task) = background_task {
            let _ = self.message_sender.remove_subgraph(subgraph_name);
            if let Some(process) = self.system.process(background_task.pid()) {
                if !process.kill() {
                    eprintln!(
                        "warn: could not drop process with PID {}",
                        background_task.pid()
                    );
                }
            }
        }
    }

    pub fn kill_tasks(&mut self) {
        if !self.tasks.is_empty() {
            let num_tasks = self.tasks.len();
            tracing::info!("dropping {} spawned background tasks", num_tasks);
            let subgraphs: Vec<&SubgraphName> = self.tasks.keys().collect();
            subgraphs.par_iter().for_each(|name| {
                self.kill_subgraph(name);
            })
        }
        tracing::info!("done dropping tasks");
    }
}

impl Drop for CommandRunner {
    fn drop(&mut self) {
        self.kill_tasks()
    }
}

#[derive(Debug)]
struct BackgroundTask {
    child: Child,
}

impl BackgroundTask {
    fn new(mut command: Command) -> Result<Self> {
        if cfg!(windows) {
            command.stdout(Stdio::null()).stderr(Stdio::null());
        }
        let child = command
            .spawn()
            .with_context(|| "could not spawn child process")?;
        Ok(Self { child })
    }

    fn pid(&self) -> Pid {
        Pid::from_u32(self.child.id())
    }
}
