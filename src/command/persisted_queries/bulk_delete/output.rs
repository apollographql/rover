use crate::command::output::CliOutput;

/// Output of `rover persisted-queries bulk-delete`.
#[derive(Debug)]
pub enum BulkDeleteOutput {
    /// The job was submitted (or resumed) and `--no-wait` was passed, so Rover
    /// exited without waiting for it to finish.
    Submitted { job_id: String },
    /// The job ran to completion.
    Success {
        job_id: String,
        list_name: String,
        /// The revision produced by the job's final chunk. `None` means no
        /// operations matched the filter, so nothing was deleted and no new
        /// revision was created.
        revision: Option<i64>,
    },
}

impl CliOutput for BulkDeleteOutput {
    fn text(&self) -> String {
        match self {
            BulkDeleteOutput::Submitted { job_id } => format!(
                "Started bulk deletion job {job_id}. It will keep running on the server even if this command is interrupted; resume watching it with `--job-id {job_id}`."
            ),
            BulkDeleteOutput::Success {
                list_name,
                revision: Some(revision),
                ..
            } => format!("Bulk deletion finished. {list_name} is now at revision {revision}."),
            BulkDeleteOutput::Success {
                list_name,
                revision: None,
                ..
            } => format!(
                "Bulk deletion finished. No operations in {list_name} matched the filter; nothing was deleted."
            ),
        }
    }

    fn json(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            BulkDeleteOutput::Submitted { job_id } => Ok(serde_json::json!({
                "status": "submitted",
                "job_id": job_id,
            })),
            BulkDeleteOutput::Success {
                job_id,
                list_name,
                revision,
            } => Ok(serde_json::json!({
                "status": "success",
                "job_id": job_id,
                "list_name": list_name,
                "revision": revision,
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use speculoos::prelude::*;

    use super::*;

    #[test]
    fn text_output_reports_the_final_revision() {
        let out = BulkDeleteOutput::Success {
            job_id: "job-123".to_string(),
            list_name: "my-list".to_string(),
            revision: Some(7),
        };
        assert_that!(out.text())
            .is_equal_to("Bulk deletion finished. my-list is now at revision 7.".to_string());
    }

    #[test]
    fn text_output_reports_no_changes_when_nothing_matched() {
        let out = BulkDeleteOutput::Success {
            job_id: "job-123".to_string(),
            list_name: "my-list".to_string(),
            revision: None,
        };
        assert_that!(out.text()).is_equal_to(
            "Bulk deletion finished. No operations in my-list matched the filter; nothing was deleted."
                .to_string(),
        );
    }

    #[test]
    fn json_output_includes_the_job_id_when_submitted() {
        let out = BulkDeleteOutput::Submitted {
            job_id: "job-123".to_string(),
        };
        assert_that!(out.json().unwrap()).is_equal_to(serde_json::json!({
            "status": "submitted",
            "job_id": "job-123",
        }));
    }
}
