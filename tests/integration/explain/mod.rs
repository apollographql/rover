use assert_cmd::cargo::cargo_bin_cmd;
use rstest::rstest;
use speculoos::prelude::*;

/// `rover explain` renders markdown, which is styled with ANSI escapes unless
/// color is off. A piped stdout is as good as `NO_COLOR` for asking for none.
#[rstest]
#[case::piped_stdout(&[], &[])]
#[case::no_color_env(&[], &[("NO_COLOR", "1")])]
#[case::apollo_no_color_env(&[], &[("APOLLO_NO_COLOR", "1")])]
#[case::no_color_flag(&["--no-color"], &[])]
fn explain_writes_no_escape_sequences_when_color_is_off(
    #[case] flags: &[&str],
    #[case] env: &[(&str, &str)],
) {
    let output = cargo_bin_cmd!("rover")
        .args(["explain", "E058"])
        .args(flags)
        .envs(env.iter().copied())
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_that!(output.status.code()).is_equal_to(Some(0));
    assert_that!(stdout.starts_with("E058\n")).is_true();
    assert_that!(stdout.contains('\u{1b}'))
        .named(&stdout)
        .is_false();
}
