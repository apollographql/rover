mod handler;
mod messages;
mod run;
mod subtask;
pub mod watcher;

pub fn rover_dev() {
    load_config();
    install_plugins();
    configure_ctrlc_handler();
    start_subgraph_watchers();
    start_composition();
    start_router_config_watcher();
    start_router();
    run_rover_dev();
}

fn load_config() {
    ()
}

fn install_plugins() {
    ()
}

fn configure_ctrlc_handler() {
    ()
}

fn start_subgraph_watchers() {
    ()
}

fn start_composition() {
    ()
}

fn start_router_config_watcher() {
    ()
}

fn start_router() {
    ()
}

fn run_rover_dev() {
    ()
}
