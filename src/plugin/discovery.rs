//! Finding the two levels a plugin can be declared at: the global level every
//! project on the machine shares, and the project level of the working
//! directory, if it has one.

use std::{fs, io};

use camino::{Utf8Path, Utf8PathBuf};

use crate::PKG_NAME;

/// The directory, at either level, that holds `rover.yaml` and its lockfile.
pub const ROVER_DIR: &str = ".rover";

/// The directories Rover reads declarations from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestDirs {
    /// The global level's `.rover/` directory, which need not exist. `None`
    /// only when neither `APOLLO_HOME` nor a home directory is available.
    pub global: Option<Utf8PathBuf>,
    /// The project level's `.rover/` directory, or `None` when the working
    /// directory is not inside a project. Rover never invents one.
    pub project: Option<Utf8PathBuf>,
}

impl ManifestDirs {
    /// Find both levels for a command run from `cwd`. A relative `cwd`
    /// resolves against the process's working directory.
    ///
    /// `apollo_home` is the value of `APOLLO_HOME`, if set, and `home` is the
    /// user's home directory, if there is one; together they place the global
    /// level exactly where plugin binaries are installed today.
    pub fn discover(
        cwd: &Utf8Path,
        apollo_home: Option<&Utf8Path>,
        home: Option<&Utf8Path>,
    ) -> Self {
        let home = home.filter(|home| !home.as_str().is_empty());
        let global = global_dir(apollo_home, home);
        let project = discover_project(cwd, home, global.as_deref());

        Self { global, project }
    }

    /// Find both levels for a command run from the process's working
    /// directory, on this machine. `apollo_home` is as for [`Self::discover`].
    pub fn for_this_process(apollo_home: Option<&Utf8Path>) -> io::Result<Self> {
        let cwd = Utf8PathBuf::try_from(std::env::current_dir()?).map_err(io::Error::other)?;
        // No home directory just means no home to stop the search at.
        let home = binstall::get_home_dir_path().ok();
        Ok(Self::discover(&cwd, apollo_home, home.as_deref()))
    }
}

/// The project in scope according to `found`, a search that may have failed.
///
/// A working directory Rover can't read, because it was deleted or its path
/// isn't UTF-8, has no project to find, so failing to look means there is
/// none: the global level still applies, as it does when no `.rover/` is
/// found (FR22). Only the debug log says why.
pub fn project_in_scope(found: io::Result<ManifestDirs>) -> Option<Utf8PathBuf> {
    match found {
        Ok(dirs) => dirs.project,
        Err(err) => {
            tracing::debug!("couldn't look for a project, so using only the global level: {err}");
            None
        }
    }
}

/// The global level's `.rover/` directory: `$APOLLO_HOME/.rover` when
/// `APOLLO_HOME` is set and not empty, `~/.rover` otherwise.
///
/// It is Rover's own directory, where the installer puts plugin binaries under
/// `bin/`, so it comes from the installer's rule rather than a copy of it.
pub fn global_dir(apollo_home: Option<&Utf8Path>, home: Option<&Utf8Path>) -> Option<Utf8PathBuf> {
    binstall::base_dir(PKG_NAME, apollo_home, home)
}

/// The project level for `cwd`: the `.rover/` directory in `cwd` or its
/// nearest ancestor that has one, as Cargo and git search.
///
/// The search stops at the first `.rover/` directory it finds, whatever that
/// directory holds, since an empty one is a valid, if empty, project; but if
/// that directory is the global level's, there is no project. The search also
/// stops, finding nothing, when it reaches `home`: the `.rover/` there is the
/// global level (or where it used to be, before `APOLLO_HOME` moved it), and
/// whether a directory under home is a project must not depend on whether
/// anything has been installed there yet.
///
/// Paths are compared, and the project is returned, with symlinks resolved,
/// so a home directory reached through a symlink is still recognized.
pub fn discover_project(
    cwd: &Utf8Path,
    home: Option<&Utf8Path>,
    global: Option<&Utf8Path>,
) -> Option<Utf8PathBuf> {
    // Made absolute first, so that a relative `cwd` that cannot be resolved
    // still has every real ancestor to search.
    let cwd = std::path::absolute(cwd)
        .ok()
        .and_then(|cwd| Utf8PathBuf::from_path_buf(cwd).ok())
        .map_or_else(|| canonical(cwd), |cwd| canonical(&cwd));
    let home = home.map(canonical);
    let global = global.map(canonical);

    for dir in cwd.ancestors() {
        if home.as_deref().is_some_and(|home| same(home, dir)) {
            return None;
        }

        let candidate = dir.join(ROVER_DIR);
        if occupies(&candidate) {
            // The global level's `.rover/` may itself be a symlink, so it
            // has to be resolved before it can be recognized.
            let candidate = canonical(&candidate);
            let is_global = global
                .as_deref()
                .is_some_and(|global| same(global, &candidate));
            return (!is_global).then_some(candidate);
        }
    }

    None
}

/// Whether `candidate` is a `.rover/` that ends the search. A directory is;
/// a file named `.rover` is not, and the search passes it over. Anything that
/// cannot be told apart from a directory — a symlink to nowhere, an entry
/// that cannot be inspected — ends the search too, rather than let it climb
/// past what may be the project into some other one; reading the manifest
/// there then reports what is wrong with it.
fn occupies(candidate: &Utf8Path) -> bool {
    match fs::metadata(candidate) {
        Ok(metadata) => metadata.is_dir(),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            candidate.symlink_metadata().is_ok()
        }
        Err(_) => true,
    }
}

/// Whether two resolved paths name the same directory. Windows keeps the
/// verbatim prefix (`\\?\`, or `\\?\UNC\` for a network share) on a
/// resolved path too long to do without it, so a deep working directory's
/// ancestors can carry it while home, resolved on its own, does not.
fn same(a: &Utf8Path, b: &Utf8Path) -> bool {
    fn plain(path: &Utf8Path) -> std::borrow::Cow<'_, str> {
        let path = path.as_str();
        match path.strip_prefix(r"\\?\UNC\") {
            Some(share) => format!(r"\\{share}").into(),
            None => path.strip_prefix(r"\\?\").unwrap_or(path).into(),
        }
    }
    Utf8Path::new(plain(a).as_ref()) == Utf8Path::new(plain(b).as_ref())
}

/// `path` with symlinks and relative components resolved, or unchanged if the
/// filesystem cannot resolve it (a global level not created yet, for one).
fn canonical(path: &Utf8Path) -> Utf8PathBuf {
    dunce::canonicalize(path)
        .ok()
        .and_then(|resolved| Utf8PathBuf::from_path_buf(resolved).ok())
        .unwrap_or_else(|| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use assert_fs::TempDir;
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::*;

    /// A directory tree for one case, and its root with symlinks resolved.
    /// Each entry is a path relative to the root; one ending in `/` is created
    /// as a directory, anything else as an empty file along with its parents.
    fn tree(entries: &[&str]) -> (TempDir, Utf8PathBuf) {
        let temp = TempDir::new().unwrap();
        let root = canonical(&Utf8PathBuf::try_from(temp.path().to_path_buf()).unwrap());
        for entry in entries {
            let path = root.join(entry.trim_end_matches('/'));
            if entry.ends_with('/') {
                fs::create_dir_all(&path).unwrap();
            } else {
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, "").unwrap();
            }
        }
        (temp, root)
    }

    // Every case runs under `home/`, which stops a search that finds nothing
    // before climbing into the real filesystem above the temporary directory.
    #[rstest]
    #[case::walks_up_from_a_nested_directory(
        &["home/work/.rover/rover.yaml", "home/work/packages/a/b/"],
        "home/work/packages/a/b",
        Some("home/work/.rover")
    )]
    #[case::finds_one_in_the_working_directory(
        &["home/work/.rover/rover.yaml"],
        "home/work",
        Some("home/work/.rover")
    )]
    #[case::the_nearest_one_wins(
        &["home/work/.rover/rover.yaml", "home/work/packages/b/.rover/rover.yaml", "home/work/packages/b/src/"],
        "home/work/packages/b/src",
        Some("home/work/packages/b/.rover")
    )]
    #[case::an_empty_one_still_stops_the_search(
        &["home/work/.rover/rover.yaml", "home/work/packages/b/.rover/"],
        "home/work/packages/b",
        Some("home/work/packages/b/.rover")
    )]
    #[case::one_with_only_a_lockfile_is_a_project(
        &["home/work/.rover/plugin-versions.lock"],
        "home/work",
        Some("home/work/.rover")
    )]
    #[case::a_file_named_dot_rover_is_passed_over(
        &["home/work/.rover/rover.yaml", "home/work/packages/.rover"],
        "home/work/packages",
        Some("home/work/.rover")
    )]
    #[case::home_is_not_a_project(
        &["home/.rover/rover.yaml", "home/work/packages/"],
        "home/work/packages",
        None
    )]
    #[case::home_itself_is_not_a_project(&["home/.rover/rover.yaml"], "home", None)]
    #[case::the_global_level_is_not_a_project_from_inside_it(
        &["home/.rover/bin/"],
        "home/.rover/bin",
        None
    )]
    #[case::nothing_above_home_is_a_project_either(
        &[".rover/rover.yaml", "home/work/"],
        "home/work",
        None
    )]
    #[case::a_search_from_outside_home_is_not_stopped_by_it(
        &[".rover/rover.yaml", "home/", "srv/app/"],
        "srv/app",
        Some(".rover")
    )]
    fn a_project_is_found_by_walking_up(
        #[case] entries: &[&str],
        #[case] cwd: &str,
        #[case] expected: Option<&str>,
    ) {
        let (_temp, root) = tree(entries);
        let home = root.join("home");

        let dirs = ManifestDirs::discover(&root.join(cwd), None, Some(&home));

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(home.join(ROVER_DIR)),
            project: expected.map(|project| root.join(project)),
        });
    }

    #[rstest]
    #[case::the_relocated_global_level_is_not_a_project(
        &["home/", "apollo-home/.rover/", "apollo-home/work/"],
        "apollo-home/work",
        None
    )]
    #[case::nor_is_a_leftover_one_under_home(
        &["home/.rover/rover.yaml", "apollo-home/", "home/work/"],
        "home/work",
        None
    )]
    #[case::a_project_under_home_is_still_found(
        &["apollo-home/", "home/work/.rover/"],
        "home/work",
        Some("home/work/.rover")
    )]
    fn apollo_home_moves_the_global_level(
        #[case] entries: &[&str],
        #[case] cwd: &str,
        #[case] expected: Option<&str>,
    ) {
        let (_temp, root) = tree(entries);
        let apollo_home = root.join("apollo-home");

        let dirs = ManifestDirs::discover(
            &root.join(cwd),
            Some(&apollo_home),
            Some(&root.join("home")),
        );

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(apollo_home.join(ROVER_DIR)),
            project: expected.map(|project| root.join(project)),
        });
    }

    /// A project's `.rover/` and the global one share a name, and the search
    /// tells them apart by path. That only works while the name discovery
    /// looks for is the one the installer creates.
    #[test]
    fn projects_use_the_directory_name_the_installer_creates() {
        let global = global_dir(None, Some(Utf8Path::new("/home/me"))).unwrap();
        assert_that!(global.file_name()).is_equal_to(Some(ROVER_DIR));
    }

    #[rstest]
    #[case::apollo_home_wins(Some("/opt/apollo"), Some("/home/me"), Some("/opt/apollo/.rover"))]
    #[case::home_by_default(None, Some("/home/me"), Some("/home/me/.rover"))]
    #[case::an_empty_apollo_home_is_unset(Some(""), Some("/home/me"), Some("/home/me/.rover"))]
    #[case::apollo_home_needs_no_home(Some("/opt/apollo"), None, Some("/opt/apollo/.rover"))]
    #[case::an_empty_home_is_none(None, Some(""), None)]
    #[case::neither(None, None, None)]
    fn the_global_level_is_where_plugins_install(
        #[case] apollo_home: Option<&str>,
        #[case] home: Option<&str>,
        #[case] expected: Option<&str>,
    ) {
        assert_that!(global_dir(
            apollo_home.map(Utf8Path::new),
            home.map(Utf8Path::new)
        ))
        .is_equal_to(expected.map(Utf8PathBuf::from));
    }

    // Creating a symlink on Windows needs a privilege a test cannot count on.
    #[cfg(unix)]
    #[rstest]
    fn home_is_recognized_through_a_symlink() {
        let (_temp, root) = tree(&["real-home/.rover/", "real-home/work/"]);
        let home = root.join("home");
        std::os::unix::fs::symlink(root.join("real-home"), &home).unwrap();

        let dirs = ManifestDirs::discover(&root.join("real-home/work"), None, Some(&home));

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(home.join(ROVER_DIR)),
            project: None,
        });
    }

    // Creating a symlink on Windows needs a privilege a test cannot count on.
    #[cfg(unix)]
    #[rstest]
    fn a_dot_rover_that_links_nowhere_stops_the_search() {
        let (_temp, root) = tree(&["home/app/.rover/", "home/app/packages/"]);
        let link = root.join("home/app/packages/.rover");
        std::os::unix::fs::symlink(root.join("moved"), &link).unwrap();

        let dirs = ManifestDirs::discover(
            &root.join("home/app/packages"),
            None,
            Some(&root.join("home")),
        );

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(root.join("home").join(ROVER_DIR)),
            project: Some(link),
        });
    }

    #[rstest]
    fn a_path_through_a_parent_component_is_searched_as_the_directory_it_names() {
        let (_temp, root) = tree(&["home/a/b/.rover/", "home/a/c/"]);

        // Lexically, `a/b/..` has `a/b` as an ancestor; really it is `a`,
        // whose ancestors do not include `a/b`.
        let dirs =
            ManifestDirs::discover(&root.join("home/a/b/../c"), None, Some(&root.join("home")));

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(root.join("home").join(ROVER_DIR)),
            project: None,
        });
    }

    // Creating a symlink on Windows needs a privilege a test cannot count on.
    #[cfg(unix)]
    #[rstest]
    fn a_global_level_that_is_a_symlink_is_not_a_project() {
        let (_temp, root) = tree(&["rover-data/", "apollo-home/work/", "home/"]);
        let apollo_home = root.join("apollo-home");
        std::os::unix::fs::symlink(root.join("rover-data"), apollo_home.join(ROVER_DIR)).unwrap();

        let dirs = ManifestDirs::discover(
            &apollo_home.join("work"),
            Some(&apollo_home),
            Some(&root.join("home")),
        );

        assert_that!(dirs).is_equal_to(ManifestDirs {
            global: Some(apollo_home.join(ROVER_DIR)),
            project: None,
        });
    }

    #[rstest]
    #[case::found(Ok(ManifestDirs { global: None, project: Some("/work/app/.rover".into()) }), Some("/work/app/.rover"))]
    #[case::none_found(Ok(ManifestDirs { global: None, project: None }), None)]
    #[case::a_working_directory_that_cannot_be_read(
        Err(io::Error::from(io::ErrorKind::NotFound)),
        None
    )]
    fn a_search_that_fails_finds_no_project(
        #[case] found: io::Result<ManifestDirs>,
        #[case] expected: Option<&str>,
    ) {
        assert_that!(project_in_scope(found)).is_equal_to(expected.map(Utf8PathBuf::from));
    }

    // A deleted working directory is only possible to arrange on Unix.
    #[cfg(unix)]
    #[rstest]
    fn a_deleted_working_directory_is_no_project() {
        // Run in a child, since the working directory is the whole process's.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "plugin::discovery::tests::deleted_working_directory_child",
                "--include-ignored",
                "--nocapture",
            ])
            .env("ROVER_TEST_DELETED_CWD_CHILD", "1")
            .output()
            .unwrap();

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let reported: Vec<&str> = stdout
            .lines()
            .filter(|line| line.starts_with("error: ") || line.starts_with("project: "))
            .collect();
        assert_that!(reported)
            .named(&stdout)
            .is_equal_to(vec!["error: true", "project: None"]);
    }

    /// Run only by [`a_deleted_working_directory_is_no_project`], in its own
    /// process: deletes its working directory and looks for a project.
    #[cfg(unix)]
    #[test]
    #[ignore = "run by a_deleted_working_directory_is_no_project in a child process"]
    fn deleted_working_directory_child() {
        if std::env::var_os("ROVER_TEST_DELETED_CWD_CHILD").is_none() {
            return;
        }
        let temp = TempDir::new().unwrap();
        let cwd = temp.path().join("gone");
        fs::create_dir(&cwd).unwrap();
        std::env::set_current_dir(&cwd).unwrap();
        fs::remove_dir(&cwd).unwrap();

        let found = ManifestDirs::for_this_process(None);
        println!("error: {}", found.is_err());
        println!("project: {:?}", project_in_scope(found));
    }

    #[rstest]
    #[case::the_same_path(r"C:\Users\me", r"C:\Users\me", true)]
    #[case::one_with_the_long_path_prefix(r"\\?\C:\Users\me", r"C:\Users\me", true)]
    #[case::a_share_with_the_long_path_prefix(r"\\?\UNC\srv\users\me", r"\\srv\users\me", true)]
    #[case::different_paths(r"\\?\C:\Users\me", r"C:\Users\you", false)]
    fn the_windows_long_path_prefix_does_not_change_a_path(
        #[case] a: &str,
        #[case] b: &str,
        #[case] expected: bool,
    ) {
        assert_that!(same(Utf8Path::new(a), Utf8Path::new(b))).is_equal_to(expected);
    }
}
