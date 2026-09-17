//! Proving that a run made no attempt to reach the network.
//!
//! An offline assertion is only worth as much as the environment it runs in. A
//! test that says "this works without the network" passes trivially on a
//! machine with a working network, and a test pointed at an unreachable host
//! proves only that the connection failed — the attempt still happened. So
//! this module does two things: it refuses to let a test pass outside an
//! environment that genuinely has no route out, and it records what a command
//! tried to do with a socket so a test can assert the count was zero.
//!
//! Both are Linux-only, and run in the network-denied CI job.

use std::{
    net::{SocketAddr, TcpStream},
    process::{Command, Output},
    time::Duration,
};

/// Set by the CI job that runs inside a network namespace with no route out.
const DENIED_ENV_VAR: &str = "ROVER_TEST_NETWORK_DENIED";

/// An address no test environment should be able to reach, used to check the
/// claim the environment makes about itself.
const UNREACHABLE_PROBE: &str = "1.1.1.1:443";

/// Fail unless this process really has no way out.
///
/// Call this first in any test that asserts offline behavior. Without it, the
/// test would pass on a developer's machine for the wrong reason.
pub fn require_network_denied() {
    assert!(
        std::env::var(DENIED_ENV_VAR).is_ok(),
        "this test asserts offline behavior and must run in the network-denied job, which sets \
         {DENIED_ENV_VAR}"
    );

    let probe: SocketAddr = UNREACHABLE_PROBE
        .parse()
        .expect("the probe address is malformed");

    assert!(
        TcpStream::connect_timeout(&probe, Duration::from_secs(2)).is_err(),
        "{DENIED_ENV_VAR} is set but {UNREACHABLE_PROBE} answered, so the network is not actually \
         denied and an offline assertion here would prove nothing"
    );
}

/// What a command did that could only be for reaching the network.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OutboundActivity {
    /// Internet sockets the command opened. Counted separately from the
    /// connections below because name resolution fails before `connect` when
    /// there is no route out: a run that tried to reach the registry and got
    /// no further than DNS still opened one of these, and that is the evidence
    /// that it tried.
    pub sockets: usize,
    /// Addresses it called `connect` on, loopback excluded.
    pub connections: Vec<String>,
}

impl OutboundActivity {
    /// Whether the command made no attempt to reach the network at all.
    pub const fn is_silent(&self) -> bool {
        self.sockets == 0 && self.connections.is_empty()
    }
}

/// Run a command under `strace` and report what it did with sockets.
///
/// # Panics
///
/// On anything but Linux, and if `strace` is missing. The job that runs these
/// tests installs it; failing loudly is better than reporting silence because
/// nothing was watching.
pub fn record_outbound(command: &mut Command) -> (Output, OutboundActivity) {
    assert_eq!(
        std::env::consts::OS,
        "linux",
        "recording outbound activity needs strace, so it only runs on Linux"
    );

    let log = tempfile::NamedTempFile::new().expect("could not create the strace log");

    let program = command.get_program().to_owned();
    let args: Vec<_> = command.get_args().map(ToOwned::to_owned).collect();

    let mut traced = Command::new("strace");
    traced
        .args(["-f", "-qq", "-e", "trace=socket,connect", "-o"])
        .arg(log.path())
        .arg(&program)
        .args(&args);

    for (key, value) in command.get_envs() {
        match value {
            Some(value) => traced.env(key, value),
            None => traced.env_remove(key),
        };
    }
    if let Some(dir) = command.get_current_dir() {
        traced.current_dir(dir);
    }

    let output = traced.output().expect(
        "could not run strace — the network-denied job installs it, and without it this would \
         report silence because nothing was watching",
    );

    let trace = std::fs::read_to_string(log.path()).expect("could not read the strace log");

    (output, parse_trace(&trace))
}

/// Pull the internet sockets and non-loopback connections out of an strace log.
fn parse_trace(trace: &str) -> OutboundActivity {
    let mut activity = OutboundActivity::default();

    for line in trace.lines() {
        if line.contains("socket(AF_INET") {
            activity.sockets += 1;
        }

        if line.contains("connect(")
            && let Some(address) = inet_address(line)
            && !is_loopback(&address)
        {
            activity.connections.push(address);
        }
    }

    activity
}

/// The address out of a `connect` line, for internet addresses only. Unix
/// sockets are how a process talks to things on its own machine, which is not
/// what this module is looking for.
fn inet_address(line: &str) -> Option<String> {
    let start = line
        .find("inet_addr(\"")
        .map(|at| at + "inet_addr(\"".len())
        .or_else(|| {
            line.find("inet6_addr(\"")
                .map(|at| at + "inet6_addr(\"".len())
        })?;
    let rest = &line[start..];
    let end = rest.find('"')?;

    Some(rest[..end].to_string())
}

fn is_loopback(address: &str) -> bool {
    address.starts_with("127.") || address == "::1"
}

#[cfg(test)]
mod tests {
    use assert_cmd::cargo::cargo_bin;
    use speculoos::prelude::*;

    use super::*;

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
}
