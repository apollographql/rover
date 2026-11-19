#[cfg(unix)]
use std::fs;
use std::{env, io::Write, path::PathBuf};

use rover_auth::external_token::{ExternalTokenOrigin, ExternalTokenSource};
use rstest::{fixture, rstest};
use serial_test::serial;
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};

// Keep every test in this target serial: cwd is shared by the whole process.
struct WorkingDirectory {
    original: PathBuf,
    directory: TempDir,
}

impl Drop for WorkingDirectory {
    fn drop(&mut self) {
        env::set_current_dir(&self.original).unwrap();
    }
}

#[fixture]
fn working_directory() -> WorkingDirectory {
    let original = env::current_dir().unwrap();
    let directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let mut token_file = NamedTempFile::new_in(directory.path()).unwrap();
    token_file.write_all(b"opaque-token").unwrap();
    token_file
        .persist(directory.path().join("access-token"))
        .unwrap();
    WorkingDirectory {
        original,
        directory,
    }
}

#[rstest]
#[tokio::test]
#[serial]
async fn relative_locations_survive_a_working_directory_change(
    working_directory: WorkingDirectory,
) {
    env::set_current_dir(working_directory.directory.path()).unwrap();
    let anchored = env::current_dir().unwrap().join("access-token");
    let source = ExternalTokenSource::new("access-token", ExternalTokenOrigin::Option);
    env::set_current_dir(&working_directory.original).unwrap();
    assert_that!(source.path()).is_equal_to(anchored.as_path());
    assert_that!(source.read().await.unwrap().secret()).is_equal_to("opaque-token");
}

#[cfg(unix)]
mod unix {
    use rover_auth::external_token::{ExternalTokenError, ExternalTokenFailure};

    use super::*;

    #[fixture]
    fn deleted_working_directory(working_directory: WorkingDirectory) -> WorkingDirectory {
        let deleted = working_directory.directory.path().join("deleted");
        fs::create_dir(&deleted).unwrap();
        env::set_current_dir(&deleted).unwrap();
        fs::remove_dir(&deleted).unwrap();
        working_directory
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn construction_defers_an_unavailable_working_directory(
        #[from(deleted_working_directory)] working_directory: WorkingDirectory,
    ) {
        let source = ExternalTokenSource::new("access-token", ExternalTokenOrigin::Environment);
        let expected = ExternalTokenError {
            origin: ExternalTokenOrigin::Environment,
            path: PathBuf::from("access-token"),
            kind: ExternalTokenFailure::UnresolvableLocation,
        };
        assert_that!(source.read().await)
            .is_err()
            .is_equal_to(&expected);
        assert_that!(expected.to_string()).is_equal_to(
            "External token source from APOLLO_ACCESS_TOKEN_FILE at 'access-token': the location cannot be resolved from the initial working directory; supply a valid absolute token-file path or invoke Rover from an accessible directory".to_string(),
        );
        env::set_current_dir(working_directory.directory.path()).unwrap();
        // Recovering cwd must not redirect the original source.
        assert_that!(source.read().await)
            .is_err()
            .is_equal_to(expected);
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn absolute_locations_work_without_a_working_directory(
        #[from(deleted_working_directory)] working_directory: WorkingDirectory,
    ) {
        let source = ExternalTokenSource::new(
            working_directory.directory.path().join("access-token"),
            ExternalTokenOrigin::Environment,
        );
        assert_that!(source.read().await.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[tokio::test]
    #[serial]
    async fn empty_locations_keep_their_error_without_a_working_directory(
        #[from(deleted_working_directory)] _working_directory: WorkingDirectory,
    ) {
        let source = ExternalTokenSource::new("", ExternalTokenOrigin::Environment);
        assert_that!(source.read().await)
            .is_err()
            .is_equal_to(ExternalTokenError {
                origin: ExternalTokenOrigin::Environment,
                path: PathBuf::new(),
                kind: ExternalTokenFailure::EmptyLocation,
            });
    }
}
