use robot_panic::setup_panic;
use rover::*;
use sputnik::Session;
use structopt::StructOpt;

use std::{process, thread};

fn main() {
    setup_panic!(Metadata {
        name: PKG_NAME.into(),
        version: PKG_VERSION.into(),
        authors: PKG_AUTHORS.into(),
        homepage: PKG_HOMEPAGE.into(),
        repository: PKG_REPOSITORY.into()
    });
    if let Err(error) = run() {
        tracing::debug!(?error);
        eprint!("{}", error);
        process::exit(1)
    } else {
        process::exit(0)
    }
}

fn run() -> Result<()> {
    let app = Rover::from_args();
    timber::init(app.log_level);
    tracing::trace!(command_structure = ?app);

    // attempt to create a new `Session` to capture anonymous usage data
    let output = match Session::new(&app) {
        // if successful, report the usage data in the background
        Ok(session) => {
            // kicks off the reporting on a background thread
            let report_thread = thread::spawn(move || {
                // log + ignore errors because it is not in the critical path
                let _ = session.report().map_err(|telemetry_error| {
                    tracing::debug!(?telemetry_error);
                    telemetry_error
                });
            });

            // kicks off the app on the main thread
            // don't return an error with ? quite yet
            // since we still want to report the usage data
            let app_result = app.run();

            // makes sure the reporting finishes in the background
            // before continuing.
            // ignore errors because it is not in the critical path
            let _ = report_thread.join();

            // return result of app execution
            // now that we have reported our usage data
            app_result
        }

        // otherwise just run the app without reporting
        Err(_) => app.run(),
    }?;

    output.print();
    Ok(())
}
