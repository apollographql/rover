use std::cmp::Ordering;
use std::fmt::{self, Display, Write as _};

use ansi_term::Colour::{Cyan, Yellow};
use rover_client::shared::GraphRef;

use crate::utils::env::RoverEnvKey;

use serde::Serialize;

/// `Suggestion` contains possible suggestions for remedying specific errors.
#[derive(Serialize, Debug)]
pub enum Suggestion {
    SubmitIssue,
    SetConfigHome,
    MigrateConfigHomeOrCreateConfig,
    CreateConfig,
    ListProfiles,
    UseFederatedGraph,
    RunComposition,
    CheckGraphNameAndAuth,
    ProvideValidSubgraph(Vec<String>),
    ProvideValidVariant {
        graph_ref: GraphRef,
        valid_variants: Vec<String>,
        frontend_url_root: String,
    },
    Adhoc(String),
    CheckKey,
    ValidComposeFile,
    ValidComposeRoutingUrl,
    ProperKey,
    NewUserNoProfiles,
    CheckServerConnection,
    ConvertGraphToSubgraph,
    CheckGnuVersion,
    FixSubgraphSchema {
        graph_ref: GraphRef,
        subgraph: String,
    },
    FixCompositionErrors,
    FixOperationsInSchema {
        graph_ref: GraphRef,
    },
    IncreaseClientTimeout,
}

impl Display for Suggestion {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suggestion = match self {
            Suggestion::SubmitIssue => {
                format!("This error was unexpected! Please submit an issue with any relevant details about what you were trying to do: {}", Cyan.normal().paint("https://github.com/apollographql/rover/issues/new/choose"))
            }
            Suggestion::SetConfigHome => {
                format!(
                    "You can override this path by setting the {} environment variable.",
                    Yellow.normal().paint(&format!("${}", RoverEnvKey::ConfigHome))
                )
            }
            Suggestion::MigrateConfigHomeOrCreateConfig => {
                format!("If you've recently changed the {} environment variable, you may need to migrate your old configuration directory to the new path. Otherwise, try setting up a new configuration profile by running {}.",
                Yellow.normal().paint(&format!("${}", RoverEnvKey::ConfigHome)),
                Yellow.normal().paint("`rover config auth`"))
            }
            Suggestion::CreateConfig => {
                format!(
                    "Try setting up a configuration profile by running {}",
                    Yellow.normal().paint("`rover config auth`")
                )
            }
            Suggestion::ListProfiles => {
                format!(
                    "Try running {} to see the possible values for the {} argument.",
                    Yellow.normal().paint("`rover config list`"),
                    Yellow.normal().paint("`--profile`")
                )
            }
            Suggestion::RunComposition => {
                format!("Try resolving the build errors in your subgraph(s), and publish them with the {} command.", Yellow.normal().paint("`rover subgraph publish`"))
            }
            Suggestion::UseFederatedGraph => {
                "Try running the command on a valid federated graph, or use the appropriate `rover graph` command instead of `rover subgraph`.".to_string()
            }
            Suggestion::CheckGraphNameAndAuth => {
                format!(
                    "Make sure your graph name is typed correctly, and that your API key is valid.\n        You can run {} to check if you are authenticated.\n        If you are trying to create a new graph, you must do so online at {}, by clicking \"New Graph\".",
                    Yellow.normal().paint("`rover config whoami`"),
                    Cyan.normal().paint("https://studio.apollographql.com")
                )
            }
            Suggestion::ProvideValidSubgraph(valid_subgraphs) => {
                format!(
                    "Try running this command with one of the following valid subgraphs: [{}]",
                    valid_subgraphs.join(", ")
                )
            }
            Suggestion::ProvideValidVariant { graph_ref, valid_variants, frontend_url_root} => {
                if let Some(maybe_variant) = did_you_mean(&graph_ref.variant, valid_variants).pop()  {
                    format!("Did you mean \"{}@{}\"?", graph_ref.name, maybe_variant)
                } else {
                    let num_valid_variants = valid_variants.len();
                    let color_graph_name = Cyan.normal().paint(&graph_ref.name);
                    match num_valid_variants {
                        0 => format!("Graph {} exists, but has no variants. You can create a new monolithic variant by running {} for your graph schema, or a new federated variant by running {} for all of your subgraph schemas.", &color_graph_name, Yellow.normal().paint("`rover graph publish`"), Yellow.normal().paint("`rover subgraph publish`")),
                        1 => format!("The only existing variant for graph {} is {}.", &color_graph_name, Cyan.normal().paint(&valid_variants[0])),
                        2 => format!("The existing variants for graph {} are {} and {}.", &color_graph_name, Cyan.normal().paint(&valid_variants[0]), Cyan.normal().paint(&valid_variants[1])),
                        3 ..= 10 => {
                            let mut valid_variants_msg = "".to_string();
                            for (i, variant) in valid_variants.iter().enumerate() {
                                if i == num_valid_variants - 1 {
                                    valid_variants_msg.push_str("and ");
                                }
                                let _ = write!(valid_variants_msg, "{}", Cyan.normal().paint(variant));
                                if i < num_valid_variants - 1 {
                                    valid_variants_msg.push_str(", ");
                                }
                            }
                            format!("The existing variants for graph {} are {}.", &color_graph_name, &valid_variants_msg)
                        }
                        _ => {
                            let graph_url = format!("{}/graph/{}/settings", &frontend_url_root, &color_graph_name);
                            format!("You can view the variants for graph \"{}\" by visiting {}", &color_graph_name, Cyan.normal().paint(&graph_url))
                        }
                    }
                }
            }
            Suggestion::CheckKey => {
                "Check your API key to make sure it's valid (are you using the right profile?).".to_string()
            }
            Suggestion::ProperKey => {
                format!("Try running {} for more details on Apollo's API keys.", Yellow.normal().paint("`rover docs open api-keys`"))
            }
            Suggestion::ValidComposeFile => {
                "Make sure supergraph compose config YAML points to a valid schema file.".to_string()
            }
            Suggestion::ValidComposeRoutingUrl=> {
                "When trying to compose with a local .graphql file, make sure you supply a `routing_url` in your config YAML.".to_string()
            }
            Suggestion::NewUserNoProfiles => {
                format!("It looks like you may be new here. Welcome! To authenticate with Apollo Studio, run {}, or set {} to a valid Apollo Studio API key.",
                    Yellow.normal().paint("`rover config auth`"), Cyan.normal().paint(format!("`${}`", RoverEnvKey::Key))
                )
            }
            Suggestion::Adhoc(msg) => msg.to_string(),
            Suggestion::CheckServerConnection => "Make sure the endpoint is accepting connections and is spelled correctly".to_string(),
            Suggestion::ConvertGraphToSubgraph => "If you are sure you want to convert a non-federated graph to a subgraph, you can re-run the same command with a `--convert` flag.".to_string(),
            Suggestion::CheckGnuVersion => "This is likely an issue with your current version of `glibc`. Try running `ldd --version`, and if the version >= 2.17, we suggest installing the Rover binary built for `x86_64-unknown-linux-gnu`".to_string(),
            Suggestion::FixSubgraphSchema { graph_ref, subgraph } => format!("The changes in the schema you proposed for subgraph {} are incompatible with supergraph {}. See {} for more information on resolving build errors.", Yellow.normal().paint(subgraph.to_string()), Yellow.normal().paint(graph_ref.to_string()), Cyan.normal().paint("https://www.apollographql.com/docs/federation/errors/")),
            Suggestion::FixCompositionErrors => format!("The subgraph schemas you provided are incompatible with each other. See {} for more information on resolving build errors.", Cyan.normal().paint("https://www.apollographql.com/docs/federation/errors/")),
            Suggestion::FixOperationsInSchema { graph_ref } => format!("The changes in the schema you proposed are incompatible with graph {}. See {} for more information on resolving operation check errors.", Yellow.normal().paint(graph_ref.to_string()), Cyan.normal().paint("https://www.apollographql.com/docs/studio/schema-checks/")),
            Suggestion::IncreaseClientTimeout => "You can try increasing the timeout value by passing a higher value to the --client-timeout option.".to_string(),
        };
        write!(formatter, "{}", &suggestion)
    }
}

// source: https://github.com/clap-rs/clap/blob/a0269a41d4abaf4b0a9ec4f9a059fe62ea0ba3a7/src/parse/features/suggestions.rs
/// returns a value that the user may have intended to type
fn did_you_mean<T, I>(value: &str, possible_values: I) -> Vec<String>
where
    T: AsRef<str>,
    I: IntoIterator<Item = T>,
{
    let mut candidates: Vec<(f64, String)> = possible_values
        .into_iter()
        .map(|possible_value| {
            (
                strsim::jaro_winkler(value, possible_value.as_ref()),
                possible_value.as_ref().to_owned(),
            )
        })
        .filter(|(confidence, _)| *confidence > 0.8)
        .collect();
    candidates.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));
    candidates.into_iter().map(|(_, pv)| pv).collect()
}

mod test {
    #[test]
    fn possible_values_match() {
        let p_vals = ["test", "possible", "values"];
        assert_eq!(
            super::did_you_mean("tst", p_vals.iter()).pop(),
            Some("test".to_string())
        );
    }

    #[test]
    fn possible_values_nomatch() {
        let p_vals = ["test", "possible", "values"];
        assert!(super::did_you_mean("hahaahahah", p_vals.iter())
            .pop()
            .is_none());
    }
}
