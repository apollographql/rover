use comfy_table::Table;

pub fn get_table() -> Table {
    let mut table = Table::new();
    table.load_preset(comfy_table::presets::UTF8_FULL);
    table
}
