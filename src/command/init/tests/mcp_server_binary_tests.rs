use std::env::consts::EXE_SUFFIX;

use camino::{Utf8Path, Utf8PathBuf};
use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::helpers::mcp_server_binary;

#[rstest]
#[case::home_only(None, Some("/home/me"), "/home/me/.rover/bin")]
#[case::override_wins(Some("/opt/apollo"), Some("/home/me"), "/opt/apollo/.rover/bin")]
#[case::empty_override_is_unset(Some(""), Some("/home/me"), "/home/me/.rover/bin")]
#[case::override_without_home(Some("/opt/apollo"), None, "/opt/apollo/.rover/bin")]
fn mcp_server_binary_is_in_rovers_bin_dir(
    #[case] rover_home: Option<&str>,
    #[case] home: Option<&str>,
    #[case] bin_dir: &str,
) {
    let binary = mcp_server_binary(rover_home.map(Utf8Path::new), home.map(Utf8Path::new));

    assert_that!(binary).is_ok().is_equal_to(
        Utf8PathBuf::from(bin_dir).join(format!("apollo-mcp-server-v0.8.0{EXE_SUFFIX}")),
    );
}

#[rstest]
#[case::nothing_set(None, None)]
#[case::empty_override_without_home(Some(""), None)]
#[case::empty_home(None, Some(""))]
fn mcp_server_binary_errors_without_a_usable_directory(
    #[case] rover_home: Option<&str>,
    #[case] home: Option<&str>,
) {
    let result = mcp_server_binary(rover_home.map(Utf8Path::new), home.map(Utf8Path::new));

    let err = assert_that!(result).is_err().subject;
    assert_that!(err.message())
        .is_equal_to("Could not find a home directory for the MCP server binary.".to_string());
    let suggestions: Vec<String> = err.suggestions().iter().map(ToString::to_string).collect();
    assert_that!(suggestions).is_equal_to(vec![
        "Set `APOLLO_HOME` or pass `--rover-home <DIR>` to choose where Rover keeps its plugins."
            .to_string(),
    ]);
}
