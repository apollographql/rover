use crate::command::RoverStdout;
use crate::{anyhow, Result};

use crate::utils::table::{self, cell, row};
use rover_client::shared::CheckResponse;

pub(crate) fn print_check_response(check_response: CheckResponse) -> Result<RoverStdout> {
    let num_changes = check_response.changes.len();

    let msg = match num_changes {
        0 => "There were no changes detected in the composed schema.".to_string(),
        _ => format!(
            "Compared {} schema changes against {} operations",
            check_response.changes.len(),
            check_response.number_of_checked_operations
        ),
    };

    eprintln!("{}", &msg);

    if !check_response.changes.is_empty() {
        let mut table = table::get_table();

        // bc => sets top row to be bold and center
        table.add_row(row![bc => "Change", "Code", "Description"]);
        for check in &check_response.changes {
            table.add_row(row![check.severity, check.code, check.description]);
        }

        println!("{}", table);
    }

    if let Some(url) = &check_response.target_url {
        eprintln!("View full details at {}", url);
    }
    match &check_response.num_failures {
        0 => Ok(RoverStdout::None),
        1 => Err(anyhow!("Encountered 1 failure.").into()),
        _ => Err(anyhow!("Encountered {} failures.", &check_response.num_failures).into()),
    }
}
