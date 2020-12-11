use url::Url;

use std::path::PathBuf;

use crate::cli::Rover;
use sputnik::{Command, Report, SputnikError};

use std::collections::HashMap;

const DEV_TELEMETRY_URL: &str = "http://localhost:8787/telemetry";
const PROD_TELEMETRY_URL: &str = "https://install.apollographql.com/telemetry";

fn get_command_from_args(mut raw_arguments: &mut serde_json::Value) -> Command {
    let mut commands = Vec::new();
    let mut arguments = HashMap::new();
    let mut should_break = true;
    loop {
        let (command_name, leftover_arguments) = get_next_command(&mut raw_arguments);
        if let Some(command_name) = command_name {
            commands.push(command_name);
            should_break = false;
        }

        if let Some(serde_json::Value::Object(object)) = leftover_arguments {
            for argument in object.iter() {
                let (key, value) = argument;
                arguments.insert(key.to_lowercase(), value.to_owned());
            }
        }

        if should_break {
            break;
        } else {
            should_break = true;
        }
    }

    let name = commands.join(" ");
    Command { name, arguments }
}

fn get_next_command(
    raw_arguments: &mut serde_json::Value,
) -> (Option<String>, Option<serde_json::Value>) {
    let mut command_name = None;
    let mut leftover_arguments = None;

    if let Some(command_info) = raw_arguments.get("command") {
        match command_info {
            serde_json::Value::Object(object) => {
                if let Some(item) = object.clone().iter_mut().next() {
                    let (name, next) = item;
                    command_name = Some(name.to_lowercase());
                    *raw_arguments = next.to_owned();
                }
            }
            serde_json::Value::String(string) => {
                command_name = Some(string.to_lowercase());
                *raw_arguments = serde_json::Value::Null;
            }
            serde_json::Value::Null => command_name = None,
            _ => {
                command_name = Some(format!("{:?}", command_info).to_lowercase());
                *raw_arguments = serde_json::Value::Null;
            }
        }
    } else {
        leftover_arguments = Some(raw_arguments.clone());
    }

    (command_name, leftover_arguments)
}

impl Report for Rover {
    fn serialize_command(&self) -> Result<Command, SputnikError> {
        let json_args = serde_json::to_string(&self)?;
        let mut value_args = serde_json::from_str(&json_args)?;
        let serialized_command = get_command_from_args(&mut value_args);
        tracing::debug!(serialized_command = ?serialized_command);
        Ok(serialized_command)
    }

    fn is_enabled(&self) -> bool {
        std::env::var_os("APOLLO_TELEMETRY_DISABLED").is_none()
    }

    fn endpoint(&self) -> Result<Url, SputnikError> {
        if let Ok(url) = std::env::var("APOLLO_TELEMETRY_URL") {
            Ok(url.parse()?)
        } else if cfg!(debug_assertions) {
            Ok(DEV_TELEMETRY_URL.parse()?)
        } else {
            Ok(PROD_TELEMETRY_URL.parse()?)
        }
    }

    fn tool_name(&self) -> String {
        option_env!("CARGO_PKG_NAME")
            .unwrap_or("unknown")
            .to_string()
    }

    fn version(&self) -> String {
        option_env!("CARGO_PKG_VERSION")
            .unwrap_or("unknown")
            .to_string()
    }

    fn machine_id_config(&self) -> Result<PathBuf, SputnikError> {
        let mut path = houston::dir().map_err(|_| SputnikError::ConfigError)?;
        path.push("machine.txt");
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Rover;
    use crate::telemetry::Report;
    use sputnik::Command;

    use serde_json::json;
    use structopt::StructOpt;

    use serial_test::serial;

    use std::collections::HashMap;

    #[test]
    fn it_can_serialize_commands() {
        let cli_name = env!("CARGO_PKG_NAME");
        let args = vec![cli_name, "config", "profile", "list"];
        let rover = Rover::from_iter(args);
        let actual_serialized_command = rover
            .serialize_command()
            .expect("could not serialize command");
        let expected_serialized_command = Command {
            name: "config profile list".to_string(),
            arguments: HashMap::new(),
        };
        assert_eq!(actual_serialized_command, expected_serialized_command);
    }

    #[test]
    fn it_can_serialize_commands_with_arguments() {
        let cli_name = env!("CARGO_PKG_NAME");
        let args = vec![
            cli_name,
            "config",
            "profile",
            "show",
            "default",
            "--sensitive",
        ];
        let rover = Rover::from_iter(args);
        let actual_serialized_command = rover
            .serialize_command()
            .expect("could not serialize command");
        let mut expected_arguments = HashMap::new();
        expected_arguments.insert("sensitive".to_string(), json!(true));
        let expected_serialized_command = Command {
            name: "config profile show".to_string(),
            arguments: expected_arguments,
        };
        assert_eq!(actual_serialized_command, expected_serialized_command);
    }

    #[test]
    #[serial]
    fn it_respects_apollo_telemetry_url() {
        let apollo_telemetry_url = "https://example.com/telemetry";
        std::env::set_var("APOLLO_TELEMETRY_URL", apollo_telemetry_url);
        let cli_name = env!("CARGO_PKG_NAME");
        let args = vec![cli_name, "config", "profile", "list"];
        let rover = Rover::from_iter(args);
        let actual_endpoint = rover
            .endpoint()
            .expect("could not parse telemetry URL")
            .to_string();
        let expected_endpoint = apollo_telemetry_url.to_string();
        assert_eq!(actual_endpoint, expected_endpoint);
    }

    #[test]
    #[serial]
    fn it_can_be_disabled() {
        std::env::set_var("APOLLO_TELEMETRY_DISABLED", "1");
        let cli_name = env!("CARGO_PKG_NAME");
        let args = vec![cli_name, "config", "profile", "list"];
        let rover = Rover::from_iter(args);
        let expect_enabled = false;
        let is_enabled = rover.is_enabled();

        // unset the env var so it does not affect subsequent tests
        std::env::remove_var("APOLLO_TELEMETRY_DISABLED");

        assert_eq!(is_enabled, expect_enabled);
    }

    #[test]
    #[serial]
    fn it_is_enabled_by_default() {
        let cli_name = env!("CARGO_PKG_NAME");
        let args = vec![cli_name, "config", "profile", "list"];
        let rover = Rover::from_iter(args);
        let expect_enabled = true;
        let is_enabled = rover.is_enabled();
        assert_eq!(is_enabled, expect_enabled);
    }
}
