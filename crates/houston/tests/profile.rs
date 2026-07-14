use assert_fs::TempDir;
use camino::Utf8Path;
use config::Config;
use houston as config;
use rover_print::print::testing::TerminalCapture;
use speculoos::prelude::*;

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

    // each profile's credential must resolve independently, not just be
    // listed - guards against the two profiles' secrets being cross-wired.
    assert_eq!(
        config::Profile::get_credential(cprofile_name, &config)
            .expect("getting corporate credential failed")
            .api_key,
        cprofile_pass
    );
    assert_eq!(
        config::Profile::get_credential(pprofile_name, &config)
            .expect("getting personal credential failed")
            .api_key,
        pprofile_pass
    );

    // clearing must purge every profile's secret-store entry (not just the
    // on-disk index directories) without erroring.
    config
        .clear(&TerminalCapture::new(false))
        .expect("clearing configuration failed");

    assert_that!(config::Profile::list(&config).expect("listing profiles failed")).is_empty();
}

#[test]
fn it_lists_no_profiles_when_none_exist() {
    let config = get_config(None);

    let profiles = config::Profile::list(&config).expect("listing profiles failed");

    assert_that!(profiles).is_empty();
}

fn get_config(override_api_key: Option<String>) -> Config {
    let tmp_home = TempDir::new().unwrap();
    let tmp_home_path = Utf8Path::from_path(tmp_home.path()).unwrap().to_owned();
    Config::new(Some(&tmp_home_path), override_api_key).unwrap()
}
