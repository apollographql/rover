use assert_fs::TempDir;
use camino::Utf8Path;
use config::Config;
use houston as config;

#[test]
fn it_can_set_and_get_an_api_key() {
    let config = get_config(None);

    let profile = "default";
    let api_key = "superdupersecret";

    let profiles_home = config.home.join("profiles");
    let profile_home = profiles_home.join(profile);

    assert!(config::Profile::set_api_key(profile, &config, api_key).is_ok());

    // the profile directory still exists as an index of known profile names,
    // even though the credential itself now lives in the secret store.
    assert!(profile_home.exists());
    assert!(!profile_home.join(".sensitive").exists());

    assert_eq!(
        config::Profile::get_credential(profile, &config)
            .expect("retreiving api key for default profile failed")
            .api_key,
        String::from(api_key)
    );

    config::Profile::delete(profile, &config).expect("deleting default profile failed");

    assert!(!profile_home.exists());

    config.clear().expect("deleting profiles dir failed.");

    assert!(!profiles_home.exists());
}

#[test]
fn it_migrates_a_legacy_plaintext_credential() {
    let config = get_config(None);

    let profile = "default";
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
    let profile = "default";
    let api_key = "superdupersecret";
    let config = get_config(Some(api_key.to_string()));

    assert_eq!(
        config::Profile::get_credential(profile, &config)
            .expect("retreiving api key for default profile failed")
            .api_key,
        String::from(api_key)
    );
}

fn get_config(override_api_key: Option<String>) -> Config {
    let tmp_home = TempDir::new().unwrap();
    let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
    Config::new(Some(&tmp_home_path), override_api_key).unwrap()
}
