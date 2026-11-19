use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use rstest::{fixture, rstest};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};
use tower::ServiceExt;

use super::*;

struct TokenFile {
    directory: TempDir,
    source: ExternalTokenSource,
}

impl TokenFile {
    fn publish(&self, contents: &[u8]) {
        let mut replacement = NamedTempFile::new_in(self.directory.path()).unwrap();
        replacement.write_all(contents).unwrap();
        // std's Windows rename supports replacing a destination that is still
        // open by a reader; tempfile's persist only uses MoveFileExW.
        let replacement = replacement.into_temp_path();
        fs::rename(&replacement, self.source.path()).unwrap();
    }
}

#[fixture]
fn token_file() -> TokenFile {
    let directory = tempfile::tempdir().unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(directory.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }
    let source = ExternalTokenSource::new(
        directory.path().join("access-token"),
        ExternalTokenOrigin::Option,
    );
    let fixture = TokenFile { directory, source };
    fixture.publish(b"opaque-token");
    fixture
}

#[rstest]
#[case::opaque("opaque-token")]
#[case::alphabet("Az09-._~+/")]
#[case::one_padding("token=")]
#[case::many_padding("token====")]
#[case::jwt("eyJhbGciOiJub25lIn0.eyJleHAiOjF9.signature")]
fn accepts_raw_bearer_tokens_without_inspecting_claims(
    #[case] token: &str,
    #[values("", "\n", "\r\n")] ending: &str,
) {
    let bytes = format!("{token}{ending}").into_bytes();
    assert_that!(ExternalAccessToken::try_from(bytes).unwrap().secret()).is_equal_to(token);
}

#[rstest]
#[case::empty(b"")]
#[case::lf(b"\n")]
#[case::crlf(b"\r\n")]
#[case::leading_space(b" token")]
#[case::trailing_space(b"token ")]
#[case::internal_space(b"tok en")]
#[case::tab(b"tok\ten")]
#[case::carriage_return(b"token\r")]
#[case::double_lf(b"token\n\n")]
#[case::double_crlf(b"token\r\n\r\n")]
#[case::extra_line(b"token\nother")]
#[case::bearer(b"Bearer token")]
#[case::header(b"Authorization: Bearer token")]
#[case::bom(b"\xef\xbb\xbftoken")]
#[case::invalid_utf8(b"\xff")]
#[case::unicode("tökén".as_bytes())]
#[case::colon(b"user:token")]
#[case::nul(b"tok\0en")]
#[case::padding_only(b"===")]
#[case::internal_padding(b"to=ken")]
#[case::padding_before_suffix(b"token=+")]
#[case::quoted(b"\"token\"")]
fn rejects_invalid_token_contents_without_returning_them(#[case] bytes: &[u8]) {
    assert_that!(ExternalAccessToken::try_from(bytes.to_vec()))
        .is_err()
        .is_equal_to(ExternalTokenFailure::InvalidContents);
}

#[rstest]
#[case::without_ending("", MAX_TOKEN_FILE_BYTES)]
#[case::with_lf("\n", MAX_TOKEN_FILE_BYTES - 1)]
#[case::with_crlf("\r\n", MAX_TOKEN_FILE_BYTES - 2)]
#[tokio::test]
async fn accepts_exactly_sixteen_kib_including_the_line_ending(
    token_file: TokenFile,
    #[case] ending: &str,
    #[case] token_length: usize,
) {
    let token = "a".repeat(token_length);
    token_file.publish(format!("{token}{ending}").as_bytes());
    assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to(token.as_str());
}

#[rstest]
#[tokio::test]
async fn refuses_oversize_files_before_reading_contents(token_file: TokenFile) {
    token_file.publish(&vec![b'a'; MAX_TOKEN_FILE_BYTES + 1]);
    assert_that!(token_file.source.read().await)
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::TooLarge));
}

struct EndlessReader {
    bytes_read: usize,
}

impl Read for EndlessReader {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        buffer.fill(b'a');
        self.bytes_read += buffer.len();
        Ok(buffer.len())
    }
}

#[test]
fn reads_at_most_the_limit_plus_one_byte_even_without_eof() {
    let mut reader = EndlessReader { bytes_read: 0 };
    assert_that!(ExternalAccessToken::read_from(&mut reader))
        .is_err()
        .is_equal_to(ExternalTokenFailure::TooLarge);
    assert_that!(reader.bytes_read).is_equal_to(MAX_TOKEN_FILE_BYTES + 1);
}

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::Error::other("untrusted error text"))
    }
}

#[test]
fn read_errors_do_not_retain_untrusted_error_text() {
    assert_that!(ExternalAccessToken::read_from(FailingReader))
        .is_err()
        .is_equal_to(ExternalTokenFailure::Unreadable);
}

#[rstest]
#[case::option(ExternalTokenOrigin::Option, "--access-token-file")]
#[case::environment(ExternalTokenOrigin::Environment, "APOLLO_ACCESS_TOKEN_FILE")]
#[tokio::test]
async fn empty_sources_fail_on_read_with_their_provenance(
    #[case] origin: ExternalTokenOrigin,
    #[case] name: &str,
) {
    let source = ExternalTokenSource::new("", origin);
    assert_that!(source.origin()).is_equal_to(origin);
    assert_that!(source.path()).is_equal_to(Path::new(""));
    let result = source.read().await;
    let error = assert_that!(result).is_err().subject;
    assert_that!(error).is_equal_to(&ExternalTokenError {
        origin,
        path: PathBuf::new(),
        kind: ExternalTokenFailure::EmptyLocation,
    });
    assert_that!(error.to_string()).is_equal_to(format!(
        "External token source from {name}: the location is empty; supply the path to the application's token file"
    ));
}

#[rstest]
#[tokio::test]
async fn construction_does_not_open_or_require_the_file(token_file: TokenFile) {
    fs::remove_file(token_file.source.path()).unwrap();
    let mut source =
        ExternalTokenSource::new(token_file.source.path(), ExternalTokenOrigin::Environment);
    assert_that!(
        source
            .ready()
            .await
            .map(|source| source.path().to_path_buf())
    )
    .is_equal_to(Ok(token_file.source.path().to_path_buf()));
    assert_that!(source.read().await)
        .is_err()
        .is_equal_to(source.error(ExternalTokenFailure::Missing));
}

#[rstest]
#[tokio::test]
async fn consumers_can_use_file_sources_as_services(token_file: TokenFile) {
    let result = token_file
        .source
        .clone()
        .boxed()
        .map_response(|token| token.secret().to_owned())
        .oneshot(())
        .await;
    assert_that!(result).is_equal_to(Ok("opaque-token".to_string()));
}

#[tokio::test]
async fn consumers_can_use_in_memory_sources_as_services() {
    let token = ExternalAccessToken::try_from(b"opaque-token".to_vec()).unwrap();
    let source = tower::service_fn(move |()| {
        let token = token.clone();
        async move { Ok::<_, ExternalTokenError>(token) }
    })
    .boxed();
    let result = source
        .map_response(|token| token.secret().to_owned())
        .oneshot(())
        .await;
    assert_that!(result).is_equal_to(Ok("opaque-token".to_string()));
}

#[rstest]
fn service_reads_wait_for_a_blocking_worker_without_blocking_the_runtime(token_file: TokenFile) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, waiting) = tokio::sync::oneshot::channel();
        let worker = tokio::task::spawn_blocking(move || {
            started.send(()).unwrap();
            blocked.recv_timeout(std::time::Duration::from_secs(5))
        });
        waiting.await.unwrap();

        let read = token_file.source.clone().oneshot(());
        tokio::pin!(read);
        let state = futures::poll!(&mut read);
        release.send(()).unwrap();
        assert_that!(state).is_equal_to(std::task::Poll::Pending);
        assert_that!(read.await.unwrap().secret()).is_equal_to("opaque-token");
        assert_that!(worker.await.unwrap()).is_equal_to(Ok(()));
    });
}

#[rstest]
fn service_reports_an_unavailable_blocking_worker_without_exposing_task_errors(
    token_file: TokenFile,
) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();
    let handle = runtime.handle().clone();
    drop(runtime);
    let _entered = handle.enter();
    let result = futures::executor::block_on(token_file.source.clone().oneshot(()));
    assert_that!(result).is_equal_to(Err(token_file
        .source
        .error(ExternalTokenFailure::Unreadable)));
}

#[rstest]
#[tokio::test]
async fn rereads_replacements_without_mutating_previous_tokens(token_file: TokenFile) {
    let first = token_file.source.read().await.unwrap();
    token_file.publish(b"replacement-token\r\n");
    let second = token_file.source.read().await.unwrap();
    assert_that!(first.secret()).is_equal_to("opaque-token");
    assert_that!(second.secret()).is_equal_to("replacement-token");
    assert_that!(fs::read(token_file.source.path()).unwrap())
        .is_equal_to(b"replacement-token\r\n".to_vec());
    assert_that!(fs::read_dir(token_file.directory.path()).unwrap().count()).is_equal_to(1);
}

#[rstest]
#[tokio::test]
async fn failures_never_reuse_a_token_and_a_repaired_source_can_be_read(token_file: TokenFile) {
    assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("opaque-token");
    fs::remove_file(token_file.source.path()).unwrap();
    assert_that!(token_file.source.read().await)
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::Missing));
    token_file.publish(b"invalid token");
    assert_that!(token_file.source.read().await)
        .is_err()
        .is_equal_to(
            token_file
                .source
                .error(ExternalTokenFailure::InvalidContents),
        );
    token_file.publish(b"repaired-token");
    assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("repaired-token");
}

#[rstest]
#[tokio::test]
async fn diagnostics_and_debug_output_do_not_expose_contents(token_file: TokenFile) {
    let token = token_file.source.read().await.unwrap();
    assert_that!(format!("{token:?}")).is_equal_to("ExternalAccessToken([REDACTED])".to_string());
    token_file.publish(b"secret invalid contents");
    let error = token_file.source.read().await.unwrap_err();
    assert_that!(error.to_string()).is_equal_to(format!(
        "External token source from --access-token-file at '{}': invalid contents; supply one raw bearer token with only an optional final LF or CRLF",
        token_file.source.path().display()
    ));
    assert_that!(format!("{error:?}")).is_equal_to(format!(
        "ExternalTokenError {{ origin: Option, path: {:?}, kind: InvalidContents }}",
        token_file.source.path()
    ));
}

#[rstest]
#[tokio::test]
async fn relative_locations_survive_a_working_directory_change(token_file: TokenFile) {
    const CHILD_DIRECTORY: &str = "ROVER_TOKEN_SOURCE_TEST_DIRECTORY";
    if let Some(directory) = std::env::var_os(CHILD_DIRECTORY) {
        let source = ExternalTokenSource::new("access-token", ExternalTokenOrigin::Option);
        let anchored = std::env::current_dir().unwrap().join("access-token");
        std::env::set_current_dir(directory).unwrap();
        assert_that!(source.path()).is_equal_to(anchored.as_path());
        assert_that!(source.read().await.unwrap().secret()).is_equal_to("opaque-token");
        return;
    }
    // Change the working directory only in an isolated process, not in the
    // parallel test runner shared by the other tests.
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "external_token::tests::relative_locations_survive_a_working_directory_change",
            "--test-threads=1",
        ])
        .current_dir(token_file.directory.path())
        .env(
            CHILD_DIRECTORY,
            token_file.directory.path().parent().unwrap(),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
}

#[rstest]
#[tokio::test]
async fn concurrent_reads_obtain_complete_independent_tokens(token_file: TokenFile) {
    let token_file = std::sync::Arc::new(token_file);
    let old = "a".repeat(1024);
    let new = "b".repeat(1024);
    token_file.publish(old.as_bytes());
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(5));
    let mut readers = Vec::new();
    for _ in 0..4 {
        let source = token_file.source.clone();
        let barrier = barrier.clone();
        let old = old.clone();
        let new = new.clone();
        readers.push(tokio::spawn(async move {
            barrier.wait().await;
            for _ in 0..100 {
                let token = source.clone().oneshot(()).await.unwrap();
                assert!(token.secret() == old || token.secret() == new);
            }
        }));
    }
    barrier.wait().await;
    let publisher_file = token_file.clone();
    let publisher = tokio::task::spawn_blocking(move || {
        for index in 0..100 {
            publisher_file.publish(if index % 2 == 0 {
                new.as_bytes()
            } else {
                old.as_bytes()
            });
        }
    });
    futures::future::try_join_all(readers).await.unwrap();
    publisher.await.unwrap();
}

#[rstest]
#[tokio::test]
async fn checks_and_reads_the_same_handle_after_atomic_replacement(token_file: TokenFile) {
    let checked = token_file.source.open_checked().unwrap();
    token_file.publish(b"new-token");
    assert_that!(ExternalAccessToken::read_from(checked).unwrap().secret())
        .is_equal_to("opaque-token");
    assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("new-token");
}

#[rstest]
fn bounds_reads_when_a_file_grows_after_metadata_checks(token_file: TokenFile) {
    let checked = token_file.source.open_checked().unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(token_file.source.path())
        .unwrap()
        .write_all(&vec![b'a'; MAX_TOKEN_FILE_BYTES + 1])
        .unwrap();
    assert_that!(ExternalAccessToken::read_from(checked))
        .is_err()
        .is_equal_to(ExternalTokenFailure::TooLarge);
}

#[cfg(windows)]
mod windows {
    use std::os::windows::fs::{symlink_dir, symlink_file};

    use super::*;

    #[rstest]
    #[case::existing(false)]
    #[case::dangling(true)]
    #[tokio::test]
    async fn refuses_leaf_symlinks(token_file: TokenFile, #[case] dangling: bool) {
        let target = token_file.directory.path().join("target");
        fs::rename(token_file.source.path(), &target).unwrap();
        if dangling {
            fs::remove_file(&target).unwrap();
        }
        symlink_file(target, token_file.source.path()).unwrap();
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
    }

    #[rstest]
    #[tokio::test]
    async fn accepts_symlinks_in_containing_directories(token_file: TokenFile) {
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("alias");
        symlink_dir(token_file.directory.path(), &alias).unwrap();
        let source =
            ExternalTokenSource::new(alias.join("access-token"), ExternalTokenOrigin::Option);
        assert_that!(source.read().await.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::directory(false)]
    #[case::directory_symlink(true)]
    #[tokio::test]
    async fn refuses_directories_and_directory_symlinks(token_file: TokenFile, #[case] link: bool) {
        fs::remove_file(token_file.source.path()).unwrap();
        let expected = if link {
            symlink_dir(token_file.directory.path(), token_file.source.path()).unwrap();
            ExternalTokenFailure::SymbolicLink
        } else {
            fs::create_dir(token_file.source.path()).unwrap();
            ExternalTokenFailure::NotRegularFile
        };
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(token_file.source.error(expected));
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    #[rstest]
    #[tokio::test]
    async fn construction_defers_an_unavailable_working_directory(token_file: TokenFile) {
        const CHILD: &str = "ROVER_TOKEN_SOURCE_TEST_MISSING_CWD";
        if std::env::var_os(CHILD).is_some() {
            let deleted = token_file.directory.path().join("deleted");
            fs::create_dir(&deleted).unwrap();
            std::env::set_current_dir(&deleted).unwrap();
            fs::remove_dir(&deleted).unwrap();
            let source = ExternalTokenSource::new("access-token", ExternalTokenOrigin::Environment);
            let expected = ExternalTokenError {
                origin: ExternalTokenOrigin::Environment,
                path: PathBuf::from("access-token"),
                kind: ExternalTokenFailure::UnresolvableLocation,
            };
            assert_that!(source.read().await).is_equal_to(Err(expected.clone()));
            assert_that!(expected.to_string()).is_equal_to(
                "External token source from APOLLO_ACCESS_TOKEN_FILE at 'access-token': the location cannot be resolved from the initial working directory; supply a valid absolute token-file path or invoke Rover from an accessible directory".to_string(),
            );
            let absolute = ExternalTokenSource::new(
                token_file.source.path(),
                ExternalTokenOrigin::Environment,
            );
            assert_that!(absolute.read().await.unwrap().secret()).is_equal_to("opaque-token");
            let empty = ExternalTokenSource::new("", ExternalTokenOrigin::Environment);
            assert_that!(empty.read().await)
                .is_err()
                .is_equal_to(empty.error(ExternalTokenFailure::EmptyLocation));
            std::env::set_current_dir(token_file.directory.path()).unwrap();
            // Repairing the process's cwd must not silently redirect a source
            // that could not be anchored when the invocation started.
            assert_that!(source.read().await).is_equal_to(Err(expected));
            return;
        }
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "external_token::tests::unix::construction_defers_an_unavailable_working_directory",
                "--test-threads=1",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{output:?}");
    }

    #[rstest]
    #[case::existing(false)]
    #[case::dangling(true)]
    #[tokio::test]
    async fn refuses_leaf_symlinks(token_file: TokenFile, #[case] dangling: bool) {
        let target = token_file.directory.path().join("target");
        fs::rename(token_file.source.path(), &target).unwrap();
        if dangling {
            fs::remove_file(&target).unwrap();
        }
        symlink(target, token_file.source.path()).unwrap();
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
    }

    #[rstest]
    #[tokio::test]
    async fn accepts_symlinks_in_containing_directories(token_file: TokenFile) {
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("alias");
        symlink(token_file.directory.path(), &alias).unwrap();
        let source =
            ExternalTokenSource::new(alias.join("access-token"), ExternalTokenOrigin::Option);
        assert_that!(source.read().await.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[tokio::test]
    async fn refuses_device_files_before_reading_contents() {
        let source = ExternalTokenSource::new("/dev/null", ExternalTokenOrigin::Option);
        assert_that!(source.read().await)
            .is_err()
            .is_equal_to(source.error(ExternalTokenFailure::NotRegularFile));
    }

    #[rstest]
    #[case::directory(false)]
    #[case::fifo(true)]
    #[timeout(std::time::Duration::from_secs(5))]
    #[tokio::test]
    async fn refuses_non_regular_files_without_blocking(token_file: TokenFile, #[case] fifo: bool) {
        fs::remove_file(token_file.source.path()).unwrap();
        if fifo {
            nix::unistd::mkfifo(token_file.source.path(), nix::sys::stat::Mode::S_IRUSR).unwrap();
        } else {
            fs::create_dir(token_file.source.path()).unwrap();
        }
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(
                token_file
                    .source
                    .error(ExternalTokenFailure::NotRegularFile),
            );
    }

    #[rstest]
    #[case::group_read(0o640)]
    #[case::group_write(0o620)]
    #[case::other_read(0o604)]
    #[case::other_write(0o602)]
    #[tokio::test]
    async fn rechecks_file_permissions_on_every_read(token_file: TokenFile, #[case] mode: u32) {
        assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("opaque-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(mode)).unwrap();
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(
                token_file
                    .source
                    .error(ExternalTokenFailure::FilePermissions),
            );
        assert_that!(
            fs::metadata(token_file.source.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        )
        .is_equal_to(mode);
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o600)).unwrap();
        assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::group_write(0o720)]
    #[case::other_write(0o702)]
    #[case::sticky_world_writable(0o1777)]
    #[tokio::test]
    async fn refuses_unsafe_directories_without_changing_them(
        token_file: TokenFile,
        #[case] mode: u32,
    ) {
        fs::set_permissions(
            token_file.directory.path(),
            fs::Permissions::from_mode(mode),
        )
        .unwrap();
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(
                token_file
                    .source
                    .error(ExternalTokenFailure::DirectoryPermissions),
            );
        assert_that!(
            fs::metadata(token_file.directory.path())
                .unwrap()
                .permissions()
                .mode()
                & 0o7777
        )
        .is_equal_to(mode);
    }

    #[rstest]
    #[case::owner_only(0o600, 0o700)]
    #[case::public_directory(0o400, 0o755)]
    #[case::execute_only_directory(0o600, 0o100)]
    #[case::file_executable_by_others(0o611, 0o711)]
    #[tokio::test]
    async fn permits_modes_that_do_not_grant_other_users_read_or_write_access(
        token_file: TokenFile,
        #[case] file_mode: u32,
        #[case] directory_mode: u32,
    ) {
        fs::set_permissions(
            token_file.source.path(),
            fs::Permissions::from_mode(file_mode),
        )
        .unwrap();
        fs::set_permissions(
            token_file.directory.path(),
            fs::Permissions::from_mode(directory_mode),
        )
        .unwrap();
        let result = token_file.source.read().await;
        fs::set_permissions(
            token_file.directory.path(),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert_that!(result.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[tokio::test]
    async fn unreadable_files_are_not_replaced_by_cached_credentials(token_file: TokenFile) {
        if nix::unistd::geteuid().is_root() {
            return; // Root can read a mode-000 file.
        }
        assert_that!(token_file.source.read().await.unwrap().secret()).is_equal_to("opaque-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o000)).unwrap();
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::Unreadable));
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o600)).unwrap();
    }

    #[test]
    fn refuses_foreign_file_and_directory_owners_but_accepts_root_owned_directories() {
        let foreign_owner = if nix::unistd::geteuid().as_raw() == 1 {
            2
        } else {
            1
        };
        let foreign = UnixPermissions {
            owner: foreign_owner,
            mode: 0o700,
        };
        assert_that!(foreign.check_file())
            .is_err()
            .is_equal_to(ExternalTokenFailure::FileOwner);
        assert_that!(foreign.check_directory())
            .is_err()
            .is_equal_to(ExternalTokenFailure::DirectoryOwner);
        let root = UnixPermissions {
            owner: 0,
            mode: 0o755,
        };
        assert_that!(root.check_directory()).is_equal_to(Ok(()));
    }

    #[rstest]
    #[tokio::test]
    async fn rechecks_replacement_file_permissions_instead_of_using_prior_metadata(
        token_file: TokenFile,
    ) {
        let checked = token_file.source.open_checked().unwrap();
        token_file.publish(b"replacement-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert_that!(ExternalAccessToken::read_from(checked).unwrap().secret())
            .is_equal_to("opaque-token");
        assert_that!(token_file.source.read().await)
            .is_err()
            .is_equal_to(
                token_file
                    .source
                    .error(ExternalTokenFailure::FilePermissions),
            );
    }
}
