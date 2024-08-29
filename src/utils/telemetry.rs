use camino::Utf8PathBuf;
use url::Url;

use crate::utils::env::RoverEnvKey;
use crate::{cli::Rover, PKG_NAME, PKG_VERSION};
use sputnik::{Command, Report, SputnikError};

use std::collections::HashMap;

const TELEMETRY_URL: &str = "https://rover.apollo.dev/telemetry";

fn get_command_from_args(raw_arguments: &mut serde_json::Value) -> Command {
    let mut commands = Vec::new();
    let mut arguments = HashMap::new();
    let mut should_break = true;
    loop {
        let (command_name, leftover_arguments) = get_next_command(raw_arguments);
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
                    next.clone_into(raw_arguments);
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
        tracing::debug!(?serialized_command);
        Ok(serialized_command)
    }

    fn is_telemetry_enabled(&self) -> Result<bool, SputnikError> {
        let value = self.get_env_var(RoverEnvKey::TelemetryDisabled)?;
        let is_telemetry_disabled = value.is_some();
        if is_telemetry_disabled {
            tracing::info!("Telemetry has been disabled.");
        } else {
            tracing::info!(
                "Telemetry is enabled. To disable, set ${}=true",
                RoverEnvKey::TelemetryDisabled.to_string()
            )
        }
        Ok(!is_telemetry_disabled)
    }

    fn endpoint(&self) -> Result<Url, SputnikError> {
        let url = self
            .get_env_var(RoverEnvKey::TelemetryUrl)?
            .unwrap_or_else(|| TELEMETRY_URL.to_string());
        Ok(Url::parse(&url)?)
    }

    fn tool_name(&self) -> String {
        PKG_NAME.to_string()
    }

    fn version(&self) -> String {
        PKG_VERSION.to_string()
    }

    fn machine_id_config(&self) -> Result<Utf8PathBuf, SputnikError> {
        let config = self
            .get_rover_config()
            .map_err(|_| SputnikError::ConfigError)?;
        Ok(config.home.join("machine.txt"))
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Rover;
    use crate::utils::env::RoverEnvKey;
    use crate::utils::telemetry::Report;
    use crate::PKG_NAME;

    use sputnik::Command;

    use clap::Parser;
    use serde_json::json;

    use std::collections::HashMap;

    #[test]
    fn it_can_serialize_commands() {
        let args = vec![PKG_NAME, "config", "list"];
        let rover = Rover::parse_from(args);
        let actual_serialized_command = rover
            .serialize_command()
            .expect("could not serialize command");
        let expected_serialized_command = Command {
            name: "config list".to_string(),
            arguments: HashMap::new(),
        };
        assert_eq!(actual_serialized_command, expected_serialized_command);
    }

    #[test]
    fn it_can_serialize_commands_with_arguments() {
        let args = vec![PKG_NAME, "config", "list", "--help"];
        let rover = Rover::parse_from(args);
        let actual_serialized_command = rover
            .serialize_command()
            .expect("could not serialize command");
        let mut expected_arguments = HashMap::new();
        expected_arguments.insert("help".to_string(), json!(true));
        let expected_serialized_command = Command {
            name: "config whoami".to_string(),
            arguments: expected_arguments,
        };
        assert_eq!(actual_serialized_command, expected_serialized_command);
    }

    #[test]
    fn it_respects_apollo_telemetry_url() {
        let apollo_telemetry_url = "https://example.com/telemetry";
        let args = vec![PKG_NAME, "config", "list"];
        let mut rover = Rover::parse_from(args);
        rover
            .insert_env_var(RoverEnvKey::TelemetryUrl, apollo_telemetry_url)
            .unwrap();
        let actual_endpoint = rover
            .endpoint()
            .expect("could not parse telemetry URL")
            .to_string();
        let expected_endpoint = apollo_telemetry_url.to_string();

        assert_eq!(actual_endpoint, expected_endpoint);
    }

    #[test]
    fn it_can_be_disabled() {
        let args = vec![PKG_NAME, "config", "list"];
        let mut rover = Rover::parse_from(args);
        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
            .unwrap();
        let expect_enabled = false;
        let mut is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();

        assert_eq!(is_telemetry_enabled, expect_enabled);

        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "true")
            .unwrap();
        is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();

        assert_eq!(is_telemetry_enabled, expect_enabled);
    }

    #[test]
    fn it_is_enabled_by_default() {
        let args = vec![PKG_NAME, "config", "list"];
        let rover = Rover::parse_from(args);
        let expect_enabled = true;
        let is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();
        assert_eq!(is_telemetry_enabled, expect_enabled);
    }
}
