use std::io;

use anyhow::anyhow;
use clap::Parser;
use rover_client::shared::ValidationPeriod;
use serde::{Deserialize, Serialize};

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

    /// Also fail the check when a blocking contract variant's own check has failed, even if
    /// Studio's overall result for the check says it passed
    ///
    /// Without this, the command succeeds or fails as Studio's overall result for the check does,
    /// and the contract variants are still reported.
    #[arg(long)]
    pub fail_on_blocking_contract_checks: bool,
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

/// Parses a percentage (`25`, `0.5`) into the fraction (`0.25`, `0.005`) the Platform API takes.
fn parse_query_percentage_threshold(threshold: &str) -> Result<f64, io::Error> {
    let percentage = threshold
        .parse::<f64>()
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    // A range check rather than `!(..).contains()` on the other side, so that NaN is refused too.
    if (0.0..=100.0).contains(&percentage) {
        Ok(percentage / 100.0)
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            anyhow!("Valid numbers are in the range 0 <= x <= 100"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;
    use speculoos::prelude::*;

    use super::parse_query_percentage_threshold;

    #[rstest]
    #[case::zero("0", 0.0)]
    #[case::whole_percentage("25", 0.25)]
    #[case::one_percent("1", 0.01)]
    #[case::just_under_the_top("99", 0.99)]
    #[case::the_top("100", 1.0)]
    #[case::decimal("0.5", 0.005)]
    #[case::decimal_above_one("12.5", 0.125)]
    fn a_percentage_becomes_the_fraction_the_api_takes(#[case] input: &str, #[case] expected: f64) {
        assert_that!(parse_query_percentage_threshold(input).unwrap()).is_close_to(expected, 1e-9);
    }

    #[rstest]
    #[case::negative("-1")]
    #[case::over_the_top("100.5")]
    #[case::far_over("1000")]
    #[case::not_a_number("lots")]
    #[case::nan("NaN")]
    #[case::infinity("inf")]
    #[case::empty("")]
    fn anything_else_is_rejected(#[case] input: &str) {
        assert_that!(parse_query_percentage_threshold(input)).is_err();
    }
}
