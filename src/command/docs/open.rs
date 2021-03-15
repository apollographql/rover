use crate::{command::RoverStdout, Result};
use crate::utils::browser;

use super::shortlinks;

use serde::Serialize;
use structopt::StructOpt;

#[derive(Debug, Serialize, StructOpt)]
pub struct Open {
    #[structopt(name = "slug", default_value = "docs", possible_values = &shortlinks::possible_shortlinks())]
    slug: String,
}

impl Open {
    pub fn run(&self) -> Result<RoverStdout> {
        let url = shortlinks::get_url_from_slug(&self.slug);
        browser::open(&url)?;
        Ok(RoverStdout::None)
    }
}
