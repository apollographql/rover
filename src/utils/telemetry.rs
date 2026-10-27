use std::collections::HashMap;

use camino::Utf8PathBuf;
use clap::{CommandFactory, parser::ValueSource};
use reqwest::Client;
use rover_studio::types::GraphRef;
use sputnik::{Command, GraphId, Report, SputnikError};
use url::Url;

use crate::{PKG_NAME, PKG_VERSION, cli::Rover, options::SettingName, utils::env::RoverEnvKey};

const TELEMETRY_URL: &str = "https://rover.apollo.dev/telemetry";

/// The command's name, including its subcommands, e.g. `config list`: the
/// subcommands clap matched, each reported under the name it has always had.
fn command_name(matches: &clap::ArgMatches) -> String {
    let mut names = Vec::new();
    let mut current = matches;
    while let Some((name, sub_matches)) = current.subcommand() {
        names.push(reported_name(name));
        current = sub_matches;
    }
    names.join(" ")
}

/// How a subcommand has always been reported. That was its Rust variant's name,
/// lowercased, which is clap's name with the hyphens dropped (`persisted-queries`
/// is `persistedqueries`) for every subcommand but one: the variant `ApiKeys` is
/// `api-key` to clap.
fn reported_name(clap_name: &str) -> String {
    match clap_name {
        "api-key" => "apikeys".to_string(),
        other => other.replace('-', "").to_lowercase(),
    }
}

/// Flags whose values telemetry reports, by clap argument id. A flag may be
/// listed only if every value it can take is non-identifying: a boolean, or
/// one with a fixed set of values. Every other flag, including any added in
/// the future, is reported by name only.
const ALLOWLISTED_FLAGS: &[&str] = &["format_kind"];

/// The flags `matches` supplied, keyed by canonical long name: the value for
/// an allowlisted flag, otherwise `null`. Positional arguments are left out.
fn supplied_flags(
    command: &clap::Command,
    matches: &clap::ArgMatches,
) -> HashMap<String, serde_json::Value> {
    let mut flags = HashMap::new();
    collect_supplied_flags(command, matches, &mut flags);
    flags
}

fn collect_supplied_flags(
    command: &clap::Command,
    matches: &clap::ArgMatches,
    flags: &mut HashMap<String, serde_json::Value>,
) {
    for arg in command.get_arguments().filter(|arg| !arg.is_positional()) {
        let id = arg.get_id().as_str();
        if !matches.try_contains_id(id).unwrap_or(false)
            || !matches!(
                matches.value_source(id),
                Some(ValueSource::CommandLine | ValueSource::EnvVariable)
            )
        {
            continue;
        }
        let value = if ALLOWLISTED_FLAGS.contains(&id) {
            allowlisted_value(arg, matches)
        } else {
            serde_json::Value::Null
        };
        flags.insert(flag_name(arg), value);
    }
    if let Some((name, sub_matches)) = matches.subcommand()
        && let Some(subcommand) = command.find_subcommand(name)
    {
        collect_supplied_flags(subcommand, sub_matches, flags);
    }
}

fn allowlisted_value(arg: &clap::Arg, matches: &clap::ArgMatches) -> serde_json::Value {
    if !arg.get_action().takes_values() {
        return serde_json::Value::Bool(true);
    }
    matches
        .get_raw(arg.get_id().as_str())
        .map(|values| {
            values
                .map(|value| value.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join(",")
        })
        .map_or(serde_json::Value::Null, serde_json::Value::String)
}

/// A flag's canonical name: its long form. The classification test requires every flag to
/// have one, so the id is only a fallback that never applies.
fn flag_name(arg: &clap::Arg) -> String {
    arg.get_long()
        .map_or_else(|| arg.get_id().to_string(), str::to_string)
}

/// The graph ref the command line `matches` came from names, from the `graph_ref` argument of
/// the command it runs, if there is one: the `GRAPH_REF` positional of a command like
/// `rover graph fetch`, or the `--graph-ref` flag of `rover dev`.
///
/// Reading it from the matches, rather than from each command's own struct, covers every command
/// that has such an argument, including any added later.
fn graph_ref_in(matches: &clap::ArgMatches) -> Option<GraphRef> {
    let mut graph_ref = None;
    let mut level = matches;
    loop {
        if let Ok(Some(found)) = level.try_get_one::<GraphRef>("graph_ref") {
            graph_ref = Some(found.clone());
        }
        match level.subcommand() {
            Some((_, next)) => level = next,
            None => return graph_ref,
        }
    }
}

impl Rover {
    /// The graph ref this invocation ran against, if it has a resolvable one (FR20).
    ///
    /// A command takes it from its arguments. A command that falls back to the setting,
    /// `rover dev`, also reads `APOLLO_GRAPH_REF`, and then the stored profile and project
    /// settings, as it does to run. This never fails and never prints: a ref that can't be found
    /// or read is no graph ref.
    pub(crate) fn resolvable_graph_ref(&self) -> Option<GraphRef> {
        self.arg_matches().and_then(graph_ref_in).or_else(|| {
            if !self.graph_ref_falls_back_to_the_setting() {
                return None;
            }
            let raw = self
                .get_env_var(RoverEnvKey::GraphRef)
                .ok()
                .flatten()
                .or_else(|| {
                    self.resolve_stored_setting(SettingName::GraphRef)
                        .ok()
                        .flatten()
                        .map(|(_, value)| value)
                })?;
            raw.parse().ok()
        })
    }

    /// The identifier of the graph this invocation ran against, for its telemetry event, when it
    /// has one. Nothing is read or written when telemetry is disabled, so an install that
    /// doesn't report, or doesn't use a graph ref, never creates an install salt.
    pub(crate) fn graph_id(&self) -> Option<GraphId> {
        if !self.is_telemetry_enabled().ok()? {
            return None;
        }
        let graph_ref = self.resolvable_graph_ref()?;
        match self.install_salt() {
            Ok(salt) => Some(salt.graph_identifier(&graph_ref)),
            Err(error) => {
                tracing::debug!(?error, "could not read the install salt");
                None
            }
        }
    }
}

impl Report for Rover {
    fn serialize_command(&self) -> Result<Command, SputnikError> {
        // Without what clap matched there's no telling which flags were
        // supplied, so there's no event to send.
        let matches = self.arg_matches().ok_or(SputnikError::CommandParseError)?;
        let mut command = Rover::command();
        command.build();
        let serialized_command = Command {
            name: command_name(matches),
            arguments: supplied_flags(&command, matches),
        };
        tracing::debug!(?serialized_command);
        Ok(serialized_command)
    }

    fn is_telemetry_enabled(&self) -> Result<bool, SputnikError> {
        let value = self.get_env_var(RoverEnvKey::TelemetryDisabled)?;
        let is_telemetry_disabled = self.is_telemetry_disabled() || value.is_some();
        if is_telemetry_disabled {
            tracing::info!("Telemetry has been disabled.");
        } else {
            tracing::info!(
                "Telemetry is enabled. To disable, set ${}=true",
                RoverEnvKey::TelemetryDisabled.to_string()
            )
        }
        Ok(!is_telemetry_disabled)
    }

    fn endpoint(&self) -> Result<Url, SputnikError> {
        let url = self
            .telemetry_url_override()
            .unwrap_or_else(|| TELEMETRY_URL.to_string());
        Ok(Url::parse(&url)?)
    }

    fn tool_name(&self) -> String {
        PKG_NAME.to_string()
    }

    fn version(&self) -> String {
        PKG_VERSION.to_string()
    }

    fn machine_id_config(&self) -> Result<Utf8PathBuf, SputnikError> {
        let config = self
            .get_rover_config()
            .map_err(|_| SputnikError::ConfigError)?;
        Ok(config.home.join("machine.txt"))
    }

    fn client(&self) -> anyhow::Result<Client, SputnikError> {
        self.get_reqwest_client(None)
            .map_err(|error| SputnikError::AdhocError(anyhow::anyhow!(error.to_string())))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet, HashMap};

    use assert_fs::prelude::*;
    use clap::{CommandFactory, FromArgMatches, Parser};
    use rstest::rstest;
    use serde_json::json;
    use speculoos::prelude::*;
    use sputnik::Command;

    use super::{ALLOWLISTED_FLAGS, flag_name, graph_ref_in};
    use crate::{
        PKG_NAME,
        cli::Rover,
        utils::{env::RoverEnvKey, telemetry::Report},
    };

    /// Names are as they were before they came from clap: a subcommand's
    /// hyphens are dropped, an alias is reported as the command it names, and
    /// `api-key` is still the plural it was named after.
    #[rstest::rstest]
    #[case::subcommand(&["config", "list"], "config list")]
    #[case::deeper(&["config", "whoami"], "config whoami")]
    #[case::leaf(&["lsp"], "lsp")]
    #[case::positional(&["explain", "E001"], "explain")]
    #[case::hyphenated(&["persisted-queries", "publish", "g@v", "--manifest", "m"], "persistedqueries publish")]
    #[case::alias(&["pq", "publish", "g@v", "--manifest", "m"], "persistedqueries publish")]
    #[case::renamed(&["api-key", "list", "an-org"], "apikeys list")]
    #[case::renamed_deeper(&["api-key", "delete", "an-org", "a-key"], "apikeys delete")]
    #[case::graph(&["graph", "fetch", "g@v"], "graph fetch")]
    #[case::client(&["client", "check", "g@v"], "client check")]
    fn commands_keep_their_names(#[case] args: &[&str], #[case] expected: &str) {
        let args = std::iter::once(PKG_NAME)
            .chain(args.iter().copied())
            .collect::<Vec<_>>();
        let command = parse_with_matches(&args).serialize_command().unwrap();
        assert_that!(command.name).is_equal_to(expected.to_string());
    }

    #[test]
    fn nothing_is_serialized_without_what_clap_matched() {
        let rover = Rover::parse_from([PKG_NAME, "config", "list"]);
        assert_that!(rover.serialize_command().map_err(|error| error.to_string()))
            .is_equal_to(Err("Could not parse command line arguments".to_string()));
    }

    const RECORDED_FLAGS: &str = include_str!("telemetry_flags.fixture.txt");

    /// Every environment variable a flag can fall back to, so that a test isn't
    /// affected by what's set on the machine running it.
    fn flag_env_vars() -> Vec<String> {
        fn walk(command: &clap::Command, vars: &mut Vec<String>) {
            vars.extend(
                command
                    .get_arguments()
                    .filter_map(|arg| arg.get_env())
                    .map(|var| var.to_string_lossy().into_owned()),
            );
            for subcommand in command.get_subcommands() {
                walk(subcommand, vars);
            }
        }

        let mut command = Rover::command();
        command.build();
        let mut vars = Vec::new();
        walk(&command, &mut vars);
        vars.sort();
        vars.dedup();
        vars
    }

    /// A `Rover` built the way `run_from_args` builds one, keeping the matches,
    /// with only the given environment variables set among those a flag reads.
    fn parse_with_matches_env(args: &[&str], env: &[(&str, &str)]) -> Rover {
        let mut vars = flag_env_vars()
            .into_iter()
            .map(|var| (var, None))
            .collect::<BTreeMap<_, _>>();
        vars.extend(
            env.iter()
                .map(|(var, value)| (var.to_string(), Some(value.to_string()))),
        );
        let matches = temp_env::with_vars(vars.into_iter().collect::<Vec<_>>(), || {
            Rover::command().get_matches_from(args)
        });
        Rover::from_arg_matches(&matches)
            .expect("could not build Rover")
            .with_arg_matches(matches)
    }

    fn parse_with_matches(args: &[&str]) -> Rover {
        parse_with_matches_env(args, &[])
    }

    fn arguments(args: &[&str]) -> HashMap<String, serde_json::Value> {
        parse_with_matches(args)
            .serialize_command()
            .expect("could not serialize command")
            .arguments
    }

    #[test]
    fn positional_arguments_are_not_reported() {
        let rover = parse_with_matches(&[PKG_NAME, "explain", "E002"]);
        let command = rover.serialize_command().unwrap();
        assert_that!(command).is_equal_to(Command {
            name: "explain".to_string(),
            arguments: HashMap::new(),
        });
    }

    #[test]
    fn flag_values_are_not_reported() {
        let mut expected = HashMap::new();
        expected.insert("profile".to_string(), serde_json::Value::Null);
        expected.insert("output".to_string(), serde_json::Value::Null);
        expected.insert("registry-url".to_string(), serde_json::Value::Null);
        assert_that!(arguments(&[
            PKG_NAME,
            "config",
            "list",
            "--profile",
            "secret-profile",
            "-o",
            "/home/someone/out.txt",
            "--registry-url",
            "https://internal.example.com",
        ]))
        .is_equal_to(expected);
    }

    #[test]
    fn allowlisted_flag_values_are_reported() {
        let mut expected = HashMap::new();
        expected.insert("format".to_string(), json!("json"));
        assert_that!(arguments(&[PKG_NAME, "config", "list", "--format", "json"]))
            .is_equal_to(expected);
    }

    #[test]
    fn flags_that_were_not_supplied_are_not_reported() {
        assert_that!(arguments(&[PKG_NAME, "config", "list"])).is_equal_to(HashMap::new());
    }

    #[test]
    fn flags_supplied_by_the_environment_are_reported() {
        let rover = parse_with_matches_env(
            &[PKG_NAME, "config", "list"],
            &[
                ("APOLLO_FORMAT", "json"),
                ("APOLLO_VCS_BRANCH", "a-private-branch"),
            ],
        );
        let mut expected = HashMap::new();
        expected.insert("format".to_string(), json!("json"));
        expected.insert("vcs-branch".to_string(), serde_json::Value::Null);
        assert_that!(rover.serialize_command().unwrap().arguments).is_equal_to(expected);
    }

    #[test]
    fn a_flag_defined_on_a_nested_subcommand_is_reported() {
        let mut expected = HashMap::new();
        expected.insert("name".to_string(), serde_json::Value::Null);
        assert_that!(arguments(&[
            PKG_NAME,
            "subgraph",
            "fetch",
            "graph@variant",
            "--name",
            "products"
        ]))
        .is_equal_to(expected);
    }

    /// Every flag in the command tree, as `command path: --flag [class]`, where
    /// the class is whether telemetry reports its value or its name only.
    /// Positional arguments are never reported, and global flags are listed
    /// once, at the root. A flag missing from `telemetry_flags.fixture.txt` fails this
    /// test, so adding one forces a decision about how telemetry reports it.
    /// The file is a superset across platforms and features, since some
    /// commands only exist on some of them, so updating it never removes a
    /// line for a flag this build can't see; delete the lines for a removed
    /// flag by hand.
    #[test]
    fn every_flag_is_classified() {
        fn walk(
            command: &clap::Command,
            path: &str,
            is_root: bool,
            lines: &mut Vec<String>,
            classes: &mut BTreeMap<String, BTreeSet<&'static str>>,
        ) {
            for arg in command.get_arguments() {
                if arg.is_positional()
                    || (arg.is_global_set() && !is_root)
                    || matches!(arg.get_id().as_str(), "help" | "version")
                {
                    continue;
                }
                assert_that!(arg.get_long())
                    .named(&format!("{path} {} has a long name", arg.get_id()))
                    .is_some();
                let class = if ALLOWLISTED_FLAGS.contains(&arg.get_id().as_str()) {
                    assert_that!(is_eligible(arg))
                        .named(&format!("{path} --{} is eligible", flag_name(arg)))
                        .is_true();
                    "value"
                } else {
                    "name-only"
                };
                classes.entry(flag_name(arg)).or_default().insert(class);
                lines.push(format!("{path}: --{} [{class}]", flag_name(arg)));
            }
            for subcommand in command.get_subcommands() {
                let path = format!("{path} {}", subcommand.get_name());
                walk(subcommand, &path, false, lines, classes);
            }
        }

        let mut command = Rover::command();
        command.build();
        let mut lines = Vec::new();
        let mut classes = BTreeMap::new();
        walk(&command, "rover", true, &mut lines, &mut classes);
        lines.sort();

        // Flags are keyed by long name in the event, so one name must never
        // be a value at one level and name-only at another: the latter would
        // overwrite the former.
        let conflicting = classes
            .iter()
            .filter(|(_, classes)| classes.len() > 1)
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>();
        assert_that!(conflicting)
            .named("flags with different classes at different levels")
            .is_equal_to(Vec::<&str>::new());

        let recorded = RECORDED_FLAGS.lines().collect::<BTreeSet<_>>();
        if std::env::var_os("UPDATE_TELEMETRY_FLAGS").is_some() {
            // Keep what this build can't see, such as flags that only exist on
            // other platforms, but replace any line for a flag found here.
            let found = lines
                .iter()
                .map(|line| flag_key(line))
                .collect::<BTreeSet<_>>();
            let merged = lines
                .iter()
                .map(String::as_str)
                .chain(
                    recorded
                        .iter()
                        .copied()
                        .filter(|line| !found.contains(flag_key(line))),
                )
                .collect::<BTreeSet<_>>();
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/utils/telemetry_flags.fixture.txt"
            );
            std::fs::write(
                path,
                format!("{}\n", merged.into_iter().collect::<Vec<_>>().join("\n")),
            )
            .unwrap();
            return;
        }
        let unclassified = lines
            .iter()
            .filter(|line| !recorded.contains(line.as_str()))
            .collect::<Vec<_>>();
        assert_that!(unclassified)
            .named(
                "flags not recorded in src/utils/telemetry_flags.fixture.txt; decide whether telemetry \
             reports each one's value or its name only, then run this test with \
             UPDATE_TELEMETRY_FLAGS=1 to record it",
            )
            .is_empty();
    }

    /// `rover command: --flag`, without the class.
    fn flag_key(line: &str) -> &str {
        line.rsplit_once(" [").map_or(line, |(key, _)| key)
    }

    /// Only a boolean or a flag with a fixed set of values may be allowlisted.
    fn is_eligible(arg: &clap::Arg) -> bool {
        !arg.get_action().takes_values() || !arg.get_possible_values().is_empty()
    }

    /// Every flag in the tree with the given id, at whatever level it's defined.
    fn flags_with_id<'a>(command: &'a clap::Command, id: &str, found: &mut Vec<&'a clap::Arg>) {
        found.extend(
            command
                .get_arguments()
                .filter(|arg| arg.get_id().as_str() == id),
        );
        for subcommand in command.get_subcommands() {
            flags_with_id(subcommand, id, found);
        }
    }

    #[test]
    fn allowlisted_flags_exist_and_are_eligible() {
        let mut command = Rover::command();
        command.build();
        for id in ALLOWLISTED_FLAGS {
            let mut found = Vec::new();
            flags_with_id(&command, id, &mut found);
            assert_that!(found.is_empty())
                .named(&format!("`{id}` names no flag"))
                .is_false();
            let ineligible = found
                .iter()
                .filter(|arg| !is_eligible(arg))
                .map(|arg| flag_name(arg))
                .collect::<Vec<_>>();
            assert_that!(ineligible)
                .named(&format!(
                    "flags with the allowlisted id `{id}` that aren't eligible"
                ))
                .is_equal_to(Vec::<String>::new());
        }
    }

    #[test]
    fn it_respects_apollo_telemetry_url() {
        let apollo_telemetry_url = "https://example.com/telemetry";
        let args = vec![PKG_NAME, "config", "list"];
        // `--telemetry-url`/`APOLLO_TELEMETRY_URL` is now a clap-native
        // env-bound flag, resolved at parse time - so the env var has to be
        // set before `Rover::parse_from` runs, not after via the
        // `RoverEnv` test cache `insert_env_var` uses.
        let rover = temp_env::with_var(
            RoverEnvKey::TelemetryUrl.to_string(),
            Some(apollo_telemetry_url),
            || Rover::parse_from(&args),
        );
        let actual_endpoint = rover
            .endpoint()
            .expect("could not parse telemetry URL")
            .to_string();
        let expected_endpoint = apollo_telemetry_url.to_string();

        assert_eq!(actual_endpoint, expected_endpoint);
    }

    #[test]
    fn it_can_be_disabled() {
        let args = vec![PKG_NAME, "config", "list"];
        let mut rover = Rover::parse_from(args);
        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "1")
            .unwrap();
        let expect_enabled = false;
        let mut is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();

        assert_eq!(is_telemetry_enabled, expect_enabled);

        rover
            .insert_env_var(RoverEnvKey::TelemetryDisabled, "true")
            .unwrap();
        is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();

        assert_eq!(is_telemetry_enabled, expect_enabled);
    }

    #[test]
    fn it_can_be_disabled_via_flag() {
        let args = vec![PKG_NAME, "config", "list", "--telemetry-disabled"];
        let rover = Rover::parse_from(args);
        let is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();

        assert_that!(is_telemetry_enabled).is_false();
    }

    #[test]
    fn it_is_enabled_by_default() {
        let args = vec![PKG_NAME, "config", "list"];
        let rover = Rover::parse_from(args);
        let expect_enabled = true;
        let is_telemetry_enabled = rover.is_telemetry_enabled().unwrap();
        assert_eq!(is_telemetry_enabled, expect_enabled);
    }

    /// Every command with a `graph_ref` argument, as the path to it.
    fn commands_with_a_graph_ref() -> Vec<String> {
        fn walk(command: &clap::Command, path: &mut Vec<String>, found: &mut Vec<String>) {
            if command
                .get_arguments()
                .any(|arg| arg.get_id() == "graph_ref")
            {
                found.push(path.join(" "));
            }
            for subcommand in command.get_subcommands() {
                path.push(subcommand.get_name().to_string());
                walk(subcommand, path, found);
                path.pop();
            }
        }
        let mut found = Vec::new();
        walk(&Rover::command(), &mut Vec::new(), &mut found);
        found
    }

    /// Fails when a command gains a `graph_ref` argument that nothing here covers, so that
    /// its invocations can't silently go without a graph identifier. Add it here and to the
    /// cases of `a_commands_graph_ref_is_found`.
    #[test]
    fn every_command_with_a_graph_ref_is_covered() {
        let mut expected: Vec<String> = [
            "contract describe",
            "contract preview",
            "contract publish",
            "dev",
            "supergraph fetch",
            "graph check",
            "graph delete",
            "graph fetch",
            "graph lint",
            "graph publish",
            "readme fetch",
            "readme publish",
            "subgraph check",
            "subgraph delete",
            "subgraph fetch",
            "subgraph lint",
            "subgraph list",
            "subgraph preview",
            "subgraph publish",
            "persisted-queries publish",
            "client check",
        ]
        .map(String::from)
        .to_vec();
        // Without the `composition-js` feature there is no `connector` command, and
        // `supergraph compose` takes no graph ref.
        if cfg!(feature = "composition-js") {
            expected.extend(["connector", "supergraph compose"].map(String::from));
        }
        expected.sort();
        let mut found = commands_with_a_graph_ref();
        found.sort();

        assert_that!(found).is_equal_to(expected);
    }

    #[rstest]
    #[case::contract_describe(&["contract", "describe", "my-graph@staging"])]
    #[case::contract_preview(&["contract", "preview", "my-graph@staging"])]
    #[case::contract_publish(&[
        "contract", "publish", "my-graph@staging", "--no-include-tags", "--no-exclude-tags",
        "--no-hide-unreachable-types",
    ])]
    #[case::dev(&["dev", "--graph-ref", "my-graph@staging"])]
    #[case::supergraph_fetch(&["supergraph", "fetch", "my-graph@staging"])]
    #[case::graph_check(&["graph", "check", "my-graph@staging", "--schema", "-"])]
    #[case::graph_delete(&["graph", "delete", "my-graph@staging"])]
    #[case::graph_fetch(&["graph", "fetch", "my-graph@staging"])]
    #[case::graph_lint(&["graph", "lint", "my-graph@staging", "--schema", "-"])]
    #[case::graph_publish(&["graph", "publish", "my-graph@staging", "--schema", "-"])]
    #[case::readme_fetch(&["readme", "fetch", "my-graph@staging"])]
    #[case::readme_publish(&["readme", "publish", "my-graph@staging", "--file", "-"])]
    #[case::subgraph_check(&["subgraph", "check", "my-graph@staging", "--name", "x", "--schema", "-"])]
    #[case::subgraph_delete(&["subgraph", "delete", "my-graph@staging", "--name", "x"])]
    #[case::subgraph_fetch(&["subgraph", "fetch", "my-graph@staging", "--name", "x"])]
    #[case::subgraph_lint(&["subgraph", "lint", "my-graph@staging", "--name", "x", "--schema", "-"])]
    #[case::subgraph_list(&["subgraph", "list", "my-graph@staging"])]
    #[case::subgraph_preview(&["subgraph", "preview", "my-graph@staging"])]
    #[case::subgraph_publish(&["subgraph", "publish", "my-graph@staging", "--name", "x", "--schema", "-"])]
    #[case::persisted_queries_publish(&["persisted-queries", "publish", "my-graph@staging", "--manifest", "-"])]
    #[case::client_check(&["client", "check", "my-graph@staging"])]
    fn a_commands_graph_ref_is_found(#[case] args: &[&str]) {
        let args = std::iter::once(PKG_NAME).chain(args.iter().copied());
        let matches = Rover::command().get_matches_from(args);

        assert_that!(graph_ref_in(&matches).map(|graph_ref| graph_ref.to_string()))
            .is_equal_to(Some("my-graph@staging".to_string()));
    }

    /// The commands that only take a graph ref when Rover can compose.
    #[cfg(feature = "composition-js")]
    #[rstest]
    #[case::connector(&[
        "connector", "--graph-ref", "my-graph@staging", "run", "--connector-id", "x", "--variables", "{}",
    ])]
    #[case::supergraph_compose(&["supergraph", "compose", "--graph-ref", "my-graph@staging"])]
    fn a_composing_commands_graph_ref_is_found(#[case] args: &[&str]) {
        let args = std::iter::once(PKG_NAME).chain(args.iter().copied());
        let matches = Rover::command().get_matches_from(args);

        assert_that!(graph_ref_in(&matches).map(|graph_ref| graph_ref.to_string()))
            .is_equal_to(Some("my-graph@staging".to_string()));
    }

    #[test]
    fn a_graph_ref_without_a_variant_is_the_current_variant() {
        let matches = Rover::command().get_matches_from([PKG_NAME, "graph", "fetch", "my-graph"]);

        assert_that!(graph_ref_in(&matches).map(|graph_ref| graph_ref.to_string()))
            .is_equal_to(Some("my-graph@current".to_string()));
    }

    #[rstest]
    #[case::a_command_that_takes_none(&["config", "list"])]
    #[case::an_optional_one_left_off(&["client", "check"])]
    #[case::dev_without_the_flag(&["dev"])]
    fn no_graph_ref_is_found(#[case] args: &[&str]) {
        let args = std::iter::once(PKG_NAME).chain(args.iter().copied());
        let matches = Rover::command().get_matches_from(args);

        assert_that!(graph_ref_in(&matches)).is_none();
    }

    fn dev_with_env_graph_ref(args: &[&str], value: &str) -> Rover {
        let mut rover = parse_with_matches(args);
        rover.insert_env_var(RoverEnvKey::GraphRef, value).unwrap();
        rover
    }

    #[test]
    fn dev_falls_back_to_the_environment_variable() {
        let rover = dev_with_env_graph_ref(&[PKG_NAME, "dev"], "my-graph@staging");

        assert_that!(
            rover
                .resolvable_graph_ref()
                .map(|graph_ref| graph_ref.to_string())
        )
        .is_equal_to(Some("my-graph@staging".to_string()));
    }

    #[test]
    fn dev_prefers_its_flag_to_the_environment_variable() {
        let rover = dev_with_env_graph_ref(
            &[PKG_NAME, "dev", "--graph-ref", "my-graph@staging"],
            "other-graph@prod",
        );

        assert_that!(
            rover
                .resolvable_graph_ref()
                .map(|graph_ref| graph_ref.to_string())
        )
        .is_equal_to(Some("my-graph@staging".to_string()));
    }

    #[test]
    fn dev_has_no_graph_ref_when_the_environment_variable_is_malformed() {
        let rover = dev_with_env_graph_ref(&[PKG_NAME, "dev"], "not a graph ref!");

        assert_that!(rover.resolvable_graph_ref()).is_none();
    }

    #[test]
    fn only_dev_falls_back_to_the_environment_variable() {
        let mut rover = parse_with_matches(&[PKG_NAME, "config", "list"]);
        rover
            .insert_env_var(RoverEnvKey::GraphRef, "my-graph@staging")
            .unwrap();

        assert_that!(rover.resolvable_graph_ref()).is_none();
    }

    /// Runs `test` with an empty config home. `RoverEnv` starts empty under `cfg(test)`, so
    /// the process's own environment can't disable telemetry here.
    fn with_config_home<T>(test: impl FnOnce(&assert_fs::TempDir, &str) -> T) -> T {
        let home = assert_fs::TempDir::new().unwrap();
        let path = home.path().to_str().unwrap().to_string();
        test(&home, &path)
    }

    #[test]
    fn the_graph_id_is_the_graph_ref_keyed_by_the_install_salt() {
        with_config_home(|home, path| {
            let args = [
                PKG_NAME,
                "--config-home",
                path,
                "graph",
                "fetch",
                "my-graph@staging",
            ];
            let rover = parse_with_matches(&args);

            let graph_id = rover.graph_id().unwrap();

            let salt = rover.install_salt().unwrap();
            assert_that!(graph_id)
                .is_equal_to(salt.graph_identifier(&"my-graph@staging".parse().unwrap()));
            assert_that!(home.child("install_salt.txt").exists()).is_true();
            assert_that!(rover.graph_id()).is_equal_to(Some(graph_id));
        });
    }

    #[test]
    fn a_command_with_no_graph_ref_has_no_graph_id_and_creates_no_salt() {
        with_config_home(|home, path| {
            let args = [PKG_NAME, "--config-home", path, "config", "list"];
            let rover = parse_with_matches(&args);

            assert_that!(rover.graph_id()).is_none();
            assert_that!(home.child("install_salt.txt").exists()).is_false();
        });
    }

    #[test]
    fn nothing_is_derived_or_stored_when_telemetry_is_disabled() {
        with_config_home(|home, path| {
            let args = [
                PKG_NAME,
                "--config-home",
                path,
                "--telemetry-disabled",
                "graph",
                "fetch",
                "my-graph@staging",
            ];
            let rover = parse_with_matches(&args);

            assert_that!(rover.graph_id()).is_none();
            assert_that!(home.child("install_salt.txt").exists()).is_false();
        });
    }
}
