use crate::errors::*;
use cli_clipboard::{ClipboardContext, ClipboardProvider};

#[derive(Debug, PartialEq)]
pub enum ClipboardContent {
    Inline(String),
    Block(String),
    None,
}

pub struct Clipboard {
    content: ClipboardContent,
    system_clipboard: Option<ClipboardContext>,
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl Clipboard {
    pub fn new() -> Clipboard {
        let system_clipboard = match ClipboardProvider::new() {
            Ok(clipboard) => Some(clipboard),
            Err(_) => None,
        };

        Clipboard {
            content: ClipboardContent::None,
            system_clipboard,
        }
    }

    pub fn get_content(&mut self) -> &ClipboardContent {
        let new_content = match self.system_clipboard {
            Some(ref mut clipboard) => {
                match clipboard.get_contents() {
                    Ok(content) => {
                        if content.is_empty() {
                            None
                        } else {
                            match self.content {
                                ClipboardContent::Inline(ref app_content)
                                | ClipboardContent::Block(ref app_content) => {
                                    if content != *app_content {
                                        Some(ClipboardContent::Inline(content))
                                    } else {
                                        None
                                    }
                                }
                                _ => Some(ClipboardContent::Inline(content)),
                            }
                        }
                    }
                    _ => None,
                }
            }
            None => None,
        };

        if let Some(new_content) = new_content {
            self.content = new_content;
        }

        &self.content
    }

    pub fn set_content(&mut self, content: ClipboardContent) -> Result<()> {
        self.content = content;

        match self.content {
            ClipboardContent::Inline(ref app_content)
            | ClipboardContent::Block(ref app_content) => {
                if let Some(ref mut clipboard) = self.system_clipboard {
                    return clipboard
                        .set_contents(app_content.clone())
                        .map_err(|_| Error::from("Failed to update system clipboard"));
                }
            }
            _ => (),
        }

        Ok(())
    }
}
