use std::collections::HashMap;

use camino::Utf8PathBuf;
use rstest::rstest;
use speculoos::prelude::*;

use crate::command::init::helpers::home_dir_from;

const UNIX_ERROR: &str = "Could not determine home directory from HOME environment variable";
const WINDOWS_ERROR: &str = "Could not determine home directory on Windows";

/// An environment lookup over `vars`, standing in for `std::env::var`.
fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let vars: HashMap<String, String> = vars
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    move |key| vars.get(key).cloned()
}

#[rstest]
#[case::unix_home(false, &[("HOME", "/home/me")], "/home/me")]
#[case::unix_ignores_windows_vars(
    false,
    &[("HOME", "/home/me"), ("USERPROFILE", r"C:\Users\other")],
    "/home/me"
)]
#[case::windows_userprofile(true, &[("USERPROFILE", r"C:\Users\me")], r"C:\Users\me")]
#[case::windows_userprofile_wins_over_homedrive(
    true,
    &[("USERPROFILE", r"C:\Users\me"), ("HOMEDRIVE", "D:"), ("HOMEPATH", r"\other")],
    r"C:\Users\me"
)]
#[case::windows_homedrive_and_homepath(
    true,
    &[("HOMEDRIVE", "C:"), ("HOMEPATH", r"\Users\me")],
    r"C:\Users\me"
)]
#[case::windows_ignores_home(
    true,
    &[("HOME", "/home/me"), ("USERPROFILE", r"C:\Users\me")],
    r"C:\Users\me"
)]
fn home_dir_from_finds_home(
    #[case] windows: bool,
    #[case] vars: &[(&str, &str)],
    #[case] expected: &str,
) {
    let home_dir = home_dir_from(env(vars), windows);

    assert_that!(home_dir)
        .is_ok()
        .is_equal_to(Utf8PathBuf::from(expected));
}

#[rstest]
#[case::unix_no_home(false, &[], UNIX_ERROR)]
#[case::unix_only_windows_vars(false, &[("USERPROFILE", r"C:\Users\me")], UNIX_ERROR)]
#[case::windows_nothing_set(true, &[], WINDOWS_ERROR)]
#[case::windows_homedrive_without_homepath(true, &[("HOMEDRIVE", "C:")], WINDOWS_ERROR)]
#[case::windows_homepath_without_homedrive(true, &[("HOMEPATH", r"\Users\me")], WINDOWS_ERROR)]
#[case::windows_only_home(true, &[("HOME", "/home/me")], WINDOWS_ERROR)]
fn home_dir_from_errors_when_no_home(
    #[case] windows: bool,
    #[case] vars: &[(&str, &str)],
    #[case] expected_message: &str,
) {
    let result = home_dir_from(env(vars), windows);

    let err = assert_that!(result).is_err().subject;
    assert_that!(err.message()).is_equal_to(expected_message.to_string());
}
