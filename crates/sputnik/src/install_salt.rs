use std::{fmt, io::Write};

use camino::Utf8Path;
use hmac::{Hmac, KeyInit, Mac};
use rover_std::Fs;
use rover_studio::types::GraphRef;
use serde::Serialize;
use sha2::Sha256;

use crate::SputnikError;

const SALT_BYTES: usize = 32;

/// A one-way identifier for a graph, as lowercase hexadecimal.
///
/// It can only be made by [`InstallSalt::graph_identifier`], so a graph ref, or anything else
/// that hasn't been through that keyed derivation, can't be put in an event as one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct GraphId(String);

#[cfg(test)]
impl GraphId {
    /// A graph identifier with a chosen value, for tests of what carries one.
    pub(crate) fn for_tests(value: &str) -> Self {
        Self(value.to_string())
    }
}

/// A random value generated once for an install, and never sent anywhere.
///
/// It keys the graph identifier, so that nobody can reproduce that identifier by hashing
/// known graph refs: without this value, which stays on the machine it was generated on,
/// the identifier can't be matched to a graph.
#[derive(Clone, PartialEq, Eq)]
pub struct InstallSalt([u8; SALT_BYTES]);

impl InstallSalt {
    /// Reads the salt stored at `path`, and generates and stores a new one if there isn't a
    /// usable one there. Deleting the file therefore resets the identifiers derived from it.
    ///
    /// Processes that start together on a fresh install all end up with the same salt, and
    /// none of them can read a half-written one: a new salt is written to a temporary file,
    /// readable only by its owner on Unix, and moved into place only if nothing is there yet.
    /// A process that loses that race uses the salt that won.
    pub(crate) fn read_or_create(path: &Utf8Path) -> Result<Self, SputnikError> {
        if let Some(salt) = Self::read(path) {
            return Ok(salt);
        }
        let salt = Self(rand::random());
        let dir = path
            .parent()
            .filter(|dir| !dir.as_str().is_empty())
            .unwrap_or_else(|| Utf8Path::new("."));
        std::fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(salt.to_hex().as_bytes())?;
        match file.persist_noclobber(path) {
            Ok(_) => Ok(salt),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                match Self::read(path) {
                    Some(winner) => Ok(winner),
                    // What's there isn't a salt, so replace it.
                    None => {
                        error.file.persist(path).map_err(|error| error.error)?;
                        Ok(salt)
                    }
                }
            }
            Err(error) => Err(error.error.into()),
        }
    }

    fn read(path: &Utf8Path) -> Option<Self> {
        Self::from_hex(Fs::read_file(path).ok()?.trim())
    }

    /// A one-way identifier for a graph ref.
    ///
    /// It is derived from the ref's canonical `name@variant` form, so a ref written without its
    /// variant gets the same identifier as one that names the default. The same salt and graph
    /// ref always give the same identifier, and a different salt gives a different one.
    pub fn graph_identifier(&self, graph_ref: &GraphRef) -> GraphId {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(&self.0).expect("HMAC accepts a key of any length");
        mac.update(graph_ref.to_string().as_bytes());
        GraphId(encode_hex(&mac.finalize().into_bytes()))
    }

    pub(crate) fn to_hex(&self) -> String {
        encode_hex(&self.0)
    }

    fn from_hex(hex: &str) -> Option<Self> {
        if hex.len() != SALT_BYTES * 2 || !hex.is_ascii() {
            return None;
        }
        let mut bytes = [0; SALT_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().chunks(2)) {
            *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
        }
        Some(Self(bytes))
    }
}

/// The salt is a secret, so it stays out of logs.
impl fmt::Debug for InstallSalt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("InstallSalt(..)")
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use assert_fs::{prelude::*, TempDir};
    use camino::Utf8PathBuf;
    use sha2::{Digest, Sha256};
    use speculoos::prelude::*;

    use super::{encode_hex, GraphId, GraphRef, InstallSalt, SALT_BYTES};

    fn graph_ref(graph_ref: &str) -> GraphRef {
        graph_ref.parse().unwrap()
    }

    fn salt_path(dir: &TempDir) -> Utf8PathBuf {
        Utf8PathBuf::try_from(dir.child("install_salt.txt").path().to_path_buf()).unwrap()
    }

    #[test]
    fn a_salt_is_generated_and_stored_when_there_is_none() {
        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);

        let salt = InstallSalt::read_or_create(&path).unwrap();

        assert_that!(std::fs::read_to_string(&path))
            .is_ok()
            .is_equal_to(salt.to_hex());
    }

    #[test]
    fn processes_starting_together_end_up_with_the_same_salt() {
        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);
        let start = std::sync::Barrier::new(16);

        let salts: Vec<InstallSalt> = std::thread::scope(|scope| {
            let starters: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        start.wait();
                        InstallSalt::read_or_create(&path).unwrap()
                    })
                })
                .collect();
            starters
                .into_iter()
                .map(|starter| starter.join().unwrap())
                .collect()
        });

        let stored = InstallSalt::read_or_create(&path).unwrap();
        assert_that!(salts).is_equal_to(vec![stored; 16]);
    }

    #[cfg(unix)]
    #[test]
    fn the_salt_can_only_be_read_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);

        InstallSalt::read_or_create(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_that!(mode & 0o777).is_equal_to(0o600);
    }

    #[test]
    fn the_stored_salt_is_read_back() {
        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);

        let created = InstallSalt::read_or_create(&path).unwrap();
        let read = InstallSalt::read_or_create(&path).unwrap();

        assert_that!(read).is_equal_to(created);
    }

    #[test]
    fn an_unusable_salt_is_replaced() {
        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);
        std::fs::write(&path, "not a salt").unwrap();

        let salt = InstallSalt::read_or_create(&path).unwrap();

        assert_that!(std::fs::read_to_string(&path))
            .is_ok()
            .is_equal_to(salt.to_hex());
    }

    #[test]
    fn two_installs_get_different_salts() {
        let first = InstallSalt::read_or_create(&salt_path(&TempDir::new().unwrap())).unwrap();
        let second = InstallSalt::read_or_create(&salt_path(&TempDir::new().unwrap())).unwrap();

        assert_that!(first).is_not_equal_to(second);
    }

    #[test]
    fn deleting_the_salt_resets_it() {
        let dir = TempDir::new().unwrap();
        let path = salt_path(&dir);
        let before = InstallSalt::read_or_create(&path).unwrap();

        std::fs::remove_file(&path).unwrap();

        assert_that!(InstallSalt::read_or_create(&path))
            .is_ok()
            .is_not_equal_to(before);
    }

    #[test]
    fn the_graph_identifier_is_a_keyed_hash_of_the_graph_ref() {
        let salt = InstallSalt([1; SALT_BYTES]);

        // HMAC-SHA256 with the salt as the key, worked out independently of this code.
        assert_that!(salt.graph_identifier(&graph_ref("my-graph@current"))).is_equal_to(
            GraphId::for_tests("015b2b8f15822524996e882cffbcf6a5d7660f804194b9cce129c4ec61f47d97"),
        );
    }

    #[test]
    fn the_same_install_gets_the_same_identifier_for_the_same_graph_ref() {
        let salt = InstallSalt([7; SALT_BYTES]);

        assert_that!(salt.graph_identifier(&graph_ref("my-graph@current")))
            .is_equal_to(salt.graph_identifier(&graph_ref("my-graph@current")));
    }

    #[test]
    fn a_graph_ref_without_a_variant_gets_the_identifier_of_the_current_variant() {
        let salt = InstallSalt([7; SALT_BYTES]);

        assert_that!(salt.graph_identifier(&graph_ref("my-graph")))
            .is_equal_to(salt.graph_identifier(&graph_ref("my-graph@current")));
    }

    #[test]
    fn different_graph_refs_get_different_identifiers() {
        let salt = InstallSalt([7; SALT_BYTES]);

        assert_that!(salt.graph_identifier(&graph_ref("my-graph@current")))
            .is_not_equal_to(salt.graph_identifier(&graph_ref("my-graph@staging")));
    }

    #[test]
    fn different_installs_get_different_identifiers_for_the_same_graph_ref() {
        assert_that!(InstallSalt([1; SALT_BYTES]).graph_identifier(&graph_ref("my-graph@current")))
            .is_not_equal_to(
                InstallSalt([2; SALT_BYTES]).graph_identifier(&graph_ref("my-graph@current")),
            );
    }

    /// With a random key this can't fail by chance. It guards against the identifier being
    /// changed to a plain, unkeyed hash of the graph ref.
    #[test]
    fn hashing_known_graph_refs_does_not_reproduce_the_identifier() {
        let salt = InstallSalt::read_or_create(&salt_path(&TempDir::new().unwrap())).unwrap();
        let graph_refs = [
            "my-graph@current",
            "my-graph",
            "my-graph@staging",
            "apollo@main",
        ];

        for written in graph_refs {
            let identifier = salt.graph_identifier(&graph_ref(written));
            for unsalted in [written.to_string(), graph_ref(written).to_string()] {
                assert_that!(identifier.clone()).is_not_equal_to(GraphId::for_tests(&encode_hex(
                    &Sha256::digest(unsalted.as_bytes()),
                )));
            }
        }
    }

    #[test]
    fn a_graph_id_serializes_as_a_plain_string() {
        assert_that!(serde_json::to_value(GraphId::for_tests("015b2b8f")))
            .is_ok()
            .is_equal_to(serde_json::json!("015b2b8f"));
    }

    #[test]
    fn the_debug_output_leaves_the_salt_out() {
        assert_that!(format!("{:?}", InstallSalt([1; SALT_BYTES])))
            .is_equal_to("InstallSalt(..)".to_string());
    }
}
