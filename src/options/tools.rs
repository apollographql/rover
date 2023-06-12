use clap::Parser;
use serde::{Serialize, Deserialize};

// use std::{io::Read};

// use crate::RoverResult;

#[derive(Debug, Clone, Serialize, Deserialize, Parser)]
pub struct ToolsMergeOpt {
    /// The path to schema files to merge.
    #[arg(long, short = 's')]
    pub schemas: String,
}

impl ToolsMergeOpt {
    
}
