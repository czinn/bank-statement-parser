use std::path::Path;

pub trait StatementFormat {
    fn parse_file(path: &Path) -> Self;
}
