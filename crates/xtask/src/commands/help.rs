pub(crate) fn run() {
    eprintln!(
        "USAGE:
    cargo xtask <SUBCOMMAND>

SUBCOMMANDS:
    dist    builds binaries for distribution
    lint    check formatting and code style
    help    print this help text"
    );
}
