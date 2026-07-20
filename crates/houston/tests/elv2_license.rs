use assert_fs::TempDir;
use camino::Utf8Path;
use config::Config;
use houston as config;

#[test]
fn it_defaults_to_elv2_not_accepted() {
    let config = get_config();

    assert!(!config.did_accept_elv2_license());
}

#[test]
fn it_remembers_elv2_license_acceptance() {
    let config = get_config();

    config
        .remember_elv2_license_accept()
        .expect("remembering elv2 license acceptance failed");

    assert!(config.did_accept_elv2_license());
}

fn get_config() -> Config {
    let tmp_home = TempDir::new().unwrap();
    let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
    Config::new(Some(&tmp_home_path), None).unwrap()
}
