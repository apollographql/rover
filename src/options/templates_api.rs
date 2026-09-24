use clap::Parser;
use serde::Serialize;

/// Overrides where `rover init` and `rover template` fetch project templates
/// from.
#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Parser)]
pub struct TemplatesApiOpt {
    /// Override where `rover init`/`rover template` fetch project templates from.
    ///
    /// You can also use the `APOLLO_TEMPLATES_API` environment variable.
    #[arg(long = "templates-api", global = true, env = "APOLLO_TEMPLATES_API")]
    pub templates_api: Option<String>,
}

impl TemplatesApiOpt {
    /// `templates::request()` (`src/command/template/templates.rs`) reads
    /// `APOLLO_TEMPLATES_API` directly and has no access to this struct.
    /// Exporting the resolved value here - only when the flag or its own env
    /// var actually supplied one - lets that existing read pick it up,
    /// mirroring `OutputOpts::set_no_color`'s use of the same technique for
    /// `NO_COLOR`.
    pub fn apply_override(&self) {
        if let Some(templates_api) = &self.templates_api {
            unsafe {
                // SAFETY: called once, synchronously, before any templates
                // request is made.
                std::env::set_var("APOLLO_TEMPLATES_API", templates_api);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::TemplatesApiOpt;

    #[test]
    fn flag_is_exported_into_the_env_var_get_template_reads() {
        temp_env::with_var_unset("APOLLO_TEMPLATES_API", || {
            let opts = TemplatesApiOpt::try_parse_from([
                "templates_api",
                "--templates-api",
                "https://mirror.example.com/templates",
            ])
            .unwrap();
            opts.apply_override();
            assert_eq!(
                std::env::var("APOLLO_TEMPLATES_API").unwrap(),
                "https://mirror.example.com/templates"
            );
        });
    }

    #[test]
    fn unset_is_left_untouched() {
        temp_env::with_var_unset("APOLLO_TEMPLATES_API", || {
            let opts = TemplatesApiOpt::try_parse_from(["templates_api"]).unwrap();
            opts.apply_override();
            assert!(std::env::var("APOLLO_TEMPLATES_API").is_err());
        });
    }
}
