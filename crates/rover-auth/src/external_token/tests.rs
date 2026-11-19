use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use rstest::{fixture, rstest};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};

use super::*;

struct TokenFile {
    directory: TempDir,
    source: ExternalTokenSource,
}

impl TokenFile {
    fn publish(&self, contents: &[u8]) {
        let mut replacement = NamedTempFile::new_in(self.directory.path()).unwrap();
        replacement.write_all(contents).unwrap();
        replacement.persist(self.source.path()).unwrap();
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
    )
    .unwrap();
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
fn accepts_raw_bearer_tokens_without_inspecting_claims(#[case] token: &str) {
    for ending in ["", "\n", "\r\n"] {
        let bytes = format!("{token}{ending}").into_bytes();
        assert_that!(ExternalAccessToken::try_from(bytes).unwrap().secret()).is_equal_to(token);
    }
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
fn accepts_exactly_sixteen_kib_including_the_line_ending(
    token_file: TokenFile,
    #[case] ending: &str,
    #[case] token_length: usize,
) {
    let token = "a".repeat(token_length);
    token_file.publish(format!("{token}{ending}").as_bytes());
    assert_that!(token_file.source.read().unwrap().secret()).is_equal_to(token.as_str());
}

#[rstest]
fn refuses_oversize_files_before_reading_contents(token_file: TokenFile) {
    token_file.publish(&vec![b'a'; MAX_TOKEN_FILE_BYTES + 1]);
    assert_that!(token_file.source.read())
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
fn empty_sources_fail_on_read_with_their_provenance(
    #[case] origin: ExternalTokenOrigin,
    #[case] name: &str,
) {
    let source = ExternalTokenSource::new("", origin).unwrap();
    assert_that!(source.origin()).is_equal_to(origin);
    assert_that!(source.path()).is_equal_to(Path::new(""));
    let error = source.read().unwrap_err();
    assert_that!(&error).is_equal_to(&ExternalTokenError {
        origin,
        path: PathBuf::new(),
        kind: ExternalTokenFailure::EmptyLocation,
    });
    assert_that!(error.to_string()).is_equal_to(format!(
        "External token source from {name} at '': the location is empty; supply the path to the application's token file"
    ));
}

#[rstest]
fn construction_does_not_open_or_require_the_file(token_file: TokenFile) {
    fs::remove_file(token_file.source.path()).unwrap();
    let source =
        ExternalTokenSource::new(token_file.source.path(), ExternalTokenOrigin::Environment)
            .unwrap();
    assert_that!(source.read())
        .is_err()
        .is_equal_to(source.error(ExternalTokenFailure::Missing));
}

#[rstest]
fn rereads_replacements_without_mutating_previous_tokens(token_file: TokenFile) {
    let first = token_file.source.read().unwrap();
    token_file.publish(b"replacement-token\r\n");
    let second = token_file.source.read().unwrap();
    assert_that!(first.secret()).is_equal_to("opaque-token");
    assert_that!(second.secret()).is_equal_to("replacement-token");
    assert_that!(fs::read(token_file.source.path()).unwrap())
        .is_equal_to(b"replacement-token\r\n".to_vec());
    assert_that!(fs::read_dir(token_file.directory.path()).unwrap().count()).is_equal_to(1);
}

#[rstest]
fn failures_never_reuse_a_token_and_a_repaired_source_can_be_read(token_file: TokenFile) {
    assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
    fs::remove_file(token_file.source.path()).unwrap();
    assert_that!(token_file.source.read())
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::Missing));
    token_file.publish(b"invalid token");
    assert_that!(token_file.source.read()).is_err().is_equal_to(
        token_file
            .source
            .error(ExternalTokenFailure::InvalidContents),
    );
    token_file.publish(b"repaired-token");
    assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("repaired-token");
}

#[rstest]
fn diagnostics_and_debug_output_do_not_expose_contents(token_file: TokenFile) {
    let token = token_file.source.read().unwrap();
    assert_that!(format!("{token:?}")).is_equal_to("ExternalAccessToken([REDACTED])".to_string());
    token_file.publish(b"secret invalid contents");
    let error = token_file.source.read().unwrap_err();
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
fn relative_locations_survive_a_working_directory_change(token_file: TokenFile) {
    const CHILD_DIRECTORY: &str = "ROVER_TOKEN_SOURCE_TEST_DIRECTORY";
    if let Some(directory) = std::env::var_os(CHILD_DIRECTORY) {
        let source = ExternalTokenSource::new("access-token", ExternalTokenOrigin::Option).unwrap();
        let anchored = std::env::current_dir().unwrap().join("access-token");
        std::env::set_current_dir(directory).unwrap();
        assert_that!(source.path()).is_equal_to(anchored.as_path());
        assert_that!(source.read().unwrap().secret()).is_equal_to("opaque-token");
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
fn concurrent_reads_obtain_complete_independent_tokens(token_file: TokenFile) {
    let old = "a".repeat(1024);
    let new = "b".repeat(1024);
    token_file.publish(old.as_bytes());
    let barrier = std::sync::Barrier::new(5);
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let source = token_file.source.clone();
            let barrier = &barrier;
            let old = &old;
            let new = &new;
            scope.spawn(move || {
                barrier.wait();
                for _ in 0..100 {
                    let token = source.read().unwrap();
                    assert!(token.secret() == old || token.secret() == new);
                }
            });
        }
        barrier.wait();
        for index in 0..100 {
            token_file.publish(if index % 2 == 0 {
                new.as_bytes()
            } else {
                old.as_bytes()
            });
        }
    });
}

#[rstest]
fn checks_and_reads_the_same_handle_after_atomic_replacement(token_file: TokenFile) {
    let checked = token_file.source.open_checked().unwrap();
    token_file.publish(b"new-token");
    assert_that!(ExternalAccessToken::read_from(checked).unwrap().secret())
        .is_equal_to("opaque-token");
    assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("new-token");
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
    fn refuses_leaf_symlinks(token_file: TokenFile, #[case] dangling: bool) {
        let target = token_file.directory.path().join("target");
        fs::rename(token_file.source.path(), &target).unwrap();
        if dangling {
            fs::remove_file(&target).unwrap();
        }
        symlink_file(target, token_file.source.path()).unwrap();
        assert_that!(token_file.source.read())
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
    }

    #[rstest]
    fn accepts_symlinks_in_containing_directories(token_file: TokenFile) {
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("alias");
        symlink_dir(token_file.directory.path(), &alias).unwrap();
        let source =
            ExternalTokenSource::new(alias.join("access-token"), ExternalTokenOrigin::Option)
                .unwrap();
        assert_that!(source.read().unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::directory(false)]
    #[case::directory_symlink(true)]
    fn refuses_directories_and_directory_symlinks(token_file: TokenFile, #[case] link: bool) {
        fs::remove_file(token_file.source.path()).unwrap();
        let expected = if link {
            symlink_dir(token_file.directory.path(), token_file.source.path()).unwrap();
            ExternalTokenFailure::SymbolicLink
        } else {
            fs::create_dir(token_file.source.path()).unwrap();
            ExternalTokenFailure::NotRegularFile
        };
        assert_that!(token_file.source.read())
            .is_err()
            .is_equal_to(token_file.source.error(expected));
    }
}

#[cfg(unix)]
mod unix {
    use std::os::unix::fs::{PermissionsExt, symlink};

    use super::*;

    #[rstest]
    #[case::existing(false)]
    #[case::dangling(true)]
    fn refuses_leaf_symlinks(token_file: TokenFile, #[case] dangling: bool) {
        let target = token_file.directory.path().join("target");
        fs::rename(token_file.source.path(), &target).unwrap();
        if dangling {
            fs::remove_file(&target).unwrap();
        }
        symlink(target, token_file.source.path()).unwrap();
        assert_that!(token_file.source.read())
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
    }

    #[rstest]
    fn accepts_symlinks_in_containing_directories(token_file: TokenFile) {
        let aliases = tempfile::tempdir().unwrap();
        let alias = aliases.path().join("alias");
        symlink(token_file.directory.path(), &alias).unwrap();
        let source =
            ExternalTokenSource::new(alias.join("access-token"), ExternalTokenOrigin::Option)
                .unwrap();
        assert_that!(source.read().unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::directory(false)]
    #[case::fifo(true)]
    #[timeout(std::time::Duration::from_secs(5))]
    fn refuses_non_regular_files_without_blocking(token_file: TokenFile, #[case] fifo: bool) {
        fs::remove_file(token_file.source.path()).unwrap();
        if fifo {
            nix::unistd::mkfifo(token_file.source.path(), nix::sys::stat::Mode::S_IRUSR).unwrap();
        } else {
            fs::create_dir(token_file.source.path()).unwrap();
        }
        assert_that!(token_file.source.read()).is_err().is_equal_to(
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
    fn rechecks_file_permissions_on_every_read(token_file: TokenFile, #[case] mode: u32) {
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(mode)).unwrap();
        assert_that!(token_file.source.read()).is_err().is_equal_to(
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
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::group_write(0o720)]
    #[case::other_write(0o702)]
    #[case::sticky_world_writable(0o1777)]
    fn refuses_unsafe_directories_without_changing_them(token_file: TokenFile, #[case] mode: u32) {
        fs::set_permissions(
            token_file.directory.path(),
            fs::Permissions::from_mode(mode),
        )
        .unwrap();
        assert_that!(token_file.source.read()).is_err().is_equal_to(
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
    fn permits_modes_that_do_not_grant_other_users_read_or_write_access(
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
        let result = token_file.source.read();
        fs::set_permissions(
            token_file.directory.path(),
            fs::Permissions::from_mode(0o700),
        )
        .unwrap();
        assert_that!(result.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    fn unreadable_files_are_not_replaced_by_cached_credentials(token_file: TokenFile) {
        if nix::unistd::geteuid().is_root() {
            return; // Root can read a mode-000 file.
        }
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o000)).unwrap();
        assert_that!(token_file.source.read())
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
    fn rechecks_replacement_file_permissions_instead_of_using_prior_metadata(
        token_file: TokenFile,
    ) {
        let checked = token_file.source.open_checked().unwrap();
        token_file.publish(b"replacement-token");
        fs::set_permissions(token_file.source.path(), fs::Permissions::from_mode(0o644)).unwrap();
        assert_that!(ExternalAccessToken::read_from(checked).unwrap().secret())
            .is_equal_to("opaque-token");
        assert_that!(token_file.source.read()).is_err().is_equal_to(
            token_file
                .source
                .error(ExternalTokenFailure::FilePermissions),
        );
    }
}
