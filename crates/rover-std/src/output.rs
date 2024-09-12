use camino::Utf8PathBuf;

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum RoverOutputDestination {
    File(Utf8PathBuf),
    Stdout,
}
