use camino::Utf8PathBuf;
use semver::Version;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
pub struct RouterBinary {
    exe: Utf8PathBuf,
    version: Version,
}

impl RouterBinary {
    pub fn new(exe: Utf8PathBuf, version: Version) -> RouterBinary {
        RouterBinary { exe, version }
    }
}
