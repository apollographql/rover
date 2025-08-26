use std::collections::HashMap;
use std::{env, fmt, io};

use heck::AsShoutySnekCase;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

/// RoverEnv allows us to mock environment variables while
/// running tests. That way we can run our tests in parallel,
/// and our local development environment will not have unintended
/// side effects on our tests.
#[derive(Debug, Clone)]
pub struct RoverEnv {
    env_store: HashMap<String, String>,
}

impl Default for RoverEnv {
    fn default() -> RoverEnv {
        RoverEnv::new()
            .expect("Encountered one or more errors while reading environment variables.")
    }
}

impl RoverEnv {
    /// creates a new environment variable store
    pub fn new() -> Result<RoverEnv, io::Error> {
        let env_store = if cfg!(test) {
            HashMap::new()
        } else {
            let mut env_store = HashMap::new();
            for key in RoverEnvKey::iter() {
                let key_str = key.to_string();

                match env::var(&key_str) {
                    Ok(value) => {
                        tracing::debug!("{}", Self::get_debug_value(key, &value));
                        env_store.insert(key_str, value);
                        Ok(())
                    }
                    Err(e) => match e {
                        env::VarError::NotPresent => {
                            tracing::trace!("${} is not set", &key_str);
                            Ok(())
                        }
                        env::VarError::NotUnicode(_) => Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            format!(
                                "The value of the environment variable \"{}\" is not valid Unicode.",
                                &key
                            ),
                        )),
                    },
                }?;
            }
            env_store
        };

        Ok(RoverEnv { env_store })
    }

    /// returns the value of the environment variable if it exists
    pub fn get(&self, key: RoverEnvKey) -> Option<String> {
        self.env_store.get(&key.to_string()).map(|s| s.to_string())
    }

    fn get_debug_value(key: RoverEnvKey, value: &str) -> String {
        let value = if let RoverEnvKey::Key = key {
            houston::mask_key(value)
        } else {
            value.to_string()
        };

        format!("${key} = {value}")
    }

    /// sets an environment variable to a value
    pub fn insert(&mut self, key: RoverEnvKey, value: &str) {
        self.env_store.insert(key.to_string(), value.into());
    }

    /// unsets an environment variable
    pub fn remove(&mut self, key: RoverEnvKey) {
        self.env_store.remove(&key.to_string());
    }
}

/// RoverEnvKey defines all of the environment variables
/// that are respected by Rover. Any time a new environment variable
/// is added to the public contract, it should be defined here.
/// Each environment variable is prefixed with `APOLLO_` and
/// the suffix is the name of the key defined here. It will automatically
/// be converted from CamelCase to SHOUTY_SNEK_CASE.
/// For example, `RoverEnvKey::ConfigHome.to_string()` becomes `APOLLO_CONFIG_HOME`
#[derive(Debug, Copy, Clone, EnumIter)]
pub enum RoverEnvKey {
    ConfigHome,
    FireFlower,
    Home,
    Key,
    RegistryUrl,
    TelemetryUrl,
    TelemetryDisabled,
    VcsRemoteUrl,
    VcsBranch,
    VcsCommit,
    VcsAuthor,
    NodeModulesBin,
    ChecksTimeoutSeconds,
}

impl fmt::Display for RoverEnvKey {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        let dbg = format!("{self:?}");
        fmt.write_str(&format!("APOLLO_{}", AsShoutySnekCase(&dbg)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_doesnt_read_from_real_env_in_tests() {
        assert!(RoverEnv::new().unwrap().env_store.is_empty())
    }

    #[test]
    fn it_parses_config_home() {
        let expected_key = "APOLLO_CONFIG_HOME";
        assert_eq!(&RoverEnvKey::ConfigHome.to_string(), expected_key);
    }

    #[test]
    fn it_can_set_and_read_from_mock() {
        let expected_value = "hey whats the big idea anyway!??";
        let key = RoverEnvKey::ConfigHome;
        let mut env_store = RoverEnv::new().unwrap();
        env_store.insert(key, expected_value);
        let actual_value = env_store.get(key).unwrap();
        assert_eq!(expected_value, &actual_value)
    }

    #[test]
    fn it_can_remove_from_mock() {
        let expected_value = "hey whats the big idea anyway!??";
        let key = RoverEnvKey::ConfigHome;
        let mut env_store = RoverEnv::new().unwrap();
        env_store.insert(key, expected_value);
        let actual_value = env_store.get(key).unwrap();
        assert_eq!(expected_value, &actual_value);
        env_store.remove(key);
        let expected_value = None;
        let actual_value = env_store.get(key);
        assert_eq!(expected_value, actual_value);
    }
}
