use std::fmt;
use std::path::PathBuf;

pub struct DisplayablePath(pub PathBuf);

impl fmt::Display for DisplayablePath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let DisplayablePath(path) = self;
        write!(f, "{}", path.to_string_lossy())
    }
}
