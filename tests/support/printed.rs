//! How a `RoverError` reads on the terminal, independent of `RUST_BACKTRACE`.

use rover::RoverError;

/// The error exactly as `rover` prints it, uncoloured.
///
/// With `RUST_BACKTRACE` set, as it is in CI, anyhow appends a
/// `Stack backtrace:` block after the causes. That comes from the
/// environment, not the error, so it's cut out here. The block runs up to the
/// first suggestion, which is the only line indented by exactly eight spaces:
/// frame lines are indented less, and their `at` lines more. This mirrors the
/// library's own test helper, which integration tests can't reach.
pub fn printed(error: RoverError) -> String {
    let printed = console::strip_ansi_codes(&error.to_string()).into_owned();
    let Some(start) = printed.find("\n\nStack backtrace:\n") else {
        return printed;
    };
    let is_suggestion = |line: &&str| {
        line.strip_prefix("        ")
            .is_some_and(|text| !text.starts_with(' '))
    };
    let suggestions = printed[start + 2..]
        .split_inclusive('\n')
        .skip_while(|line| !is_suggestion(line))
        .collect::<String>();
    format!("{}\n{suggestions}", &printed[..start])
}
