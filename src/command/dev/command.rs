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
use sysinfo::{Pid, PidExt, System, SystemExt};

use crate::{
    command::dev::{
        netstat::{get_all_local_graphql_endpoints_except, get_all_local_sockets_except},
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
        session_socket_addrs: &[SocketAddr],
    ) -> Result<SubgraphUrl> {
        let mut preexisting_socket_addrs = get_all_local_sockets_except(session_socket_addrs);
        preexisting_socket_addrs.extend(session_socket_addrs);
        self.spawn(&subgraph_name, &command)?;
        let mut new_graphql_endpoint = None;
        let now = Instant::now();
        let timeout_secs = 5;
        let mut err = RoverError::new(anyhow!(
            "could not find a new GraphQL endpoint for this `rover dev` session after {} seconds",
            timeout_secs
        ));
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
            Ok(graphql_endpoint)
        } else {
            self.kill_task(&subgraph_name);
            if err.suggestion().is_none() {
                err.set_suggestion(Suggestion::Adhoc("this is either a problem with your subgraph server, introspection is disabled, or it is being served from an endpoint other than the root, `/graphql` or `/query`. if you think this subgraph is running correctly, try re-running this command, and pass the endpoint via the `--url` argument.".to_string()))
            }
            Err(err)
        }
    }

    pub fn remove_subgraph_message(&self, subgraph_name: &SubgraphName) {
        let _ = self.message_sender.remove_subgraph(subgraph_name);
    }

    pub fn kill_task(&mut self, subgraph_name: &SubgraphName) {
        self.remove_subgraph_message(subgraph_name);
        self.tasks.remove(subgraph_name);
    }

    pub fn kill_tasks(&mut self) {
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

    fn kill(&mut self) {
        let pid = self.child.id();
        #[cfg(unix)]
        {
            // attempt to stop gracefully
            unsafe {
                libc::kill(libc::pid_t::from_ne_bytes(pid.to_ne_bytes()), libc::SIGTERM);
            }

            for _ in 0..10 {
                if self.child.try_wait().ok().flatten().is_some() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }

        if self.child.try_wait().ok().flatten().is_none() {
            // still alive? kill it with fire
            let _ = self.child.kill();
        }

        if self.child.try_wait().ok().flatten().is_none() {
            eprintln!("warn: could not kill process with PID '{}'", pid);
        }
    }
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.kill()
    }
}
