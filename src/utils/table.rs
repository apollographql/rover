use prettytable::{
    format::{consts::FORMAT_BOX_CHARS, TableFormat},
    Table,
};

pub use prettytable::{cell, row};

pub fn get_table() -> Table {
    let mut table = Table::new();
    table.set_format(get_table_format());
    table
}

pub fn get_table_format() -> TableFormat {
    *FORMAT_BOX_CHARS
}
