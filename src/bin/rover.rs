use robot_panic::setup_panic;
use rover::cli::Rover;
use structopt::StructOpt;

fn main() {
    setup_panic!(Metadata {
        name: rover::PKG_NAME.into(),
        version: rover::PKG_VERSION.into(),
        authors: rover::PKG_AUTHORS.into(),
        homepage: rover::PKG_HOMEPAGE.into(),
        repository: rover::PKG_REPOSITORY.into()
    });
    let app = Rover::from_args();
    app.run();
}
