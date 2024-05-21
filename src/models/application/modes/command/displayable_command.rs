use crate::commands::Command;
use std::fmt;

pub struct DisplayableCommand {
    pub description: &'static str,
    pub command: Command,
}

impl fmt::Display for DisplayableCommand {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description)
    }
}
