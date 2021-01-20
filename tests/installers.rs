use std::env;
use std::ffi::{OsStr, OsString};

use directories_next::BaseDirs;
use which::which;

#[test]
fn it_can_install_all_versions() {
    for release in get_releases() {
        let old_path = clear_path();
        assert!(which("rover").is_err());
        attempt_install(&release);
        assert!(which("rover").is_ok());
        set_path(&old_path);
    }
}

fn clear_path() -> OsString {
    let old_path = env::var_os("PATH").unwrap();
    env::remove_var("PATH");
    old_path
}

fn get_releases() -> Vec<String> {
    // TODO: query these from the GitHub API
    vec!["v0.0.1-rc.0".to_string()]
}

fn set_path<P: AsRef<OsStr>>(path: P) {
    env::set_var("PATH", path);
}

fn attempt_install(_release: &str) {
    // TODO: replace this with a call to a curl | sh install command specific to a release version
    let base_dirs = BaseDirs::new().unwrap();
    let new_path = base_dirs.home_dir().join(".rover").join("bin");
    set_path(new_path.as_os_str());
}
