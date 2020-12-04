use std::path::PathBuf;
#[derive(Debug)]
pub enum SchemaLocation {
    Stdin,
    File(PathBuf),
}

pub fn parse_schema_location(loc: &str) -> SchemaLocation {
    if loc == "-" {
        SchemaLocation::Stdin
    } else {
        let path = PathBuf::from(loc);
        SchemaLocation::File(path)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_correctly_parses_stdin_flag() {
        unimplemented!();
    }

    #[test]
    fn it_correctly_parses_path_option() {
        unimplemented!();
    }

    #[test]
    fn it_errs_with_empty_path() {
        unimplemented!();
    }
}
