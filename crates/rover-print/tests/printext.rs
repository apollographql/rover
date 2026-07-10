//! Integration tests for the `PrintExt` prefix helpers, using the `testing`
//! feature's `TerminalCapture` recorder to assert the exact emitted lines (a real
//! terminal printer can't be captured in-process).
//!
//! Gated on the `testing` feature: without it, `print::testing` isn't compiled,
//! so this file becomes an empty test crate. (`test` is always set for an
//! integration test crate, so the gate is `feature` only — not `any(test, …)`.)
#![cfg(feature = "testing")]

use rover_print::print::{PrintExt, testing::TerminalCapture};
use rstest::rstest;
use speculoos::prelude::*;

/// The ANSI escape introducer present in any colored output.
const ANSI: &str = "\u{1b}[";

/// With color off, each prefix helper emits exactly `"<prefix> <message>"`.
#[rstest]
fn prefix_helpers_compose_prefix_and_message() {
    let printer = TerminalCapture::new(false);

    printer
        .infoln("watching supergraph.yaml for changes")
        .unwrap();
    printer
        .warnln("using an unpinned federation version")
        .unwrap();
    printer.errln("could not parse supergraph config").unwrap();
    printer.successln("composition succeeded").unwrap();

    assert_that!(&printer.lines()).is_equal_to(vec![
        "==> watching supergraph.yaml for changes".to_string(),
        "warning: using an unpinned federation version".to_string(),
        "error: could not parse supergraph config".to_string(),
        "✓ composition succeeded".to_string(),
    ]);
}

fn info(printer: &TerminalCapture, message: &str) -> std::io::Result<()> {
    printer.infoln(message)
}
fn warn(printer: &TerminalCapture, message: &str) -> std::io::Result<()> {
    printer.warnln(message)
}
fn error(printer: &TerminalCapture, message: &str) -> std::io::Result<()> {
    printer.errln(message)
}
fn success(printer: &TerminalCapture, message: &str) -> std::io::Result<()> {
    printer.successln(message)
}

/// With color on, each prefix helper colors its prefix, so the emitted line
/// carries ANSI styling while still containing the prefix marker and message.
#[rstest]
#[case::infoln(info, "==>")]
#[case::warnln(warn, "warning:")]
#[case::errln(error, "error:")]
#[case::successln(success, "✓")]
fn prefix_helpers_color_the_prefix_when_enabled(
    #[case] call: fn(&TerminalCapture, &str) -> std::io::Result<()>,
    #[case] prefix: &str,
) {
    let printer = TerminalCapture::new(true);

    call(&printer, "the message").unwrap();

    let line = printer.lines().pop().expect("a line was printed");
    assert_that!(&line).contains(ANSI);
    assert_that!(&line).contains(prefix);
    assert_that!(&line).contains("the message");
}
