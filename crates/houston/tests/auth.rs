use assert_fs::TempDir;
use camino::Utf8Path;
use config::Config;
use houston as config;
use rover_print::print::testing::TerminalCapture;

#[test]
fn it_can_set_and_get_an_api_key() {
    let config = get_config(None);

    let profile = "default-key-roundtrip";
    let api_key = "superdupersecret";

    let profiles_home = config.home.join("profiles");
    let profile_home = profiles_home.join(profile);

    assert!(config::Profile::set_api_key(profile, &config, api_key).is_ok());

    // the profile directory still exists as an index of known profile names,
    // even though the credential itself now lives in the secret store.
    assert!(profile_home.exists());
    assert!(!profile_home.join(".sensitive").exists());

    let credential = config::Profile::get_credential(profile, &config)
        .expect("retreiving api key for default profile failed");
    assert_eq!(credential.api_key, String::from(api_key));
    assert_eq!(
        credential.origin,
        config::CredentialOrigin::ConfigFile(profile.to_string())
    );

    config::Profile::delete(profile, &config).expect("deleting default profile failed");

    assert!(!profile_home.exists());

    config
        .clear(&TerminalCapture::new(false))
        .expect("deleting profiles dir failed.");

    assert!(!profiles_home.exists());
}

#[test]
fn it_migrates_a_legacy_plaintext_credential() {
    let config = get_config(None);

    let profile = "legacy-migration";
    let api_key = "superdupersecret";

    // simulate a profile created by a version of Rover that predates the
    // secret store, storing its credential as a plaintext `.sensitive` TOML file.
    let profile_home = config.home.join("profiles").join(profile);
    std::fs::create_dir_all(profile_home.as_std_path()).unwrap();
    let sensitive_file = profile_home.join(".sensitive");
    std::fs::write(
        sensitive_file.as_std_path(),
        format!("api_key = \"{api_key}\"\n"),
    )
    .unwrap();

    assert_eq!(
        config::Profile::get_credential(profile, &config)
            .expect("retreiving api key for default profile failed")
            .api_key,
        String::from(api_key)
    );

    // the legacy file is migrated away on first read...
    assert!(!sensitive_file.exists());
    // ...and subsequent reads still succeed from the secret store.
    assert_eq!(
        config::Profile::get_credential(profile, &config)
            .expect("retreiving api key for default profile failed")
            .api_key,
        String::from(api_key)
    );
}

#[test]
fn it_can_get_an_api_key_via_env_var() {
    let profile = "env-var-override";
    let api_key = "superdupersecret";
    let config = get_config(Some(api_key.to_string()));

    assert_eq!(
        config::Profile::get_credential(profile, &config)
            .expect("retreiving api key for default profile failed")
            .api_key,
        String::from(api_key)
    );
}

#[test]
fn it_prioritizes_env_var_override_even_when_a_profile_credential_exists() {
    let profile = "override-precedence";
    let profile_key = "profile-based-key";
    let override_key = "env-override-key";

    let tmp_home = TempDir::new().unwrap();
    let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();

    let setup_config = Config::new(Some(&tmp_home_path), None).unwrap();
    config::Profile::set_api_key(profile, &setup_config, profile_key)
        .expect("setting api key failed");

    let config = Config::new(Some(&tmp_home_path), Some(override_key.to_string())).unwrap();
    let credential =
        config::Profile::get_credential(profile, &config).expect("retreiving api key failed");

    assert_eq!(credential.api_key, override_key);
    assert_eq!(credential.origin, config::CredentialOrigin::EnvVar);
}

#[test]
fn it_returns_profile_not_found_for_missing_profile_when_others_exist() {
    let config = get_config(None);
    config::Profile::set_api_key("existing-profile", &config, "some-key")
        .expect("setting api key failed");

    let error = config::Profile::get_credential("missing-profile", &config)
        .expect_err("expected a missing profile to error");

    assert!(matches!(error, config::HoustonProblem::ProfileNotFound(_)));
}

#[test]
fn it_returns_no_config_profiles_when_none_exist() {
    let config = get_config(None);

    let error = config::Profile::get_credential("anything", &config)
        .expect_err("expected an empty config to error");

    assert!(matches!(error, config::HoustonProblem::NoConfigProfiles));
}

#[test]
fn it_rejects_a_corrupted_legacy_credential() {
    let config = get_config(None);
    let profile = "corrupted-legacy";

    // historical Windows/PowerShell bug: older Rover versions could write a
    // profile's api_key as the single raw byte 22 (`` in TOML).
    let profile_home = config.home.join("profiles").join(profile);
    std::fs::create_dir_all(profile_home.as_std_path()).unwrap();
    std::fs::write(
        profile_home.join(".sensitive").as_std_path(),
        "api_key = \"\\u0016\"\n",
    )
    .unwrap();

    let error = config::Profile::get_credential(profile, &config)
        .expect_err("expected a corrupted legacy credential to error");

    assert!(matches!(error, config::HoustonProblem::CorruptedProfile(_)));
}

#[test]
fn it_rejects_a_corrupted_credential_via_current_api() {
    let config = get_config(None);
    let profile = "corrupted-current";

    config::Profile::set_api_key(profile, &config, "\u{16}").expect("setting api key failed");

    let error = config::Profile::get_credential(profile, &config)
        .expect_err("expected a corrupted credential to error");

    assert!(matches!(error, config::HoustonProblem::CorruptedProfile(_)));
}

#[test]
fn it_surfaces_malformed_legacy_toml_as_a_deserialization_error() {
    let config = get_config(None);
    let profile = "malformed-legacy";

    let profile_home = config.home.join("profiles").join(profile);
    std::fs::create_dir_all(profile_home.as_std_path()).unwrap();
    std::fs::write(
        profile_home.join(".sensitive").as_std_path(),
        "this is not valid toml {{{",
    )
    .unwrap();

    let error = config::Profile::get_credential(profile, &config)
        .expect_err("expected malformed legacy toml to error");

    assert!(matches!(
        error,
        config::HoustonProblem::TomlDeserialization(_)
    ));
}

#[test]
fn it_does_not_leak_an_orphaned_secret_after_delete() {
    let config = get_config(None);
    let profile = "delete-then-recreate-dir";

    config::Profile::set_api_key(profile, &config, "some-key").expect("setting api key failed");
    config::Profile::delete(profile, &config).expect("deleting profile failed");

    // recreate only the profile's index directory, without writing a new
    // secret, to confirm the old secret wasn't left behind (orphaned) in the
    // secret store after delete.
    let profile_home = config.home.join("profiles").join(profile);
    std::fs::create_dir_all(profile_home.as_std_path()).unwrap();

    let error = config::Profile::get_credential(profile, &config)
        .expect_err("expected no credential to be found after delete");

    // no secret and no legacy `.sensitive` file means the read falls through
    // to a filesystem error trying to open the (nonexistent) legacy file.
    assert!(matches!(error, config::HoustonProblem::RoverStdError(_)));
}

#[test]
fn it_errors_cleanly_when_deleting_a_profile_that_does_not_exist() {
    let config = get_config(None);

    let error = config::Profile::delete("never-created", &config)
        .expect_err("expected deleting a nonexistent profile to error");

    assert!(matches!(error, config::HoustonProblem::RoverStdError(_)));
}

#[test]
fn it_rejects_a_non_directory_override_home() {
    let tmp_dir = TempDir::new().unwrap();
    let file_path = tmp_dir.path().join("not-a-directory");
    std::fs::write(&file_path, "").unwrap();
    let path = Utf8Path::from_path(&file_path).unwrap().to_owned();

    let error = Config::new(Some(&path), None)
        .expect_err("expected a non-directory override home to error");

    assert!(matches!(
        error,
        config::HoustonProblem::InvalidOverrideConfigDir(_)
    ));
}

#[test]
fn it_errors_when_clearing_an_already_cleared_config() {
    let config = get_config(None);
    config::Profile::set_api_key("clear-twice", &config, "some-key")
        .expect("setting api key failed");

    config
        .clear(&TerminalCapture::new(false))
        .expect("clearing configuration failed");
    let error = config
        .clear(&TerminalCapture::new(false))
        .expect_err("expected clearing an already-cleared config to error");

    assert!(matches!(error, config::HoustonProblem::NoConfigFound(_)));
}

fn get_config(override_api_key: Option<String>) -> Config {
    let tmp_home = TempDir::new().unwrap();
    let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
    Config::new(Some(&tmp_home_path), override_api_key).unwrap()
}
