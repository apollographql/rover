use serde::{Deserialize, Serialize};
use structopt::StructOpt;

use rover_client::shared::ValidationPeriod;

use crate::{error::RoverError, Result};

#[derive(Debug, Serialize, Deserialize, StructOpt)]
pub struct CheckConfigOpts {
    /// The minimum number of times a query or mutation must have been executed
    /// in order to be considered in the check operation
    #[structopt(long, parse(try_from_str = parse_query_count_threshold))]
    pub query_count_threshold: Option<i64>,

    /// Minimum percentage of times a query or mutation must have been executed
    /// in the time window, relative to total request count, for it to be
    /// considered in the check. Valid numbers are in the range 0 <= x <= 100
    #[structopt(long, parse(try_from_str = parse_query_percentage_threshold))]
    pub query_percentage_threshold: Option<f64>,

    /// Size of the time window with which to validate schema against (i.e "24h" or "1w 2d 5h")
    #[structopt(long)]
    pub validation_period: Option<ValidationPeriod>,
}

fn parse_query_count_threshold(threshold: &str) -> Result<i64> {
    let threshold = threshold.parse::<i64>()?;
    if threshold < 1 {
        Err(RoverError::parse_error(
            "The number of queries must be a positive integer.",
        ))
    } else {
        Ok(threshold)
    }
}

fn parse_query_percentage_threshold(threshold: &str) -> Result<f64> {
    let threshold = threshold.parse::<i64>()?;
    if !(0..=100).contains(&threshold) {
        Err(RoverError::parse_error(
            "Valid numbers are in the range 0 <= x <= 100",
        ))
    } else {
        Ok((threshold / 100) as f64)
    }
}
