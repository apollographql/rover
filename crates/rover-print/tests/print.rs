//! Integration tests showcasing `rover-print`'s public API and behavior.
//!
//! Printers are built with module-level constructors:
//!   - `stdout::term(bool)` / `stderr::term(bool)` — explicit color on/off
//!   - `stdout::default()` / `stderr::default()` — color auto-detected from the
//!     environment (`NO_COLOR` / `CLICOLOR` / TTY, plus `APOLLO_NO_COLOR`)
//!   - `stdout::mock()` / `stderr::mock()` — a recording mock for assertions
//!     (requires the `testing` feature; exercised by the crate's unit tests)
//!
//! `term` renders through `force_styling(with_color)`, so `term(true)` always
//! emits color and `term(false)` never does — independent of whether the test
//! runs under a TTY, which keeps the color assertions below deterministic in
//! CI. `default()` auto-detects color, so only stream-independent properties
//! (e.g. unstyled text round-tripping verbatim) are asserted for it.

use rover_print::{
    print::{Print, PrintExt},
    stderr, stdout,
    style::{Style, StyledText},
};
use rstest::rstest;
use speculoos::prelude::*;

/// The ANSI escape introducer present in any colored output.
const ANSI: &str = "\u{1b}[";

#[rstest]
#[case::command(Style::Command)]
#[case::link(Style::Link)]
#[case::error(Style::Error)]
#[case::heading(Style::Heading)]
#[case::none(Style::None)]
fn render_without_color_is_verbatim(#[case] style: Style) {
    let printer = stderr::term(false);

    let rendered = printer.render(&StyledText::new(style, "hello world"));

    assert_that!(&rendered).is_equal_to("hello world".to_string());
}

#[rstest]
#[case::command(Style::Command)]
#[case::link(Style::Link)]
#[case::error(Style::Error)]
#[case::heading(Style::Heading)]
fn render_with_color_wraps_text_in_ansi(#[case] style: Style) {
    let printer = stderr::term(true);

    let rendered = printer.render(&StyledText::new(style, "hello world"));

    assert_that!(&rendered).contains("hello world");
    assert_that!(&rendered).contains(ANSI);
}

#[rstest]
fn command_style_renders_cyan() {
    let printer = stdout::term(true);

    let rendered = printer.paint(Style::Command, "rover dev");

    // cyan foreground is ANSI code 36
    assert_that!(&rendered).contains("\u{1b}[36m");
    assert_that!(&rendered).contains("rover dev");
}

#[rstest]
fn none_style_is_plain_even_with_color_enabled() {
    let printer = stdout::term(true);

    let rendered = printer.render(&StyledText::plain("just text"));

    assert_that!(&rendered).is_equal_to("just text".to_string());
}

/// The dominant rover pattern: styled tokens interpolated into a sentence.
#[rstest]
fn inline_tokens_compose_into_a_sentence_without_color() {
    let printer = stderr::term(false);

    let line = format!(
        "run {} or {}",
        printer.paint(Style::Command, "rover graph publish"),
        printer.paint(Style::Command, "rover subgraph publish"),
    );

    assert_that!(&line)
        .is_equal_to("run rover graph publish or rover subgraph publish".to_string());
}

#[rstest]
fn inline_tokens_each_get_colored_with_color() {
    let printer = stderr::term(true);

    let line = format!(
        "run {} or {}",
        printer.paint(Style::Command, "rover graph publish"),
        printer.paint(Style::Command, "rover subgraph publish"),
    );

    // both tokens individually wrapped in cyan, surrounding prose left plain
    assert_that!(&line.matches("\u{1b}[36m").count()).is_equal_to(2);
    assert_that!(&line).starts_with("run ");
}

/// `Stdout` and `Stderr` wrap the same `Term` rendering, so a given style
/// renders identically through either stream.
#[rstest]
#[case::with_color(true)]
#[case::without_color(false)]
fn stdout_and_stderr_render_identically(#[case] with_color: bool) {
    let token = StyledText::new(Style::Link, "https://apollographql.com");

    let via_stdout = stdout::term(with_color).render(&token);
    let via_stderr = stderr::term(with_color).render(&token);

    assert_that!(&via_stdout).is_equal_to(&via_stderr);
}

/// `default()` auto-detects color from the environment, so we don't assert on
/// ANSI here; instead we exercise the constructor and the stream-independent
/// guarantee that unstyled text round-trips verbatim regardless of that
/// decision.
#[rstest]
fn default_constructors_render_unstyled_text_verbatim() {
    let out = stdout::default();
    let err = stderr::default();

    assert_that!(&out.render(&StyledText::plain("plain text")))
        .is_equal_to("plain text".to_string());
    assert_that!(&err.render(&StyledText::plain("plain text")))
        .is_equal_to("plain text".to_string());
}

// --- PrintExt helpers ------------------------------------------------------

/// `PrintExt::paint` renders a single styled token for interpolation into a
/// larger line, honoring the printer's color setting: ANSI when on, raw text
/// when off.
#[rstest]
fn printext_paint_renders_a_styled_token() {
    let colored = stdout::term(true);
    let plain = stdout::term(false);

    assert_that!(&colored.paint(Style::Command, "rover dev")).contains(ANSI);
    assert_that!(&plain.paint(Style::Command, "rover dev")).is_equal_to("rover dev".to_string());
}

// The `PrintExt` prefix helpers (`infoln`/`warnln`/`errln`/`successln`) write to
// a real terminal, so their composed output is asserted via the `TerminalCapture`
// recorder in `tests/printext.rs` (which requires the `testing` feature).
