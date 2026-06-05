use std::fmt;

use crate::style::{Style, StyledText};

pub mod stderr;
pub mod stdout;
mod term;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use term::Term;

#[cfg_attr(any(test, feature = "testing"), mockall::automock)]
pub trait Print {
    /// Print a single styled message, followed by a newline.
    fn print(&self, message: &StyledText) -> std::io::Result<()>;

    /// Print a line composed of multiple styled segments, followed by a
    /// single newline. Each segment is rendered independently, so a line can
    /// mix styles (e.g. a colored prefix followed by plain text).
    fn print_line(&self, segments: &[StyledText]) -> std::io::Result<()>;

    /// Render styled text to a string the way this printer would emit it.
    /// Use this to build a line with inline styled tokens before printing
    /// (e.g. via [`PrintExt::paint`]).
    fn render(&self, text: &StyledText) -> String;
}

/// Ergonomic helpers layered on top of [`Print`]. Provided for free to every
/// `Print` implementor via the blanket impl below, so the methods live in one
/// place rather than being duplicated across stream wrappers.
pub trait PrintExt: Print {
    /// Render a single styled token to a string for interpolation into a
    /// larger line, e.g. `format!("run {}", p.paint(Style::Command, cmd))`.
    fn paint(&self, style: Style, value: impl AsRef<str>) -> String {
        self.render(&StyledText::new(style, value.as_ref()))
    }

    /// Print an informational line, prefixed with a styled `==>`.
    fn infoln(&self, message: impl fmt::Display) -> std::io::Result<()> {
        self.print(&StyledText::plain(format!(
            "{} {message}",
            self.paint(Style::Info, "==>")
        )))
    }

    /// Print a warning line, prefixed with a styled `warning:`.
    fn warnln(&self, message: impl fmt::Display) -> std::io::Result<()> {
        self.print(&StyledText::plain(format!(
            "{} {message}",
            self.paint(Style::Warning, "warning:")
        )))
    }

    /// Print an error line, prefixed with a styled `error:`.
    fn errln(&self, message: impl fmt::Display) -> std::io::Result<()> {
        self.print(&StyledText::plain(format!(
            "{} {message}",
            self.paint(Style::Error, "error:")
        )))
    }

    /// Print a success line, prefixed with a styled `✓`.
    fn successln(&self, message: impl fmt::Display) -> std::io::Result<()> {
        self.print(&StyledText::plain(format!(
            "{} {message}",
            self.paint(Style::Success, "✓")
        )))
    }
}

impl<T: Print + ?Sized> PrintExt for T {}

/// Whether Apollo's `APOLLO_NO_COLOR` opt-out is set. Mirrors `rover-std`'s
/// truthiness: unset / empty / "0" / "false" all count as *not* set.
fn is_apollo_no_color_set() -> bool {
    !matches!(
        std::env::var("APOLLO_NO_COLOR").as_deref(),
        Err(..) | Ok("") | Ok("0") | Ok("false") | Ok("False") | Ok("FALSE")
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use rstest::rstest;
    use sealed_test::prelude::*;
    use speculoos::prelude::*;

    use super::*;

    // `is_apollo_no_color_set` reads a process-global env var, so each case runs in an
    // isolated process via `sealed_test` to avoid cross-test interference.

    #[sealed_test]
    fn apollo_no_color_is_false_when_unset() {
        // SAFETY: `sealed_test` runs this in a dedicated process, so mutating
        // the environment cannot race with or leak into other tests.
        unsafe { std::env::remove_var("APOLLO_NO_COLOR") };

        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "")])]
    fn apollo_no_color_is_false_when_empty() {
        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "0")])]
    fn apollo_no_color_is_false_when_zero() {
        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "false")])]
    fn apollo_no_color_is_false_when_false_lowercase() {
        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "False")])]
    fn apollo_no_color_is_false_when_false_titlecase() {
        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "FALSE")])]
    fn apollo_no_color_is_false_when_false_uppercase() {
        assert_that!(&is_apollo_no_color_set()).is_false();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "1")])]
    fn apollo_no_color_is_true_when_one() {
        assert_that!(&is_apollo_no_color_set()).is_true();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "true")])]
    fn apollo_no_color_is_true_when_true() {
        assert_that!(&is_apollo_no_color_set()).is_true();
    }

    // Only the lower/title/upper `false` spellings are treated as falsy, so
    // `TRUE` (and any other non-empty value) enables the opt-out.
    #[sealed_test(env = [("APOLLO_NO_COLOR", "TRUE")])]
    fn apollo_no_color_is_true_when_true_uppercase() {
        assert_that!(&is_apollo_no_color_set()).is_true();
    }

    #[sealed_test(env = [("APOLLO_NO_COLOR", "anything-else")])]
    fn apollo_no_color_is_true_for_arbitrary_value() {
        assert_that!(&is_apollo_no_color_set()).is_true();
    }

    // Prefix helpers route through `paint` (→ `render`) for the prefix and
    // hand the assembled line to `print`. We can't capture a real terminal, so
    // we render the prefix verbatim and capture the line passed to `print` to
    // assert on its exact contents.
    fn info(p: &MockPrint) -> std::io::Result<()> {
        p.infoln("a thing happened")
    }
    fn warn(p: &MockPrint) -> std::io::Result<()> {
        p.warnln("a thing happened")
    }
    fn error(p: &MockPrint) -> std::io::Result<()> {
        p.errln("a thing happened")
    }
    fn success(p: &MockPrint) -> std::io::Result<()> {
        p.successln("a thing happened")
    }

    #[rstest]
    #[case::info(info, "==> a thing happened")]
    #[case::warn(warn, "warning: a thing happened")]
    #[case::error(error, "error: a thing happened")]
    #[case::success(success, "✓ a thing happened")]
    fn prefix_helpers_compose_prefix_and_message(
        #[case] call: fn(&MockPrint) -> std::io::Result<()>,
        #[case] expected_line: &'static str,
    ) {
        let printed = Arc::new(Mutex::new(None::<String>));
        let mut mock = MockPrint::new();
        // render the prefix token verbatim (color is covered by integration tests)
        mock.expect_render()
            .returning(|text| text.text().to_string());
        let sink = Arc::clone(&printed);
        mock.expect_print().times(1).returning(move |message| {
            *sink.lock().unwrap() = Some(message.text().to_string());
            Ok(())
        });

        let result = call(&mock);

        assert_that!(&result).is_ok();
        let printed = printed.lock().unwrap();
        assert_that!(&printed.as_deref()).is_equal_to(Some(expected_line));
    }

    #[rstest]
    fn print_forwards_a_single_styled_message() {
        let printed = Arc::new(Mutex::new(None::<String>));
        let mut mock = MockPrint::new();
        let sink = Arc::clone(&printed);
        mock.expect_print().times(1).returning(move |message| {
            *sink.lock().unwrap() = Some(message.text().to_string());
            Ok(())
        });

        let result = mock.print(&StyledText::plain("hello"));

        assert_that!(&result).is_ok();
        let printed = printed.lock().unwrap();
        assert_that!(&printed.as_deref()).is_equal_to(Some("hello"));
    }

    #[rstest]
    fn print_line_receives_all_segments_in_order() {
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        let mut mock = MockPrint::new();
        let sink = Arc::clone(&captured);
        mock.expect_print_line()
            .times(1)
            .returning(move |segments| {
                *sink.lock().unwrap() = segments.iter().map(|s| s.text().to_string()).collect();
                Ok(())
            });

        let segments = [
            StyledText::plain("run "),
            StyledText::new(Style::Command, "rover dev"),
            StyledText::plain(" now"),
        ];
        let result = mock.print_line(&segments);

        assert_that!(&result).is_ok();
        let captured = captured.lock().unwrap();
        assert_that!(&*captured).is_equal_to(vec![
            "run ".to_string(),
            "rover dev".to_string(),
            " now".to_string(),
        ]);
    }
}
