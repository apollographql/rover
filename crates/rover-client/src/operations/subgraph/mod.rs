/// "subgraph delete" command execution
pub mod delete;

/// "subgraph check" command execution
pub mod check_workflow;

/// "subgraph check --async" command execution
pub mod check;

/// "subgraph fetch" command execution
pub mod fetch;

/// "subgraph publish" command execution
pub mod publish;

/// "subgraph publish" no (--routing-url) command execution
pub mod routing_url;

/// "subgraph list"
pub mod list;

/// "subgraph introspect"
pub mod introspect;
