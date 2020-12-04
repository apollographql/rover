use std::path::PathBuf;

// type Stdin = Box<dyn std::io::Read>;
trait ReadAndDebug: std::io::Read + std::fmt::Debug {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result;
}

impl ReadAndDebug for std::io::Stdin {
    fn fmt(&self, _f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Ok(())
    }
}
#[derive(Debug)]
pub enum SchemaLocation {
    Stdin(Box<dyn ReadAndDebug>),
    File(PathBuf),
}

// Stdin(Box<dyn std::io::Read>),
pub fn parse_schema_location(loc: &str) -> SchemaLocation {
    if loc == "-" {
        SchemaLocation::Stdin(Box::new(std::io::stdin()))
    } else {
        let path = PathBuf::from(loc);
        SchemaLocation::File(path)
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_schema_location, SchemaLocation};
    // use std::path::PathBuf;

    #[test]
    fn it_correctly_parses_stdin_flag() {
        let loc = parse_schema_location("-");
        match loc {
            SchemaLocation::Stdin(_) => {
                assert!(true);
            }
            SchemaLocation::File(_) => {
                panic!("Parsing schema location failed. Should be stdin. Found File");
            }
        }
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
