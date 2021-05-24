pub(crate) fn run() {
    eprintln!(
        "USAGE:
    cargo xtask <SUBCOMMAND>

FLAGS:
    -v, --verbose    Prints more verbose information

SUBCOMMANDS:
    dist    builds binaries for distribution
    lint    check formatting and code style
    test    run unit/integration tests
    help    print this help text"
    );
}
