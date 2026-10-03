//! `rover`'s own long help keeps its description. A flattened options
//! struct's doc comment would otherwise replace it.

use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use speculoos::prelude::*;

#[rstest]
#[case::help_flag(&["--help"])]
#[case::help_subcommand(&["help"])]
fn long_help_opens_with_rovers_description(#[case] args: &[&str]) {
    let output = cargo_bin_cmd!("rover")
        .args(args)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    assert_that!(output.status.code()).is_equal_to(Some(0));
    let help = String::from_utf8_lossy(&output.stdout);
    // Help names the binary it was run as, which is `rover.exe` on Windows.
    let help = help.replacen("Usage: rover.exe ", "Usage: rover ", 1);
    let (description, _) = help.split_once("\nUsage: ").expect("help has a usage line");
    assert_that!(description).is_equal_to("Rover - Your Graph Companion\n");
}
