use clap::Parser;
use serde::Serialize;

/// Overrides where `rover template` fetches project templates from.
#[cfg_attr(test, derive(Default))]
#[derive(Debug, Clone, Serialize, Parser)]
pub struct TemplatesApiOpt {
    /// Override where `rover template` fetches project templates from.
    ///
    /// You can also use the `APOLLO_TEMPLATES_API` environment variable.
    #[arg(long = "templates-api", global = true, env = "APOLLO_TEMPLATES_API")]
    pub templates_api: Option<String>,
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::TemplatesApiOpt;

    #[test]
    fn flag_wins_over_env_var() {
        let opts = temp_env::with_var(
            "APOLLO_TEMPLATES_API",
            Some("https://env.example.com/templates"),
            || {
                TemplatesApiOpt::try_parse_from([
                    "templates_api",
                    "--templates-api",
                    "https://flag.example.com/templates",
                ])
            },
        )
        .unwrap();
        assert_eq!(
            opts.templates_api.as_deref(),
            Some("https://flag.example.com/templates")
        );
    }

    #[test]
    fn env_var_applies_alone() {
        let opts = temp_env::with_var(
            "APOLLO_TEMPLATES_API",
            Some("https://env.example.com/templates"),
            || TemplatesApiOpt::try_parse_from(["templates_api"]),
        )
        .unwrap();
        assert_eq!(
            opts.templates_api.as_deref(),
            Some("https://env.example.com/templates")
        );
    }

    #[test]
    fn defaults_to_none() {
        let opts = temp_env::with_var_unset("APOLLO_TEMPLATES_API", || {
            TemplatesApiOpt::try_parse_from(["templates_api"])
        })
        .unwrap();
        assert_eq!(opts.templates_api, None);
    }
}
