use thiserror::Error;

/// Renders `err` followed by every error in its `source` chain, joined with `": "`.
///
/// `RoverError` renders the chain itself, via anyhow's `Debug` impl. Call sites that print an
/// error directly — `errln!`, an LSP diagnostic, a message flattened into a `String` field —
/// get only the outermost message, so they need this to keep the cause.
///
/// This is what `anyhow::Error`'s alternate `Display` (`{:#}`) produces, but it takes a plain
/// `std::error::Error` — most of these call sites hold a concrete error rather than an
/// `anyhow::Error`, and some of them hold one that isn't `Sync` and so can't become one.
pub fn format_error_chain(err: &dyn std::error::Error) -> String {
    let mut rendered = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        rendered.push_str(": ");
        rendered.push_str(&cause.to_string());
        source = cause.source();
    }
    rendered
}

#[derive(Error, Debug)]
pub enum RoverStdError {
    /// AdhocError comes from the anyhow crate
    #[error(transparent)]
    AdhocError(#[from] anyhow::Error),
    /// This error is thrown when there is an error watching a file
    #[error("an unexpected error occured while watching for changes")]
    Notify(#[from] notify::Error),
    /// This error is thrown when there is an empty file
    #[error("\"{empty_file}\" is an empty file.")]
    EmptyFile {
        /// The empty file path
        empty_file: String,
    },
    /// This error is thrown when a watched file is removed
    #[error("\"{file}\" has been removed.")]
    FileRemoved {
        /// The empty file path
        file: String,
    },
    #[error("unable to find dependency: \"{err}\"")]
    MissingDependency {
        /// The error while attempting to find the dependency
        err: String,
    },
    #[error("ELV2 license must be accepted")]
    LicenseNotAccepted,
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;
    use thiserror::Error;

    use super::format_error_chain;

    #[derive(Error, Debug)]
    #[error("the outermost failure")]
    struct Outer(#[source] Middle);

    #[derive(Error, Debug)]
    #[error("what the outermost failure was caused by")]
    struct Middle(#[source] Innermost);

    #[derive(Error, Debug)]
    #[error("the root cause")]
    struct Innermost;

    #[test]
    fn an_error_without_a_source_renders_as_itself() {
        assert_that!(format_error_chain(&Innermost)).is_equal_to("the root cause".to_string());
    }

    #[test]
    fn every_error_in_the_chain_is_rendered_once_in_order() {
        assert_that!(format_error_chain(&Outer(Middle(Innermost)))).is_equal_to(
            "the outermost failure: what the outermost failure was caused by: the root cause"
                .to_string(),
        );
    }
}
