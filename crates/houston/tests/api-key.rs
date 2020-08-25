use houston as config;

use std::env;
use std::path::PathBuf;

#[test]
fn it_can_set_and_get_an_api_key_via_creds_file() {
    let config_home = PathBuf::from("./");
    env::set_var("APOLLO_CONFIG_HOME", &config_home);

    let profile = "default";
    let api_key = "superdupersecret";

    let profiles_home = config_home.join("profiles");
    let profile_home = profiles_home.join(profile);
    let sensitive_file = profile_home.join(".sensitive");

    assert!(config::Profile::set_api_key(profile, String::from(api_key)).is_ok());

    assert!(profile_home.exists());
    assert!(sensitive_file.exists());

    assert_eq!(
        config::Profile::get_api_key(profile)
            .expect("retreiving api key for default profile failed"),
        String::from(api_key)
    );

    config::Profile::delete(profile).expect("deleting default profile failed");

    assert!(!profile_home.exists());

    config::clear().expect("deleting profiles dir failed.");

    assert!(!profiles_home.exists());
}

#[test]
fn it_can_get_an_api_key_via_env_var() {
    let profile = "default";
    let api_key = "superdupersecret";
    env::set_var("APOLLO_KEY", api_key);

    assert_eq!(
        config::Profile::get_api_key(profile)
            .expect("retreiving api key for default profile failed"),
        String::from(api_key)
    );
}
