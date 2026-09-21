//! Its own test target so the network-denied CI job has one direct thing to
//! invoke, rather than filtering the shared `main` binary down to a module by
//! name. See `support::network` for what these tests are proving.

mod support;

use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use speculoos::prelude::*;
use support::network::{OutboundActivity, record_outbound, require_network_denied};

#[test]
#[ignore = "runs in the network-denied job"]
fn the_environment_really_has_no_route_out() {
    require_network_denied();
}

#[test]
#[ignore = "runs in the network-denied job"]
fn a_command_that_reaches_out_is_recorded() {
    require_network_denied();

    // The control for the recorder itself. Without it, a test asserting
    // silence could be passing because the recorder sees nothing ever.
    // Connecting to a literal address rather than a name, so that the
    // attempt is a `connect` and not a name lookup that fails first.
    let mut command = Command::new("bash");
    command.args(["-c", &format!("exec 3<>/dev/tcp/{}", "1.1.1.1/443")]);

    let (_, activity) = record_outbound(&mut command);

    assert_that!(activity.is_silent()).is_false();
    assert_that!(activity.connections).contains("1.1.1.1".to_string());
}

#[test]
#[ignore = "runs in the network-denied job"]
fn printing_a_version_touches_no_socket() {
    require_network_denied();

    let mut command = Command::new(cargo_bin("rover"));
    command.arg("--version");

    let (output, activity) = record_outbound(&mut command);

    assert_that!(output.status.success()).is_true();
    assert_that!(activity).is_equal_to(OutboundActivity::default());
}
