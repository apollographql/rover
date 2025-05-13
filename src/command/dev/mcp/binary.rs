use camino::Utf8PathBuf;
use semver::Version;

#[derive(Clone, Debug)]
#[cfg_attr(test, derive(derive_getters::Getters))]
#[allow(unused)]
pub struct McpServerBinary {
    exe: Utf8PathBuf,
    version: Version,
}

impl McpServerBinary {
    pub fn new(exe: Utf8PathBuf, version: Version) -> McpServerBinary {
        McpServerBinary { exe, version }
    }
}
