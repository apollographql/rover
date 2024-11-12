use robot_panic::setup_panic;
use rover::cli::Rover;

#[tokio::main]
#[calm_io::pipefail]
async fn main() -> Result<_, std::io::Error> {
    setup_panic!(Metadata {
        name: rover::PKG_NAME.into(),
        version: rover::PKG_VERSION.into(),
        authors: rover::PKG_AUTHORS.into(),
        homepage: rover::PKG_HOMEPAGE.into(),
        repository: rover::PKG_REPOSITORY.into()
    });

    Ok(Rover::run_from_args().await)
}
