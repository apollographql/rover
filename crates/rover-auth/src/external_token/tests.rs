#[cfg(unix)]
use std::os::unix::fs::{PermissionsExt, symlink as symlink_dir, symlink as symlink_file};
#[cfg(windows)]
use std::os::windows::fs::{symlink_dir, symlink_file};
use std::{
    fs,
    io::{self, Read, Write},
    path::PathBuf,
};

use rstest::{fixture, rstest};
use speculoos::prelude::*;
use tempfile::{NamedTempFile, TempDir};
use tower::ServiceExt;

use super::{mock::MockTokenSourceService, *};

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

#[cfg(unix)]
impl TokenFile {
    fn set_file_mode(&self, mode: u32) {
        fs::set_permissions(self.source.path(), fs::Permissions::from_mode(mode)).unwrap();
    }

    fn set_directory_mode(&self, mode: u32) {
        fs::set_permissions(self.directory.path(), fs::Permissions::from_mode(mode)).unwrap();
    }

    fn file_mode(&self) -> u32 {
        fs::metadata(self.source.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    }

    fn directory_mode(&self) -> u32 {
        fs::metadata(self.directory.path())
            .unwrap()
            .permissions()
            .mode()
            & 0o7777
    }
}

#[fixture]
fn token_file() -> TokenFile {
    let directory = tempfile::tempdir().unwrap();
    let source = ExternalTokenSource::new(
        directory.path().join("access-token"),
        ExternalTokenOrigin::Option,
    );
    let fixture = TokenFile { directory, source };
    #[cfg(unix)]
    fixture.set_directory_mode(0o700);
    fixture.publish(b"opaque-token");
    fixture
}

#[fixture]
fn absent_token_file(token_file: TokenFile) -> TokenFile {
    fs::remove_file(token_file.source.path()).unwrap();
    token_file
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
    assert_that!(ExternalAccessToken::parse(&bytes).unwrap().secret()).is_equal_to(token);
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
    assert_that!(ExternalAccessToken::parse(bytes))
        .is_err()
        .is_equal_to(ExternalTokenFailure::InvalidContents);
}

#[rstest]
#[case::without_ending("", MAX_TOKEN_FILE_BYTES)]
#[case::with_lf("\n", MAX_TOKEN_FILE_BYTES - 1)]
#[case::with_crlf("\r\n", MAX_TOKEN_FILE_BYTES - 2)]
fn parses_exactly_sixteen_kib_including_the_line_ending(
    #[case] ending: &str,
    #[case] token_length: usize,
) {
    let token = "a".repeat(token_length);
    let bytes = format!("{token}{ending}").into_bytes();
    assert_that!(ExternalAccessToken::parse(&bytes).unwrap().secret()).is_equal_to(token.as_str());
}

#[rstest]
#[case::without_ending("")]
#[case::with_lf("\n")]
#[case::with_crlf("\r\n")]
fn parsing_refuses_oversize_input_including_the_line_ending(#[case] ending: &str) {
    let token = "a".repeat(MAX_TOKEN_FILE_BYTES + 1 - ending.len());
    let bytes = format!("{token}{ending}").into_bytes();
    assert_that!(ExternalAccessToken::parse(&bytes))
        .is_err()
        .is_equal_to(ExternalTokenFailure::TooLarge);
}

#[rstest]
#[case::without_ending("", MAX_TOKEN_FILE_BYTES)]
#[case::with_lf("\n", MAX_TOKEN_FILE_BYTES - 1)]
#[case::with_crlf("\r\n", MAX_TOKEN_FILE_BYTES - 2)]
#[test]
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
#[test]
fn refuses_oversize_files_before_reading_contents(token_file: TokenFile) {
    token_file.publish(&vec![b'a'; MAX_TOKEN_FILE_BYTES + 1]);
    assert_that!(token_file.source.read())
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::TooLarge));
}

#[test]
fn reads_at_most_the_limit_plus_one_byte_even_without_eof() {
    let mut reader = io::repeat(b'a').take(u64::MAX);
    assert_that!(ExternalAccessToken::read_from(&mut reader))
        .is_err()
        .is_equal_to(ExternalTokenFailure::TooLarge);
    assert_that!(u64::MAX - reader.limit()).is_equal_to((MAX_TOKEN_FILE_BYTES + 1) as u64);
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
#[case::option(ExternalTokenOrigin::Option)]
#[case::environment(ExternalTokenOrigin::Environment)]
#[test]
fn empty_sources_fail_on_read_with_their_provenance(#[case] origin: ExternalTokenOrigin) {
    let source = ExternalTokenSource::new("", origin);
    assert_that!(source.origin()).is_equal_to(origin);
    assert_that!(source.path()).is_equal_to(Path::new(""));
    let result = source.read();
    let error = assert_that!(result).is_err().subject;
    assert_that!(error).is_equal_to(&ExternalTokenError {
        origin,
        path: PathBuf::new(),
        kind: ExternalTokenFailure::EmptyLocation,
    });
    assert_that!(error.to_string()).is_equal_to(
        "External token source: the location is empty; supply the path to the application's token file".to_string()
    );
}

#[rstest]
#[tokio::test]
async fn construction_does_not_open_or_require_the_file(
    #[from(absent_token_file)] token_file: TokenFile,
) {
    let mut source =
        ExternalTokenSource::new(token_file.source.path(), ExternalTokenOrigin::Environment);
    assert_that!(
        source
            .ready()
            .await
            .map(|source| source.path().to_path_buf())
    )
    .is_ok()
    .is_equal_to(token_file.source.path().to_path_buf());
    assert_that!(source.read())
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
    assert_that!(result)
        .is_ok()
        .is_equal_to("opaque-token".to_string());
}

#[tokio::test]
async fn consumers_can_use_in_memory_sources_as_services() {
    let token = ExternalAccessToken::parse(b"opaque-token").unwrap();
    let mut source = MockTokenSourceService::new();
    rover_tower::expect_poll_ready!(source);
    source
        .expect_call()
        .times(1)
        .return_once(move |()| futures::future::ready(Ok(token)));
    let result = source
        .boxed()
        .map_response(|token| token.secret().to_owned())
        .oneshot(())
        .await;
    assert_that!(result)
        .is_ok()
        .is_equal_to("opaque-token".to_string());
}

#[rstest]
#[test]
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
#[test]
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

#[test]
fn token_debug_output_does_not_expose_contents() {
    let token = ExternalAccessToken::parse(b"opaque-token").unwrap();
    assert_that!(format!("{token:?}")).is_equal_to("ExternalAccessToken([REDACTED])".to_string());
}

#[rstest]
#[test]
fn source_diagnostics_do_not_expose_contents(token_file: TokenFile) {
    token_file.publish(b"secret invalid contents");
    let result = token_file.source.read();
    let error = assert_that!(result).is_err().subject;
    assert_that!(error.to_string()).is_equal_to(format!(
        "External token source at '{}': invalid contents; supply one raw bearer token with only an optional final LF or CRLF",
        token_file.source.path().display()
    ));
    assert_that!(format!("{error:?}")).is_equal_to(format!(
        "ExternalTokenError {{ origin: Option, path: {:?}, kind: InvalidContents }}",
        token_file.source.path()
    ));
}

#[rstest]
fn concurrent_reads_obtain_complete_independent_tokens(token_file: TokenFile) {
    let old = "a".repeat(1024);
    let new = "b".repeat(1024);
    let expected_tokens = [Ok(old.clone()), Ok(new.clone())];
    token_file.publish(old.as_bytes());
    let replacements = [new.as_bytes(), old.as_bytes()]
        .into_iter()
        .cycle()
        .take(100);
    // Readers run on their own threads, all started before the first
    // replacement, so reads overlap the replacements.
    let reader_results = std::thread::scope(|scope| {
        let mut readers = Vec::new();
        for _ in 0..4 {
            let source = token_file.source.clone();
            readers.push(scope.spawn(move || {
                (0..100)
                    .map(|_| source.read().map(|token| token.secret().to_owned()))
                    .collect::<Vec<_>>()
            }));
        }
        for replacement in replacements {
            token_file.publish(replacement);
        }
        readers
            .into_iter()
            .flat_map(|reader| reader.join().unwrap())
            .collect::<Vec<_>>()
    });
    assert_that!(reader_results.len()).is_equal_to(400);
    let unexpected_reads = reader_results
        .into_iter()
        .enumerate()
        .filter(|(_, result)| !expected_tokens.contains(result))
        .collect::<Vec<_>>();
    assert_that!(unexpected_reads).is_equal_to(Vec::new());
}

#[rstest]
#[test]
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

#[rstest]
#[test]
fn refuses_leaf_symlinks(token_file: TokenFile) {
    let target = token_file.directory.path().join("target");
    fs::rename(token_file.source.path(), &target).unwrap();
    symlink_file(target, token_file.source.path()).unwrap();
    assert_that!(token_file.source.read())
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
}

#[rstest]
#[test]
fn refuses_dangling_leaf_symlinks(#[from(absent_token_file)] token_file: TokenFile) {
    symlink_file(
        token_file.directory.path().join("missing"),
        token_file.source.path(),
    )
    .unwrap();
    assert_that!(token_file.source.read())
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
}

#[rstest]
#[test]
fn accepts_symlinks_in_containing_directories(token_file: TokenFile) {
    let aliases = tempfile::tempdir().unwrap();
    let alias = aliases.path().join("alias");
    symlink_dir(token_file.directory.path(), &alias).unwrap();
    let source = ExternalTokenSource::new(alias.join("access-token"), ExternalTokenOrigin::Option);
    assert_that!(source.read().unwrap().secret()).is_equal_to("opaque-token");
}

#[rstest]
#[test]
fn refuses_directories(#[from(absent_token_file)] token_file: TokenFile) {
    fs::create_dir(token_file.source.path()).unwrap();
    assert_that!(token_file.source.read()).is_err().is_equal_to(
        token_file
            .source
            .error(ExternalTokenFailure::NotRegularFile),
    );
}

#[cfg(windows)]
#[rstest]
#[test]
fn refuses_directory_symlinks(#[from(absent_token_file)] token_file: TokenFile) {
    symlink_dir(token_file.directory.path(), token_file.source.path()).unwrap();
    assert_that!(token_file.source.read())
        .is_err()
        .is_equal_to(token_file.source.error(ExternalTokenFailure::SymbolicLink));
}

#[cfg(unix)]
mod unix {
    use super::*;

    #[test]
    fn refuses_device_files_before_reading_contents() {
        let source = ExternalTokenSource::new("/dev/null", ExternalTokenOrigin::Option);
        assert_that!(source.read())
            .is_err()
            .is_equal_to(source.error(ExternalTokenFailure::NotRegularFile));
    }

    #[rstest]
    #[timeout(std::time::Duration::from_secs(5))]
    #[test]
    fn refuses_fifos_without_blocking(#[from(absent_token_file)] token_file: TokenFile) {
        nix::unistd::mkfifo(token_file.source.path(), nix::sys::stat::Mode::S_IRUSR).unwrap();
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
    #[test]
    fn rechecks_file_permissions_on_every_read(token_file: TokenFile, #[case] mode: u32) {
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
        token_file.set_file_mode(mode);
        assert_that!(token_file.source.read()).is_err().is_equal_to(
            token_file
                .source
                .error(ExternalTokenFailure::FilePermissions),
        );
        assert_that!(token_file.file_mode()).is_equal_to(mode);
        token_file.set_file_mode(0o600);
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[case::group_write(0o720)]
    #[case::other_write(0o702)]
    #[case::sticky_world_writable(0o1777)]
    #[test]
    fn refuses_unsafe_directories_without_changing_them(token_file: TokenFile, #[case] mode: u32) {
        token_file.set_directory_mode(mode);
        assert_that!(token_file.source.read()).is_err().is_equal_to(
            token_file
                .source
                .error(ExternalTokenFailure::DirectoryPermissions),
        );
        assert_that!(token_file.directory_mode()).is_equal_to(mode);
    }

    #[rstest]
    #[case::owner_only(0o600, 0o700)]
    #[case::public_directory(0o400, 0o755)]
    #[case::execute_only_directory(0o600, 0o100)]
    #[case::file_executable_by_others(0o611, 0o711)]
    #[test]
    fn permits_modes_that_do_not_grant_other_users_read_or_write_access(
        token_file: TokenFile,
        #[case] file_mode: u32,
        #[case] directory_mode: u32,
    ) {
        token_file.set_file_mode(file_mode);
        token_file.set_directory_mode(directory_mode);
        let result = token_file.source.read();
        token_file.set_directory_mode(0o700);
        assert_that!(result.unwrap().secret()).is_equal_to("opaque-token");
    }

    #[rstest]
    #[test]
    fn unreadable_files_are_not_replaced_by_cached_credentials(token_file: TokenFile) {
        if nix::unistd::geteuid().is_root() {
            return; // Root can read a mode-000 file.
        }
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
        token_file.set_file_mode(0o000);
        assert_that!(token_file.source.read())
            .is_err()
            .is_equal_to(token_file.source.error(ExternalTokenFailure::Unreadable));
        token_file.set_file_mode(0o600);
        assert_that!(token_file.source.read().unwrap().secret()).is_equal_to("opaque-token");
    }

    #[fixture]
    fn foreign_permissions() -> UnixPermissions {
        UnixPermissions {
            owner: nix::unistd::geteuid().as_raw().wrapping_add(1),
            mode: 0o700,
        }
    }

    #[rstest]
    fn refuses_foreign_file_owners(foreign_permissions: UnixPermissions) {
        assert_that!(foreign_permissions.check_file())
            .is_err()
            .is_equal_to(ExternalTokenFailure::FileOwner);
    }

    #[rstest]
    fn refuses_foreign_directory_owners(foreign_permissions: UnixPermissions) {
        assert_that!(foreign_permissions.check_directory())
            .is_err()
            .is_equal_to(ExternalTokenFailure::DirectoryOwner);
    }

    #[test]
    fn accepts_root_owned_directories() {
        let root = UnixPermissions {
            owner: 0,
            mode: 0o755,
        };
        assert_that!(root.check_directory()).is_ok().is_equal_to(());
    }

    #[rstest]
    #[test]
    fn rechecks_replacement_file_permissions_instead_of_using_prior_metadata(
        token_file: TokenFile,
    ) {
        let checked = token_file.source.open_checked().unwrap();
        token_file.publish(b"replacement-token");
        token_file.set_file_mode(0o644);
        assert_that!(ExternalAccessToken::read_from(checked).unwrap().secret())
            .is_equal_to("opaque-token");
        assert_that!(token_file.source.read()).is_err().is_equal_to(
            token_file
                .source
                .error(ExternalTokenFailure::FilePermissions),
        );
    }
}
