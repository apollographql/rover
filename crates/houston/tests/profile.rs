use assert_fs::TempDir;
use config::Config;
use houston as config;

#[test]
fn it_lists_many_profiles() {
    let config = get_config(None);
    let cprofile_name = "corporate";
    let cprofile_pass = "corporatepassword";

    config::Profile::set_api_key(cprofile_name, &config, cprofile_pass)
        .expect("setting api key failed");

    let pprofile_name = "personal";
    let pprofile_pass = "personalpassword";

    config::Profile::set_api_key(pprofile_name, &config, pprofile_pass)
        .expect("setting api key failed");

    let profiles = config::Profile::list(&config).expect("listing profiles failed");

    assert!(profiles.len() == 2);
    assert!(profiles.contains(&String::from(pprofile_name)));
    assert!(profiles.contains(&String::from(cprofile_name)));

    config.clear().expect("clearing configuration failed");
}

fn get_config(override_api_key: Option<String>) -> Config {
    let tmp_home = TempDir::new().unwrap();
    Config::new(Some(&tmp_home.path()), override_api_key).unwrap()
}
