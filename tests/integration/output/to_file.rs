use std::fs;

use assert_cmd::Command;
use camino::Utf8PathBuf;
use houston::{Config, Profile};
use rover::utils::env::RoverEnvKey;
use rstest::rstest;
use speculoos::{assert_that, prelude::BooleanAssertions};
use tempfile::TempDir;

const CUSTOM_PROFILE: &str = "custom-profile";
const CUSTOM_API_KEY: &str = "custom-api-key";

#[rstest]
#[case::simple_file_path("profiles.json", vec![], vec![], Utf8PathBuf::from("profiles.json"), false)]
#[case::simple_file_path_local("./profiles.json", vec![], vec![], Utf8PathBuf::from("profiles.json"), false)]
#[case::file_path_with_prefix("a/b/c/profiles.json", vec![Utf8PathBuf::from("a/b/c")], vec![], Utf8PathBuf::from("a/b/c/profiles.json"), false)]
#[case::file_path_with_prefix_failure_due_to_intermediate_files("a/b/c/profiles.json", vec![Utf8PathBuf::from("a")], vec![Utf8PathBuf::from("a/b")], Utf8PathBuf::from("a/b/c/profiles.json"), true)]
#[case::file_path_with_prefix_missing_inermediate("a/b/c/profiles.json", vec![Utf8PathBuf::from("a/b")], vec![], Utf8PathBuf::from("a/b/c/profiles.json"), false)]
#[case::complex_path("a/b/../../a/b/c/profiles.json", vec![Utf8PathBuf::from("a/b/c")], vec![], Utf8PathBuf::from("a/b/c/profiles.json"), false)]
#[case::complex_path_failure_due_to_intermediate_files("a/b/../../a/b/c/profiles.json", vec![Utf8PathBuf::from("a")], vec![Utf8PathBuf::from("a/b")], Utf8PathBuf::from("a/b/c/profiles.json"), true)]
#[case::complex_path_missing_intermediates("a/b/../../a/b/c/profiles.json", vec![Utf8PathBuf::from("a/b")], vec![], Utf8PathBuf::from("a/b/c/profiles.json"), false)]
fn it_can_write_files_correctly_no_matter_the_input_path(
    #[case] output_argument: &str,
    #[case] existing_paths: Vec<Utf8PathBuf>,
    #[case] existing_files: Vec<Utf8PathBuf>,
    #[case] expected_output_path: Utf8PathBuf,
    #[case] should_fail: bool,
) {
    let temp_config_dir = TempDir::new().expect("Could not create temporary directory");
    let temp_config_dir_path = Utf8PathBuf::from_path_buf(temp_config_dir.keep()).unwrap();
    let config = Config::new(Some(&temp_config_dir_path).as_ref(), None).unwrap();
    Profile::set_api_key(CUSTOM_PROFILE, &config, CUSTOM_API_KEY).unwrap();

    let temp_output_dir = TempDir::new().expect("Could not create temporary directory");
    let temp_output_dir_path = Utf8PathBuf::from_path_buf(temp_output_dir.keep()).unwrap();
    // Create any directories we want to exist in advance
    for existing_path in existing_paths {
        let path_to_create = temp_output_dir_path.join(existing_path);
        fs::create_dir_all(path_to_create).expect("Could not create existing directories");
    }
    // Create any files we want to exist in advance
    for existing_file in existing_files {
        let file = temp_output_dir_path.join(existing_file);
        fs::write(file, "foo bar bash").expect("Could not create existing directories");
    }

    let mut starter_cmd = Command::cargo_bin("rover").unwrap();
    starter_cmd
        .env(RoverEnvKey::ConfigHome.to_string(), temp_config_dir_path)
        .args(vec![
            "config",
            "list",
            "--format",
            "json",
            "--output",
            output_argument,
        ])
        .current_dir(&temp_output_dir_path)
        .assert()
        .success();

    if should_fail {
        assert_that(&temp_output_dir_path.join(expected_output_path).exists()).is_false()
    } else {
        assert_that(&temp_output_dir_path.join(expected_output_path).exists()).is_true()
    }
}
