use std::fs::read_to_string;
use std::path::Path;
use std::process::Command;

use tempfile::NamedTempFile;

pub fn pdftotext(path: &Path, layout: bool) -> std::io::Result<String> {
    let temp_file = NamedTempFile::new()?;
    let mut command = Command::new("pdftotext");
    command.arg(path).arg(temp_file.path());
    if layout {
        command.arg("-layout");
    }
    let mut child = command.spawn()?;
    let _ = child.wait()?;
    read_to_string(temp_file.path())
}
