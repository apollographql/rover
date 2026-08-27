use std::{
    process::Command,
    str::{FromStr, from_utf8},
    thread,
    time::Duration,
};

use apollo_federation_types::config::{FederationVersion, PluginVersion, RouterVersion};
use assert_cmd::cargo;
use assert_fs::TempDir;
use binstall::Installer;
use camino::Utf8PathBuf;
use regex::Regex;
use rover_tower::retry::ExponentialBackoffPolicy;
use rstest::{fixture, rstest};
use serial_test::serial;
use speculoos::prelude::*;
use tower::{Service, ServiceBuilder, service_fn};
use tracing_test::traced_test;

/// Runs a rover install command with retries to handle transient network failures in CI.
/// Returns the output of the first successful attempt, or the output of the last attempt
/// if all retries are exhausted.
fn run_with_retries(cmd_fn: impl Fn() -> Command, max_attempts: u32) -> std::process::Output {
    let mut last_output = None;
    for attempt in 1..=max_attempts {
        let output = cmd_fn().output().expect("Could not run command");
        if output.status.success() || attempt == max_attempts {
            return output;
        }
        eprintln!(
            "attempt {attempt}/{max_attempts} failed (exit {}), retrying in 5s...",
            output.status
        );
        last_output = Some(output);
        thread::sleep(Duration::from_secs(5));
    }
    last_output.unwrap()
}

#[rstest]
#[case::installs_supergraph_at_pinned_version(Vec::from(["install", "--plugin", "supergraph@=2.8.0"]), "supergraph-v2.8.0")]
#[case::installs_supergraph_at_latest(Vec::from(["install", "--plugin", "supergraph@latest-2"]), "supergraph-")]
// installs_supergraph_at_latest_0 (Federation 1) is intentionally absent: `rover install
// --plugin` now rejects Federation 1 versions outright.
#[case::installs_router_at_pinned_version(Vec::from(["install", "--plugin", "router@=1.0.0"]), "router-v1.0.0")]
#[case::installs_router_at_latest(Vec::from(["install", "--plugin", "router@latest"]), "router-")]
#[case::installs_router_2x(Vec::from(["install", "--plugin", "router@2"]), "router-")]
#[case::installs_apollo_mcp_server_at_latest(Vec::from(["install", "--plugin", "apollo-mcp-server@latest"]), "apollo-mcp-server")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugin(#[case] args: Vec<&str>, #[case] binary_name: &str) {
    // GIVEN
    //   - an install command for the supergraph binary that forces replacement; sometimes this
    //   forces a replacement (whenever there's already a supergraph binary of the right version
    //   installed) and other times it just intsalls the plugin
    // WHEN
    //   - it's run
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");
    let args_owned: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&args_owned);
            cmd
        },
        3,
    );

    asserting(&format!(
        "Was expecting success but instead got: {}",
        from_utf8(output.stderr.as_slice()).unwrap()
    ))
    .that(&output.status.success())
    .is_true();

    // THEN
    //   - it successfully installs
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(binary_name)
        });

    assert_that!(installed).is_true();
}

// We use a stable directory across the following install tests to make sure that --force works as
// expected
#[fixture]
#[once]
fn temp_dir() -> Utf8PathBuf {
    Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap()
}

#[rstest]
#[case::force_installs_supergraph(Vec::from(["install", "--force", "--plugin", "supergraph@=2.8.0", "--log", "debug"]), "supergraph", "supergraph-v2.8.0")]
#[case::force_installs_router(Vec::from(["install", "--force", "--plugin", "router@=1.0.0", "--log", "debug"]), "router",  "router-v1.0.0")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugin_with_force_opt(
    #[case] args: Vec<&str>,
    #[case] binary: &str,
    #[case] binary_filename: &str,
    temp_dir: &Utf8PathBuf,
) {
    let bin_path = temp_dir.join(".rover/bin");

    let forced_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let args_without_force_option: Vec<String> = args
        .iter()
        .filter(|opt| *opt != &"--force")
        .map(|s| s.to_string())
        .collect();

    // FIRST INSTALLATION, NO FORCE
    let temp_dir_clone = temp_dir.clone();
    let args_clone = args_without_force_option.clone();
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir_clone);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&args_clone);
            cmd
        },
        3,
    );
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    assert_that!(stderr).contains(format!("the '{binary}' plugin was successfully installed"));

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directroy filename to str")
                .contains(binary_filename)
        });
    assert_that(&installed).is_true();

    // SECOND INSTALLATION, NO FORCE, USES EXISTING BINARY
    let mut cmd = Command::new(cargo::cargo_bin!("rover"));
    cmd.env("APOLLO_HOME", temp_dir.clone());
    cmd.env("APOLLO_ELV2_LICENSE", "accept");
    cmd.args(args_without_force_option);
    let output = cmd.output().expect("Could not run command");
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new("exists, skipping install").unwrap();
    assert_that(&re.is_match(stderr)).is_true();
    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(binary_filename)
        });
    assert_that!(installed).is_true();

    // THIRD INSTALLATION, USES FORCE, BINARY EXISTS
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", temp_dir.as_path());
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(&forced_args);
            cmd
        },
        3,
    );
    let stderr = std::str::from_utf8(&output.stderr).expect("failed to convert bytes to a str");
    let re = Regex::new(&format!("the '{binary}' plugin was successfully installed")).unwrap();
    assert_that!(re.is_match(stderr)).is_true();
}

#[rstest]
// "1" is the CLI-facing spec for router's Federation-1-era latest track (`router@1`); orbiter's
// wire-protocol alias for it is "latest-plugin", not "latest-1" — RouterVersion::get_tarball_version
// is what actually produces that, so this test derives it the same way Plugin::get_tarball_url does
// rather than hardcoding it.
#[case::router_latest_1("router", "1")]
// supergraph_latest_0 (Federation 1) is intentionally absent: `rover install --plugin`
// now rejects Federation 1 versions outright.
// router's `latest-2` isn't covered here; `installs_router_2x` in `e2e_test_rover_install_plugin`
// (same file) already exercises `router@2` end-to-end, so it's not dropped coverage.
#[case::supergraph_latest_2("supergraph", "latest-2")]
#[tokio::test(flavor = "multi_thread")]
#[traced_test]
#[serial]
async fn e2e_test_rover_install_plugins_from_latest_version(
    #[case] binary_name: &str,
    #[case] cli_version_spec: &str,
) {
    let temp_dir = Utf8PathBuf::try_from(TempDir::new().unwrap().path().to_path_buf()).unwrap();
    let bin_path = temp_dir.join(".rover/bin");

    // orbiter owns the `latest-*` alias -> concrete version mapping (rover no longer keeps a
    // local copy). Ask orbiter directly via the same redirect-disabled-HEAD/X-Version contract
    // Installer::get_plugin_version already relies on in production, rather than reading a file
    // it owns. The target triple doesn't affect which version is resolved, so any triple that's
    // reliably released works here.
    let tarball_version_segment = match binary_name {
        "router" => RouterVersion::from_str(cli_version_spec)
            .expect("failed to parse router version spec")
            .get_tarball_version(),
        "supergraph" => FederationVersion::from_str(cli_version_spec)
            .expect("failed to parse supergraph version spec")
            .get_tarball_version(),
        other => panic!("unexpected binary name in test case: {other}"),
    };
    // Honor the same download-host override the `rover install` invocation below (and the
    // Installer it shares this logic with) respects, so this resolves against the overridden
    // host too instead of always hitting prod.
    let download_host = std::env::var("APOLLO_ROVER_DOWNLOAD_HOST")
        .unwrap_or_else(|_| "https://rover.apollo.dev".to_string());
    let tarball_url = format!(
        "{download_host}/tar/{binary_name}/x86_64-unknown-linux-gnu/{tarball_version_segment}"
    );
    let installer = Installer {
        binary_name: binary_name.to_string(),
        force_install: false,
        executable_location: temp_dir.clone(),
        override_install_path: None,
    };
    let installer_ref = &installer;
    let tarball_url_ref = tarball_url.as_str();
    let mut resolve_latest_version = ServiceBuilder::new()
        // Roughly matches the overall retry budget `run_with_retries` gives the subsequent
        // `rover install` call below (3 attempts x 5s sleep).
        .retry(ExponentialBackoffPolicy::new(Duration::from_secs(15)))
        .service(service_fn(move |_: ()| async move {
            installer_ref
                .get_plugin_version(tarball_url_ref, true)
                .await
        }));
    let latest_version_from_orbiter = resolve_latest_version
        .call(())
        .await
        .expect("failed to resolve latest version from orbiter");

    let plugin_arg = format!("{binary_name}@{latest_version_from_orbiter}");
    let output = run_with_retries(
        || {
            let mut cmd = Command::new(cargo::cargo_bin!("rover"));
            cmd.env("APOLLO_HOME", &temp_dir);
            cmd.env("APOLLO_ELV2_LICENSE", "accept");
            cmd.args(["install", "--plugin", &plugin_arg]);
            cmd
        },
        3,
    );

    asserting(&format!(
        "Was expecting success but instead got: {}",
        from_utf8(output.stderr.as_slice()).unwrap()
    ))
    .that(&output.status.success())
    .is_true();

    // THEN
    //   - it successfully installs
    let formatted_latest_version = latest_version_from_orbiter.replace("v", "-v");
    let downloaded_binary_name = format!("{binary_name}{formatted_latest_version}");

    let installed = bin_path
        .read_dir()
        .expect("unable to read contents of directory")
        .map(|f| f.expect("failed to get file {file:?} in ${temp_dir:?}"))
        .any(|f| {
            f.file_name()
                .to_str()
                .expect("failed to convert directory filename to str")
                .contains(&downloaded_binary_name)
        });

    assert_that!(installed).is_true();
}
