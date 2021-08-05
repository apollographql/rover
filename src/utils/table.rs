use prettytable::{format::consts::FORMAT_BOX_CHARS, Table};

pub use prettytable::{cell, row};

pub fn get_table() -> Table {
    let mut table = Table::new();
    table.set_format(*FORMAT_BOX_CHARS);
    table
}
