//! A Federation 1 pin is refused with the Federation 1 message from every place a version can be
//! written, however it is spelled. `=1.0.0` in particular isn't a version the supergraph
//! plugin's own parser accepts, and used to be dropped with the rest of the config, leaving the
//! composition to carry on silently on Federation 2.

use assert_cmd::Command;
use rstest::rstest;
use speculoos::prelude::*;

const REFUSED: &str = "Federation 1 is no longer supported by Rover";

fn compose(yaml_version: &str, flags: &[&str]) -> (Option<i32>, String) {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("supergraph.yaml"),
        format!(
            "federation_version: {yaml_version}\nsubgraphs:\n  users:\n    routing_url: http://localhost:4001\n    schema:\n      sdl: \"type Query {{ a: Int }}\"\n"
        ),
    )
    .unwrap();
    let output = Command::cargo_bin("rover")
        .unwrap()
        .current_dir(temp.path())
        .args(["supergraph", "compose", "--config", "supergraph.yaml"])
        .args([
            "--skip-update",
            "--skip-update-check",
            "--telemetry-disabled",
        ])
        .args(flags)
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[rstest]
#[case::exact_one("=1.0.0")]
#[case::exact_zero("=0.36.0")]
#[case::bare_one("1")]
#[case::latest_one("latest-1")]
fn a_federation_one_pin_in_the_config_is_refused(#[case] pin: &str) {
    let (code, stderr) = compose(pin, &[]);

    assert_that!(code).is_equal_to(Some(1));
    assert_that!(stderr.contains(REFUSED))
        .named(&stderr)
        .is_true();
}

#[rstest]
#[case::exact_one("=1.0.0")]
#[case::v_prefixed("v1.2.3")]
#[case::exact_zero("=0.10.0")]
fn a_federation_one_pin_on_the_flag_is_refused(#[case] pin: &str) {
    let (code, stderr) = compose("2", &["--federation-version", pin]);

    assert_that!(code).is_equal_to(Some(2));
    assert_that!(stderr.contains(REFUSED))
        .named(&stderr)
        .is_true();
}

#[rstest]
#[case::another_major("=3.0.0")]
#[case::not_a_version("banana")]
fn an_unsupported_flag_value_no_longer_offers_one(#[case] value: &str) {
    let (code, stderr) = compose("2", &["--federation-version", value]);

    assert_that!(code).is_equal_to(Some(2));
    assert_that!(stderr.contains(&format!(
        "Specified version `{value}` is not supported. You can specify '2', or a fully qualified version prefixed with an '=', like: =2.0.0"
    )))
    .named(&stderr)
    .is_true();
    assert_that!(stderr.contains("'1', '2'"))
        .named(&stderr)
        .is_false();
}
