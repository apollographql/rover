use anyhow::anyhow;
use clap::Parser;
use serde::{Deserialize, Serialize};

use rover_client::shared::ValidationPeriod;

use std::io;

#[derive(Debug, Serialize, Deserialize, Parser)]
pub struct CheckConfigOpts {
    /// The minimum number of times a query or mutation must have been executed
    /// in order to be considered in the check operation
    #[arg(long, value_parser = parse_query_count_threshold)]
    pub query_count_threshold: Option<i64>,

    /// Minimum percentage of times a query or mutation must have been executed
    /// in the time window, relative to total request count, for it to be
    /// considered in the check. Valid numbers are in the range 0 <= x <= 100
    #[arg(long, value_parser = parse_query_percentage_threshold)]
    pub query_percentage_threshold: Option<f64>,

    /// Size of the time window with which to validate schema against (i.e "24h" or "1w 2d 5h")
    #[arg(long)]
    pub validation_period: Option<ValidationPeriod>,

    /// If the check should be run asynchronously and exit without waiting for check results
    #[arg(long)]
    pub background: bool,
}

fn parse_query_count_threshold(threshold: &str) -> Result<i64, io::Error> {
    let threshold = threshold
        .parse::<i64>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if threshold < 1 {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            anyhow!("The number of queries must be a positive integer."),
        ))
    } else {
        Ok(threshold)
    }
}

fn parse_query_percentage_threshold(threshold: &str) -> Result<f64, io::Error> {
    let threshold = threshold
        .parse::<i64>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    if !(0..=100).contains(&threshold) {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            anyhow!("Valid numbers are in the range 0 <= x <= 100"),
        ))
    } else {
        Ok((threshold / 100) as f64)
    }
}
