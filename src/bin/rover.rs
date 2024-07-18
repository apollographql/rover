use robot_panic::setup_panic;
use rover::cli::Rover;
use tokio::runtime::Runtime;

#[calm_io::pipefail]
fn main() -> Result<_, std::io::Error> {
    setup_panic!(Metadata {
        name: rover::PKG_NAME.into(),
        version: rover::PKG_VERSION.into(),
        authors: rover::PKG_AUTHORS.into(),
        homepage: rover::PKG_HOMEPAGE.into(),
        repository: rover::PKG_REPOSITORY.into()
    });

    let rt = Runtime::new().expect("failed to start asynchronous runtime");
    Ok(rt.block_on(Rover::run_from_args()))
}
