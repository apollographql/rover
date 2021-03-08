use config::Config;
use houston as config;

use assert_fs::TempDir;
use camino::Utf8Path;

#[test]
fn it_can_set_and_get_an_api_key_via_creds_file() {
    let config = get_config(None);

    let profile = "default";
    let api_key = "superdupersecret";

    let profiles_home = config.home.join("profiles");
    let profile_home = profiles_home.join(profile);
    let sensitive_file = profile_home.join(".sensitive");

    assert!(config::Profile::set_api_key(profile, &config, api_key).is_ok());

    assert!(profile_home.exists());
    assert!(sensitive_file.exists());

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
