//! FR4: telling a user that a version spelling is deprecated, and doing it
//! once.

use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use rover_print::print::{Print, PrintExt};

use super::version::DeprecatedSpelling;

/// Which deprecated spellings have already been reported during this run.
///
/// A single command can read the same version from several places — a flag,
/// an environment variable, a manifest, `supergraph.yaml` — and repeating the
/// same advice each time trains people to ignore it.
#[derive(Debug, Default)]
pub struct DeprecationWarnings {
    reported: Mutex<HashSet<String>>,
}

impl DeprecationWarnings {
    /// The set for this invocation. Production call sites share this one so
    /// that "once per invocation" means what it says; tests construct their
    /// own with [`Default`] so they do not see each other's warnings.
    pub fn process() -> &'static Self {
        static PROCESS: OnceLock<DeprecationWarnings> = OnceLock::new();
        PROCESS.get_or_init(Self::default)
    }

    /// Warn that `value` is written in a deprecated spelling, unless that
    /// spelling has already been reported. Prints nothing for a version
    /// spelled the modern way, or for one that does not parse at all — that
    /// is FR5's error to raise, and raising it is the caller's job.
    pub fn warn_once<P: Print + ?Sized>(&self, printer: &P, value: &str) {
        let Some(spelling) = DeprecatedSpelling::of(value) else {
            return;
        };

        // Keyed on the spelling as written rather than on its shape: `v1.2.3`
        // and `v2.0.0` are the same deprecated form, but the advice for each
        // names a different replacement, so each is worth saying once.
        let first_time = self
            .reported
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(spelling.legacy.clone());

        if first_time {
            printer.warnln(spelling);
        }
    }
}

#[cfg(test)]
mod tests {
    use rover_print::print::testing::TerminalCapture;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn the_same_spelling_is_reported_once_however_often_it_is_seen() {
        let printer = TerminalCapture::new(false);
        let warnings = DeprecationWarnings::default();

        warnings.warn_once(&printer, "latest-2");
        warnings.warn_once(&printer, "latest-2");
        warnings.warn_once(&printer, "latest-2");

        assert_that!(printer.lines()).is_equal_to(vec![
            "warning: `latest-2` is a deprecated version format. Use `2` instead.".to_string(),
        ]);
    }

    #[test]
    fn each_distinct_spelling_gets_its_own_warning() {
        let printer = TerminalCapture::new(false);
        let warnings = DeprecationWarnings::default();

        warnings.warn_once(&printer, "latest-2");
        warnings.warn_once(&printer, "v1.2.3");
        warnings.warn_once(&printer, "latest-2");
        warnings.warn_once(&printer, "v2.0.0");

        assert_that!(printer.lines()).is_equal_to(vec![
            "warning: `latest-2` is a deprecated version format. Use `2` instead.".to_string(),
            "warning: `v1.2.3` is a deprecated version format. Use `=1.2.3` instead.".to_string(),
            "warning: `v2.0.0` is a deprecated version format. Use `=2.0.0` instead.".to_string(),
        ]);
    }

    #[rstest]
    #[case("latest")]
    #[case("2")]
    #[case("=2.9.0")]
    #[case("nonsense")]
    #[case("")]
    fn a_modern_or_unparseable_version_warns_about_nothing(#[case] value: &str) {
        let printer = TerminalCapture::new(false);
        let warnings = DeprecationWarnings::default();

        warnings.warn_once(&printer, value);

        assert_that!(printer.lines()).is_empty();
    }
}
