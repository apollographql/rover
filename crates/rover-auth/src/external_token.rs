//! Application-owned bearer tokens. This module never persists or refreshes a credential.
//!
//! Construct the source once, then call
//! [`ExternalTokenSource::read`](crate::external_token::ExternalTokenSource::read)
//! before each Platform API attempt. Selecting credentials and attaching them to
//! requests belong to callers; merely constructing a source does not open its file.

use std::{
    fmt,
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

use rover_tower::ResponseFuture;
use tower::Service;

/// Maximum token-file size, including its optional line ending.
pub const MAX_TOKEN_FILE_BYTES: usize = 16 * 1024;

/// The runtime input that supplied a token-file location.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalTokenOrigin {
    /// A command-line option.
    Option,
    /// An environment variable.
    Environment,
}

/// A token read from an external source, with redacted debug output.
///
/// Deliberately does not implement `Display` or serialization. Accessing the
/// credential requires an explicit call to [`Self::secret`].
#[derive(Clone, PartialEq, Eq)]
pub struct ExternalAccessToken(String);

impl ExternalAccessToken {
    /// Parses raw token-file bytes with an optional final LF or CRLF.
    ///
    /// The size limit includes the line ending; other whitespace is rejected.
    pub fn parse(bytes: &[u8]) -> Result<Self, ExternalTokenFailure> {
        if bytes.len() > MAX_TOKEN_FILE_BYTES {
            return Err(ExternalTokenFailure::TooLarge);
        }
        let bytes = bytes
            .strip_suffix(b"\r\n")
            .or_else(|| bytes.strip_suffix(b"\n"))
            .unwrap_or(bytes);
        let token =
            std::str::from_utf8(bytes).map_err(|_| ExternalTokenFailure::InvalidContents)?;
        let unpadded = token.trim_end_matches('=');
        if unpadded.is_empty()
            || !unpadded.bytes().all(|byte| {
                byte.is_ascii_alphanumeric()
                    || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/')
            })
        {
            return Err(ExternalTokenFailure::InvalidContents);
        }
        Ok(Self(token.to_owned()))
    }

    /// Exposes the credential for authentication or deliberate identity unmasking.
    pub fn secret(&self) -> &str {
        &self.0
    }

    fn read_from(reader: impl Read) -> Result<Self, ExternalTokenFailure> {
        let mut bytes = Vec::new();
        reader
            .take(MAX_TOKEN_FILE_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(ExternalTokenFailure::from)?;
        Self::parse(&bytes)
    }
}

impl fmt::Debug for ExternalAccessToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExternalAccessToken([REDACTED])")
    }
}

/// A source failure category, kept separate from location and input provenance.
///
/// Callers can map these variants to distinct machine-readable error codes
/// without inspecting diagnostic text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ExternalTokenFailure {
    /// The input was present but empty.
    #[error("the location is empty; supply the path to the application's token file")]
    EmptyLocation,
    /// The file or a containing directory does not exist.
    #[error("the source is missing; have the supplying application publish its token file")]
    Missing,
    /// Opening, checking, or reading the source failed.
    #[error("the source is unreadable; check that Rover can access the file and its directory")]
    Unreadable,
    /// The location could not be anchored to the invocation's working directory.
    #[error(
        "the location cannot be resolved from the initial working directory; supply a valid absolute token-file path or invoke Rover from an accessible directory"
    )]
    UnresolvableLocation,
    /// The file exceeds the size limit.
    #[error("the source exceeds 16 KiB; supply a token file no larger than 16,384 bytes")]
    TooLarge,
    /// The content is not a single RFC 6750 bearer token.
    #[error("invalid contents; supply one raw bearer token with only an optional final LF or CRLF")]
    InvalidContents,
    /// The final path component is a symbolic link.
    #[error("the source is a symbolic link; supply a regular file, not a link")]
    SymbolicLink,
    /// The source is a directory, pipe, device, or another non-regular file.
    #[error("the source is not a regular file; supply a regular token file")]
    NotRegularFile,
    /// The token file belongs to another Unix user.
    #[error("the file has the wrong owner; make the user running Rover its owner")]
    FileOwner,
    /// Other Unix users can read or write the file.
    #[error(
        "the file allows other users to read or write it; remove group/other read and write permissions (chmod 600)"
    )]
    FilePermissions,
    /// The containing directory belongs to neither the user nor root.
    #[error(
        "the containing directory has the wrong owner; use a directory owned by the user running Rover or root"
    )]
    DirectoryOwner,
    /// Other Unix users can write the containing directory.
    #[error(
        "the containing directory is group- or world-writable; remove group/other write permissions (chmod go-w), even if the sticky bit is set"
    )]
    DirectoryPermissions,
}

impl From<io::Error> for ExternalTokenFailure {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::NotFound => Self::Missing,
            _ => Self::Unreadable,
        }
    }
}

/// A source diagnostic that never includes file contents.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub struct ExternalTokenError {
    /// Which option or environment variable supplied the location.
    pub origin: ExternalTokenOrigin,
    /// Resolved location, or the original input if empty or unresolvable.
    pub path: PathBuf,
    /// What failed, independently of the human-readable message.
    pub kind: ExternalTokenFailure,
}

impl fmt::Display for ExternalTokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("External token source")?;
        if !self.path.as_os_str().is_empty() {
            write!(f, " at '{}'", self.path.display())?;
        }
        write!(f, ": {}", self.kind)
    }
}

/// A read-only, non-caching source whose relative location is anchored at construction.
///
/// Implements [`Service<()>`] so consumers can inject any service returning an
/// [`ExternalAccessToken`]. Each call reads anew; becoming ready does not access
/// the file. Callers may clone the source for independent concurrent reads.
#[derive(Clone, Debug)]
pub struct ExternalTokenSource {
    path: PathBuf,
    origin: ExternalTokenOrigin,
    initial_failure: Option<ExternalTokenFailure>,
}

impl ExternalTokenSource {
    /// Anchors a relative location to the current working directory without opening it.
    ///
    /// Construct this at invocation startup so later working-directory changes do
    /// not change its meaning. Empty locations and resolution failures are
    /// retained and fail only on read, so commands that do not authenticate
    /// cannot fail merely because a token source was configured.
    pub fn new(location: impl AsRef<Path>, origin: ExternalTokenOrigin) -> Self {
        let location = location.as_ref();
        let (path, initial_failure) = if location.as_os_str().is_empty() {
            (PathBuf::new(), Some(ExternalTokenFailure::EmptyLocation))
        } else {
            match std::path::absolute(location) {
                Ok(path) => (path, None),
                Err(_) => (
                    location.to_path_buf(),
                    Some(ExternalTokenFailure::UnresolvableLocation),
                ),
            }
        };
        Self {
            path,
            origin,
            initial_failure,
        }
    }

    /// The input that supplied this location.
    pub const fn origin(&self) -> ExternalTokenOrigin {
        self.origin
    }

    /// The anchored location, or the original input if empty or unresolvable.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Opens, checks, and reads the source anew, without changing it or caching a token.
    ///
    /// Returns an unreadable-source error if no Tokio runtime is available.
    /// The complete open/check/read sequence runs on one blocking worker, never
    /// on an async runtime worker. Dropping the future does not stop a blocking
    /// operation that has already started.
    pub async fn read(&self) -> Result<ExternalAccessToken, ExternalTokenError> {
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|_| self.error(ExternalTokenFailure::Unreadable))?;
        let source = self.clone();
        runtime
            .spawn_blocking(move || {
                let file = source.open_checked().map_err(|kind| source.error(kind))?;
                ExternalAccessToken::read_from(file).map_err(|kind| source.error(kind))
            })
            .await
            .map_err(|_| self.error(ExternalTokenFailure::Unreadable))?
    }

    fn error(&self, kind: ExternalTokenFailure) -> ExternalTokenError {
        ExternalTokenError {
            origin: self.origin,
            path: self.path.clone(),
            kind,
        }
    }

    fn open_checked(&self) -> Result<File, ExternalTokenFailure> {
        if let Some(failure) = self.initial_failure {
            return Err(failure);
        }
        self.check_file(self.open_file()?)
    }

    #[cfg(unix)]
    fn open_file(&self) -> Result<File, ExternalTokenFailure> {
        use nix::{
            errno::Errno,
            fcntl::{OFlag, open, openat},
            sys::stat::Mode,
        };

        let parent = self
            .path
            .parent()
            .ok_or(ExternalTokenFailure::NotRegularFile)?;
        let name = self
            .path
            .file_name()
            .ok_or(ExternalTokenFailure::NotRegularFile)?;

        // Pin the containing directory so a path replacement cannot make us
        // check one directory and open the token through a different one.
        #[cfg(any(target_os = "linux", target_os = "android"))]
        let access = OFlag::O_PATH;
        #[cfg(target_vendor = "apple")]
        let access = OFlag::O_SEARCH;
        // Linux and macOS are Rover's supported Unix targets. The fallback
        // for other Unix platforms additionally needs directory read access.
        #[cfg(not(any(target_os = "linux", target_os = "android", target_vendor = "apple")))]
        let access = OFlag::O_RDONLY;
        let directory = File::from(
            open(
                parent,
                access | OFlag::O_DIRECTORY | OFlag::O_CLOEXEC,
                Mode::empty(),
            )
            .map_err(io::Error::from)?,
        );
        UnixPermissions::from(&directory.metadata()?).check_directory()?;

        // O_NOFOLLOW checks the leaf as part of the open. O_NONBLOCK prevents
        // an application-supplied FIFO from hanging before its type is checked.
        // O_NOCTTY prevents a device from becoming the controlling terminal.
        let file = File::from(
            openat(
                &directory,
                Path::new(name),
                OFlag::O_RDONLY
                    | OFlag::O_CLOEXEC
                    | OFlag::O_NOFOLLOW
                    | OFlag::O_NONBLOCK
                    | OFlag::O_NOCTTY,
                Mode::empty(),
            )
            .map_err(|error| match error {
                // Linux and macOS report a refused leaf symlink as ELOOP.
                Errno::ELOOP => ExternalTokenFailure::SymbolicLink,
                _ => io::Error::from(error).into(),
            })?,
        );
        Ok(file)
    }

    #[cfg(windows)]
    fn open_file(&self) -> Result<File, ExternalTokenFailure> {
        use std::{fs::OpenOptions, os::windows::fs::OpenOptionsExt};

        // Open the reparse point itself rather than following a leaf symlink.
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        // Also open directories so their type (or a directory symlink) can be
        // diagnosed from the same handle, rather than failing with access denied.
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(&self.path)
            .map_err(Into::into)
    }

    fn check_file(&self, file: File) -> Result<File, ExternalTokenFailure> {
        let metadata = file.metadata()?;
        // Unix has already refused leaf symlinks atomically with O_NOFOLLOW.
        #[cfg(windows)]
        if metadata.file_type().is_symlink() {
            return Err(ExternalTokenFailure::SymbolicLink);
        }
        if !metadata.is_file() {
            return Err(ExternalTokenFailure::NotRegularFile);
        }
        #[cfg(unix)]
        UnixPermissions::from(&metadata).check_file()?;
        if metadata.len() > MAX_TOKEN_FILE_BYTES as u64 {
            return Err(ExternalTokenFailure::TooLarge);
        }
        // All checks and the subsequent read use this same open handle.
        Ok(file)
    }
}

impl Service<()> for ExternalTokenSource {
    type Response = ExternalAccessToken;
    type Error = ExternalTokenError;
    type Future = ResponseFuture<Result<Self::Response, Self::Error>>;

    rover_tower::default_poll_ready!();

    fn call(&mut self, (): ()) -> Self::Future {
        let source = self.clone();
        Box::pin(async move { source.read().await })
    }
}

#[cfg(unix)]
struct UnixPermissions {
    owner: u32,
    mode: u32,
}

#[cfg(unix)]
impl From<&std::fs::Metadata> for UnixPermissions {
    fn from(metadata: &std::fs::Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;
        Self {
            owner: metadata.uid(),
            mode: metadata.mode(),
        }
    }
}

#[cfg(unix)]
impl UnixPermissions {
    fn check_file(&self) -> Result<(), ExternalTokenFailure> {
        if self.owner != nix::unistd::geteuid().as_raw() {
            return Err(ExternalTokenFailure::FileOwner);
        }
        if self.mode & 0o066 != 0 {
            return Err(ExternalTokenFailure::FilePermissions);
        }
        Ok(())
    }

    fn check_directory(&self) -> Result<(), ExternalTokenFailure> {
        if self.owner != 0 && self.owner != nix::unistd::geteuid().as_raw() {
            return Err(ExternalTokenFailure::DirectoryOwner);
        }
        if self.mode & 0o022 != 0 {
            return Err(ExternalTokenFailure::DirectoryPermissions);
        }
        Ok(())
    }
}

/// Mock token sources for consumers and tests.
#[cfg(any(test, feature = "testing"))]
pub mod mock {
    use super::{ExternalAccessToken, ExternalTokenError};

    /// Request accepted by an external token source.
    pub type TokenSourceRequest = ();

    rover_tower::mock_service!(
        TokenSource,
        TokenSourceRequest,
        ExternalAccessToken,
        ExternalTokenError
    );
}

#[cfg(test)]
mod tests;
