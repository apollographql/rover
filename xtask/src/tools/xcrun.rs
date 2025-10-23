use anyhow::{anyhow, Result};

use crate::{tools::Runner, utils::PKG_PROJECT_ROOT};

pub(crate) struct XcrunRunner {
    runner: Runner,
}

impl XcrunRunner {
    pub(crate) fn new() -> Self {
        let runner = Runner::new("xcrun");

        XcrunRunner { runner }
    }

    pub(crate) fn notarize(
        &mut self,
        dist_zip: &str,
        apple_username: &str,
        apple_team_id: &str,
        notarization_password: &str,
    ) -> Result<()> {
        crate::info!("Beginning notarization process...");
        self.runner.set_bash_descriptor(format!("xcrun notarytool submit {dist_zip} --apple-id {apple_username} --apple-team-id {apple_team_id} --password xxxx-xxxx-xxxx-xxxx --wait --timeout 20m"));
        let project_root = PKG_PROJECT_ROOT.clone();
        self.runner
            .exec(
                &[
                    "notarytool",
                    "submit",
                    dist_zip,
                    "--apple-id",
                    apple_username,
                    "--team-id",
                    apple_team_id,
                    "--password",
                    notarization_password,
                    "--wait",
                    "--timeout",
                    "20m",
                ],
                &project_root,
                None,
            )
            .map_err(|e| {
                anyhow!(
                    "{}",
                    e.to_string()
                        .replace(notarization_password, "xxxx-xxxx-xxxx-xxxx")
                )
            })?;
        crate::info!("Notarization successful.");
        Ok(())
    }
}
