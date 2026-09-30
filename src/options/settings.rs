// This catalogue is a foundation layer: nothing in the CLI wires it up yet
// (that lands with the settings-resolution and `rover config show`/`set`/
// `unset` slices). Until then its `pub(crate)` items have no in-crate
// caller, which `dead_code` can't tell apart from genuinely unused code -
// remove this once a consumer lands.
#![allow(dead_code)]

use std::{fmt, str::FromStr};

use url::Url;

#[cfg(feature = "oauth")]
use super::oauth::{
    DEFAULT_AUTHORIZATION_URL, DEFAULT_CLIENT_ID, DEFAULT_DEVICE_AUTHORIZATION_URL,
    DEFAULT_REVOCATION_URL, DEFAULT_TOKEN_URL, DEFAULT_WHOAMI_URL,
};

// Duplicated from `utils::client::STUDIO_PROD_API_ENDPOINT` and
// `utils::telemetry::TELEMETRY_URL`, which are private to their own modules.
// The settings-resolution slice that consumes this catalogue is expected to
// make those `pub(crate)` and reference them here instead, so the default
// lives in exactly one place.
const DEFAULT_REGISTRY_URL: &str = "https://api.apollographql.com/graphql";
const DEFAULT_TELEMETRY_URL: &str = "https://rover.apollo.dev/telemetry";

/// This slice's subset of the full settings catalogue (spec.md §3.1/FR1):
/// the registry/telemetry/OAuth-endpoint group that ROVER-451 Part A's first
/// slice covers. Every setting here is profile-eligible. The OAuth endpoints
/// only exist when the `oauth` feature is compiled in - the whole
/// `rover auth login` machinery they configure is gated the same way, so a
/// stored value for one would otherwise have nothing to affect.
///
/// A setting from the full catalogue that isn't a variant here (VCS context,
/// graph ref, checks timeout, download host, templates API) isn't
/// unsupported forever - it's simply out of scope for this slice, per the
/// PRD's delivery plan. `SettingName::from_str` can't yet distinguish "known
/// to Rover but not profile-eligible" (FR42's first required-text example)
/// from "not a Rover setting at all" for one of those - that distinction
/// needs the full catalogue and is deferred to the slice that adds it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingName {
    RegistryUrl,
    TelemetryUrl,
    TelemetryDisabled,
    #[cfg(feature = "oauth")]
    OauthAuthorizationUrl,
    #[cfg(feature = "oauth")]
    OauthTokenUrl,
    #[cfg(feature = "oauth")]
    OauthDeviceAuthorizationUrl,
    #[cfg(feature = "oauth")]
    OauthRevocationUrl,
    #[cfg(feature = "oauth")]
    OauthWhoamiUrl,
    #[cfg(feature = "oauth")]
    OauthClientId,
}

/// A setting's syntactic type, for write-time and read-time validation.
/// Deliberately doesn't distinguish "checked" from "unchecked" - every
/// setting is only ever validated syntactically (FR44); no setting's value
/// is ever checked against a live server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingType {
    /// A URL with a scheme, e.g. `https://registry.example.com`.
    Url,
    /// A typed boolean: `true` or `false`, case-insensitively. This is
    /// deliberately narrower than a setting's environment-variable parsing
    /// convention might be (`APOLLO_TELEMETRY_DISABLED` is presence-only as
    /// an env var, FR21) - a *stored* boolean is always a real boolean
    /// (FR24).
    Bool,
    /// An opaque string with no further syntactic constraint.
    String,
}

impl SettingName {
    /// Every setting this slice's catalogue covers, in the order FR1's table
    /// lists them.
    pub(crate) fn all() -> Vec<SettingName> {
        #[allow(unused_mut)]
        let mut all = vec![
            SettingName::RegistryUrl,
            SettingName::TelemetryUrl,
            SettingName::TelemetryDisabled,
        ];
        #[cfg(feature = "oauth")]
        all.extend([
            SettingName::OauthAuthorizationUrl,
            SettingName::OauthTokenUrl,
            SettingName::OauthDeviceAuthorizationUrl,
            SettingName::OauthRevocationUrl,
            SettingName::OauthWhoamiUrl,
            SettingName::OauthClientId,
        ]);
        all
    }

    /// The setting's canonical name - its environment variable's spelling,
    /// verbatim (spec.md FR2). This is the only spelling `rover config`
    /// accepts and the only one Rover ever prints.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            SettingName::RegistryUrl => "APOLLO_REGISTRY_URL",
            SettingName::TelemetryUrl => "APOLLO_TELEMETRY_URL",
            SettingName::TelemetryDisabled => "APOLLO_TELEMETRY_DISABLED",
            #[cfg(feature = "oauth")]
            SettingName::OauthAuthorizationUrl => "APOLLO_OAUTH_AUTHORIZATION_URL",
            #[cfg(feature = "oauth")]
            SettingName::OauthTokenUrl => "APOLLO_OAUTH_TOKEN_URL",
            #[cfg(feature = "oauth")]
            SettingName::OauthDeviceAuthorizationUrl => "APOLLO_OAUTH_DEVICE_AUTHORIZATION_URL",
            #[cfg(feature = "oauth")]
            SettingName::OauthRevocationUrl => "APOLLO_OAUTH_REVOCATION_URL",
            #[cfg(feature = "oauth")]
            SettingName::OauthWhoamiUrl => "APOLLO_OAUTH_WHOAMI_URL",
            #[cfg(feature = "oauth")]
            SettingName::OauthClientId => "APOLLO_OAUTH_CLIENT_ID",
        }
    }

    /// The setting's syntactic type.
    pub(crate) const fn setting_type(self) -> SettingType {
        match self {
            SettingName::RegistryUrl | SettingName::TelemetryUrl => SettingType::Url,
            SettingName::TelemetryDisabled => SettingType::Bool,
            #[cfg(feature = "oauth")]
            SettingName::OauthAuthorizationUrl
            | SettingName::OauthTokenUrl
            | SettingName::OauthDeviceAuthorizationUrl
            | SettingName::OauthRevocationUrl
            | SettingName::OauthWhoamiUrl => SettingType::Url,
            #[cfg(feature = "oauth")]
            SettingName::OauthClientId => SettingType::String,
        }
    }

    /// Whether this setting's value is a host or URL Rover sends requests to
    /// or downloads code from (spec.md §3.1, the "Net" column of FR1). This
    /// is what gates the case-(b) override notice (FR59) once notices ship.
    pub(crate) const fn is_network_destination(self) -> bool {
        match self {
            SettingName::RegistryUrl | SettingName::TelemetryUrl => true,
            SettingName::TelemetryDisabled => false,
            #[cfg(feature = "oauth")]
            SettingName::OauthAuthorizationUrl
            | SettingName::OauthTokenUrl
            | SettingName::OauthDeviceAuthorizationUrl
            | SettingName::OauthRevocationUrl
            | SettingName::OauthWhoamiUrl => true,
            #[cfg(feature = "oauth")]
            SettingName::OauthClientId => false,
        }
    }

    /// The setting's built-in default, spelled the way its environment
    /// variable would spell it (FR54) - this is what `rover config show`
    /// reports as `value` when nothing overrides it.
    pub(crate) fn builtin_default(self) -> String {
        match self {
            SettingName::RegistryUrl => DEFAULT_REGISTRY_URL.to_string(),
            SettingName::TelemetryUrl => DEFAULT_TELEMETRY_URL.to_string(),
            SettingName::TelemetryDisabled => "false".to_string(),
            #[cfg(feature = "oauth")]
            SettingName::OauthAuthorizationUrl => DEFAULT_AUTHORIZATION_URL.to_string(),
            #[cfg(feature = "oauth")]
            SettingName::OauthTokenUrl => DEFAULT_TOKEN_URL.to_string(),
            #[cfg(feature = "oauth")]
            SettingName::OauthDeviceAuthorizationUrl => {
                DEFAULT_DEVICE_AUTHORIZATION_URL.to_string()
            }
            #[cfg(feature = "oauth")]
            SettingName::OauthRevocationUrl => DEFAULT_REVOCATION_URL.to_string(),
            #[cfg(feature = "oauth")]
            SettingName::OauthWhoamiUrl => DEFAULT_WHOAMI_URL.to_string(),
            #[cfg(feature = "oauth")]
            SettingName::OauthClientId => DEFAULT_CLIENT_ID.to_string(),
        }
    }
}

impl fmt::Display for SettingName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Why a string couldn't be parsed as a setting's canonical name (FR42).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SettingNameError {
    /// The input is a known setting's canonical name, spelled entirely in
    /// lowercase (FR69's project-file-only alias, typed somewhere that isn't
    /// the project file).
    #[error(
        "`{input}` isn't a Rover setting name. Settings are named as their environment \
        variables are, so use `{canonical}`. The lowercase spelling is accepted in \
        `.rover/rover.yaml` only."
    )]
    LowercaseSpelling {
        input: String,
        canonical: SettingName,
    },
    /// The input isn't any known setting's canonical name, in any casing.
    #[error(
        "`{input}` isn't a Rover setting. Run `rover config show` to list the settings \
        Rover recognizes."
    )]
    Unrecognized { input: String },
}

impl FromStr for SettingName {
    type Err = SettingNameError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let all = SettingName::all();
        if let Some(name) = all.iter().find(|name| name.as_str() == input) {
            return Ok(*name);
        }
        if let Some(name) = all
            .iter()
            .find(|name| name.as_str().to_lowercase() == input)
        {
            return Err(SettingNameError::LowercaseSpelling {
                input: input.to_string(),
                canonical: *name,
            });
        }
        Err(SettingNameError::Unrecognized {
            input: input.to_string(),
        })
    }
}

/// Why a raw value failed a setting's syntactic type check (FR43/FR44).
/// Carries no source (a profile, the project file, `config set`'s argument)
/// and no remediation text - callers that need FR84's fuller, source-aware
/// message wrap this rather than reimplementing the type check.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum SettingValueError {
    #[error(
        "`{input}` isn't a valid URL. URLs must include a scheme, for example \
        `https://example.com`."
    )]
    InvalidUrl { input: String },
    #[error(
        "`{input}` isn't a valid URL. Rover only accepts `http`/`https` URLs for this \
        setting."
    )]
    UnsupportedUrlScheme { input: String },
    #[error("`{input}` isn't a valid boolean. Use `true` or `false`.")]
    InvalidBool { input: String },
}

impl SettingType {
    /// Checks that `value` is syntactically valid for this type, handing it
    /// back on success so callers can use the validated value directly
    /// instead of holding onto their own copy. Never contacts a network - a
    /// URL that parses is accepted, whether or not anything answers at it
    /// (FR44). Deliberately hands back `value` itself rather than a parsed
    /// `Url`/`bool` - callers and the on-disk/wire format always want the
    /// setting's own spelling (FR54), and round-tripping through `Url`
    /// wouldn't reproduce it (e.g. `Url` normalizes `https://example.com` to
    /// `https://example.com/`).
    ///
    /// Every `Url`-typed setting is a network-destination setting Rover
    /// sends real HTTP requests to, so a syntactically valid but non-http(s)
    /// scheme (`ftp://`, `file://`, `data:`, ...) is rejected here too - it
    /// would otherwise pass this check only to fail later, deep inside the
    /// HTTP client, with a much less actionable error.
    pub(crate) fn validate(self, value: String) -> Result<String, SettingValueError> {
        match self {
            SettingType::Url => match Url::parse(&value) {
                Ok(url) if url.scheme() == "http" || url.scheme() == "https" => Ok(value),
                Ok(_) => Err(SettingValueError::UnsupportedUrlScheme { input: value }),
                Err(_) => Err(SettingValueError::InvalidUrl { input: value }),
            },
            SettingType::Bool => {
                if value.eq_ignore_ascii_case("true") || value.eq_ignore_ascii_case("false") {
                    Ok(value)
                } else {
                    Err(SettingValueError::InvalidBool { input: value })
                }
            }
            SettingType::String => Ok(value),
        }
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn every_canonical_name_round_trips_through_from_str() {
        for name in SettingName::all() {
            let description = format!("{name:?}");
            assert_that!(name.as_str().parse::<SettingName>())
                .named(&description)
                .is_ok_containing(name);
        }
    }

    #[test]
    fn a_lowercase_canonical_name_is_rejected_with_the_canonical_spelling() {
        let error = "apollo_registry_url".parse::<SettingName>().unwrap_err();

        assert_that!(error).is_equal_to(SettingNameError::LowercaseSpelling {
            input: "apollo_registry_url".to_string(),
            canonical: SettingName::RegistryUrl,
        });
        assert_that!(error.to_string()).is_equal_to(
            "`apollo_registry_url` isn't a Rover setting name. Settings are named as their \
            environment variables are, so use `APOLLO_REGISTRY_URL`. The lowercase spelling is \
            accepted in `.rover/rover.yaml` only."
                .to_string(),
        );
    }

    #[test]
    fn mixed_case_is_unrecognized_not_a_lowercase_alias() {
        let error = "Apollo_Registry_Url".parse::<SettingName>().unwrap_err();

        assert_that!(error).is_equal_to(SettingNameError::Unrecognized {
            input: "Apollo_Registry_Url".to_string(),
        });
    }

    #[test]
    fn an_unknown_name_is_unrecognized() {
        let error = "APOLLO_NOT_A_SETTING".parse::<SettingName>().unwrap_err();

        assert_that!(error).is_equal_to(SettingNameError::Unrecognized {
            input: "APOLLO_NOT_A_SETTING".to_string(),
        });
        assert_that!(error.to_string()).is_equal_to(
            "`APOLLO_NOT_A_SETTING` isn't a Rover setting. Run `rover config show` to list the \
            settings Rover recognizes."
                .to_string(),
        );
    }

    #[rstest]
    #[case::http_scheme("http://registry.example.com", true)]
    #[case::https_scheme("https://registry.example.com", true)]
    #[case::no_scheme("registry.example.com", false)]
    #[case::empty("", false)]
    #[case::ftp_scheme("ftp://example.com", false)]
    #[case::file_scheme("file:///etc/passwd", false)]
    #[case::data_scheme("data:text/plain,hi", false)]
    fn url_validation(#[case] value: &str, #[case] valid: bool) {
        assert_that!(SettingType::Url.validate(value.to_string()).is_ok()).is_equal_to(valid);
    }

    #[test]
    fn a_valid_url_is_handed_back_unchanged() {
        // deliberately not normalized through `Url` - see `validate`'s doc
        // comment for why (a trailing slash would be added otherwise).
        assert_that!(SettingType::Url.validate("https://registry.example.com".to_string()))
            .is_ok_containing("https://registry.example.com".to_string());
    }

    #[rstest]
    #[case::lower_true("true")]
    #[case::upper_true("TRUE")]
    #[case::lower_false("false")]
    #[case::upper_false("FALSE")]
    fn valid_bool_values_are_accepted(#[case] value: &str) {
        assert_that!(SettingType::Bool.validate(value.to_string())).is_ok();
    }

    #[rstest]
    #[case::one("1")]
    #[case::zero("0")]
    #[case::empty("")]
    #[case::garbage("sure")]
    fn other_values_are_not_a_valid_stored_boolean(#[case] value: &str) {
        // deliberately narrower than the env-var convention (FR20/FR24):
        // `1` is a set env var but not a typed boolean literal.
        assert_that!(SettingType::Bool.validate(value.to_string())).is_err();
    }

    #[test]
    fn invalid_url_message_names_the_input_and_a_correction() {
        let error = SettingType::Url
            .validate("registry.example.com".to_string())
            .unwrap_err();

        assert_that!(error.to_string()).is_equal_to(
            "`registry.example.com` isn't a valid URL. URLs must include a scheme, for example \
            `https://example.com`."
                .to_string(),
        );
    }

    // A URL with a non-http(s) scheme parses fine syntactically, so it needs
    // its own rejection path (and message) distinct from "no scheme at all" -
    // otherwise it would pass this check only to fail later, deep inside the
    // HTTP client, with a worse error.
    #[test]
    fn a_non_http_scheme_is_rejected_with_its_own_message() {
        let error = SettingType::Url
            .validate("ftp://example.com".to_string())
            .unwrap_err();

        assert_that!(error).is_equal_to(SettingValueError::UnsupportedUrlScheme {
            input: "ftp://example.com".to_string(),
        });
        assert_that!(error.to_string()).is_equal_to(
            "`ftp://example.com` isn't a valid URL. Rover only accepts `http`/`https` URLs for \
            this setting."
                .to_string(),
        );
    }

    #[test]
    fn invalid_bool_message_names_the_input_and_the_accepted_values() {
        let error = SettingType::Bool.validate("sure".to_string()).unwrap_err();

        assert_that!(error.to_string())
            .is_equal_to("`sure` isn't a valid boolean. Use `true` or `false`.".to_string());
    }

    #[test]
    fn string_type_accepts_anything_unchanged() {
        assert_that!(SettingType::String.validate(String::new())).is_ok_containing(String::new());
        assert_that!(SettingType::String.validate("anything at all".to_string()))
            .is_ok_containing("anything at all".to_string());
    }

    #[test]
    fn network_destination_settings_match_the_fr1_net_column() {
        assert_that!(SettingName::RegistryUrl.is_network_destination()).is_true();
        assert_that!(SettingName::TelemetryUrl.is_network_destination()).is_true();
        assert_that!(SettingName::TelemetryDisabled.is_network_destination()).is_false();
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_client_id_is_a_string_and_not_a_network_destination() {
        assert_that!(SettingName::OauthClientId.setting_type()).is_equal_to(SettingType::String);
        assert_that!(SettingName::OauthClientId.is_network_destination()).is_false();
    }

    #[cfg(feature = "oauth")]
    #[test]
    fn oauth_endpoints_are_network_destination_urls() {
        for name in [
            SettingName::OauthAuthorizationUrl,
            SettingName::OauthTokenUrl,
            SettingName::OauthDeviceAuthorizationUrl,
            SettingName::OauthRevocationUrl,
            SettingName::OauthWhoamiUrl,
        ] {
            let description = format!("{name:?}");
            assert_that!(name.setting_type())
                .named(&description)
                .is_equal_to(SettingType::Url);
            assert_that!(name.is_network_destination())
                .named(&description)
                .is_true();
        }
    }

    #[test]
    fn every_builtin_default_is_valid_for_its_own_type() {
        for name in SettingName::all() {
            let description = format!("{name:?}");
            assert_that!(name.setting_type().validate(name.builtin_default()))
                .named(&description)
                .is_ok();
        }
    }
}
