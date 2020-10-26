use anyhow::Result;
use rover::*;
use sputnik::Session;
use structopt::StructOpt;

use std::thread;

fn main() -> Result<()> {
    logger::init();
    let app = cli::Rover::from_args();

    // attempt to create a new `Session` to capture anonymous usage data
    match Session::new(&app) {
        // if successful, report the usage data in the background
        Ok(session) => {
            // kicks off the reporting on a background thread
            let report_thread = thread::spawn(move || {
                // log + ignore errors because it is not in the critical path
                let _ = session.report().map_err(|e| {
                    log::debug!("{:?}", e);
                    e
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
            app_result?
        }

        // otherwise just run the app without reporting
        Err(_) => app.run()?,
    }

    Ok(())
}
