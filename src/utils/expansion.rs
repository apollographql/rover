//! The code in this file is borrowed from the router for consistent syntax. As such, it is covered
//! by the [ELv2 license](https://www.apollographql.com/docs/resources/elastic-license-v2-faq/).
//! Before calling this code from other functions, make sure that the license is accepted (like
//! `supergraph compose`)
use anyhow::{anyhow, bail, Context, Error};
use std::env;
use std::path::Path;

use rover_std::Fs;
use shellexpand::env_with_context;

use crate::RoverResult;

/// Implements router-config-style
/// [variable expansion](https://www.apollographql.com/docs/router/configuration/overview/#variable-expansion)
/// for use in Rover configs (like `supergraph.yaml`).
pub(crate) fn expand(value: &str) -> RoverResult<String> {
    env_with_context(value, context)
        .map_err(|e| anyhow!(e).context("While expanding variables").into())
        .map(|cow| cow.into_owned())
}

fn context(key: &str) -> Result<Option<String>, Error> {
    if let Some(env_var_key) = key.strip_prefix("env.") {
        env::var(env_var_key).map(Some).with_context(|| {
            format!(
                "While reading env var {} for variable expansion",
                env_var_key
            )
        })
    } else if let Some(file_name) = key.strip_prefix("file.") {
        if !Path::new(file_name).exists() {
            Ok(None)
        } else {
            Fs::read_file(file_name).map(Some)
        }
    } else {
        bail!("Invalid variable expansion key: {}", key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::fixture::{FileWriteBin, FileWriteStr, NamedTempFile};

    // Env vars are global, so if you're going to reuse them you'd better make them constants
    // These point at each other for testing nested values
    const ENV_VAR_KEY_1: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_1";
    const ENV_VAR_VALUE_1: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_2";
    const ENV_VAR_KEY_2: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_2";
    const ENV_VAR_VALUE_2: &str = "RESOLVE_HEADER_VALUE_TEST_VAR_1";

    #[test]
    fn valid_env_var() {
        let value = format!("${{env.{}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        assert_eq!(expand(&value).unwrap(), ENV_VAR_VALUE_1);
    }

    #[test]
    fn partial_env_var_partial_static() {
        let value = format!("static-part-${{env.{}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        assert_eq!(
            expand(&value).unwrap(),
            format!("static-part-{}", ENV_VAR_VALUE_1)
        );
    }

    #[test]
    fn multiple_env_vars() {
        let value = format!(
            "${{env.{}}}-static-part-${{env.{}}}",
            ENV_VAR_KEY_1, ENV_VAR_KEY_2
        );
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        env::set_var(ENV_VAR_KEY_2, ENV_VAR_VALUE_2);
        assert_eq!(
            expand(&value).unwrap(),
            format!("{}-static-part-{}", ENV_VAR_VALUE_1, ENV_VAR_VALUE_2)
        );
    }

    #[test]
    fn nested_env_vars() {
        let value = format!("${{env.${{env.{}}}}}", ENV_VAR_KEY_1);
        env::set_var(ENV_VAR_KEY_1, ENV_VAR_VALUE_1);
        env::set_var(ENV_VAR_KEY_2, ENV_VAR_VALUE_2);
        assert!(expand(&value).is_err());
    }

    #[test]
    fn not_env_var() {
        let value = "test_value";
        assert_eq!(expand(value).unwrap(), value);
    }

    #[test]
    fn env_var_not_found() {
        let value = "${env.RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST}";
        assert!(expand(value).is_err());
    }

    #[test]
    fn missing_end_brace() {
        let value = "${env.RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST";
        assert_eq!(expand(value).unwrap(), value);
    }

    #[test]
    fn missing_start_section() {
        let value = "RESOLVE_HEADER_VALUE_TEST_VAR_DOES_NOT_EXIST}";
        assert_eq!(expand(value).unwrap(), value);
    }

    #[test]
    fn content_from_file() {
        let temp = NamedTempFile::new("variable.txt").unwrap();
        temp.write_str("test_value").unwrap();
        let value = format!("${{file.{}}}", temp.path().to_str().unwrap());
        assert_eq!(expand(&value).unwrap(), "test_value");
    }

    /// This behavior is copied from Router
    #[test]
    fn missing_file() {
        let value = "${file.afilethatdefinitelydoesntexisthere}";
        assert_eq!(expand(value).unwrap(), value);
    }

    #[test]
    fn invalid_file() {
        let temp = NamedTempFile::new("variable.txt").unwrap();
        // Invalid UTF-8
        temp.write_binary(&[0x80]).unwrap();
        let value = format!("${{file.{}}}", temp.path().to_str().unwrap());
        assert!(expand(&value).is_err());
    }
}
