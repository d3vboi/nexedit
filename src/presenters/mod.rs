pub mod error;
pub mod modes;

use crate::view::{Colors, StatusLineData, Style};
use git2::{self, Repository, Status};
use scribe::Workspace;
use std::path::{Path, PathBuf};

fn path_as_title(path: &Path) -> String {
    format!(" {}", path.to_string_lossy())
}

fn current_buffer_status_line_data(workspace: &mut Workspace) -> StatusLineData {
    let modified = workspace
        .current_buffer
        .as_ref()
        .map(|b| b.modified())
        .unwrap_or(false);

    let (content, style) = workspace
        .current_buffer_path()
        .map(|path| {
            if modified {
                let mut title = path_as_title(path);
                title.push('*');

                (title, Style::Bold)
            } else {
                (path_as_title(path), Style::Default)
            }
        })
        .unwrap_or((String::new(), Style::Default));

    StatusLineData {
        content,
        style,
        colors: Colors::Focused,
    }
}

fn git_status_line_data(repo: &Option<Repository>, path: &Option<PathBuf>) -> StatusLineData {
    let mut content = String::new();
    if let Some(ref repo) = *repo {
        if let Some(ref path) = *path {
            if let Some(repo_path) = repo.workdir() {
                if let Ok(relative_path) = path.strip_prefix(repo_path) {
                    if let Ok(status) = repo.status_file(relative_path) {
                        content = presentable_status(&status).to_string();
                    }
                }
            }
        }
    }

    StatusLineData {
        content,
        style: Style::Default,
        colors: Colors::Focused,
    }
}
fn presentable_status(status: &Status) -> &str {
    if status.contains(git2::Status::WT_NEW) {
        if status.contains(git2::Status::INDEX_NEW) {
            "[partially staged]"
        } else {
            "[untracked]"
        }
    } else if status.contains(git2::Status::INDEX_NEW) {
        "[staged]"
    } else if status.contains(git2::Status::WT_MODIFIED) {
        if status.contains(git2::Status::INDEX_MODIFIED) {
            "[partially staged]"
        } else {
            "[modified]"
        }
    } else if status.contains(git2::Status::INDEX_MODIFIED) {
        "[staged]"
    } else {
        "[ok]"
    }
}

#[cfg(test)]
mod tests {
    use super::presentable_status;
    use git2;

    #[test]
    pub fn presentable_status_returns_untracked_when_status_is_locally_new() {
        let status = git2::Status::WT_NEW;
        assert_eq!(presentable_status(&status), "[untracked]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_ok_when_status_unmodified() {
        let status = git2::Status::CURRENT;
        assert_eq!(presentable_status(&status), "[ok]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_staged_when_only_modified_in_index() {
        let status = git2::Status::INDEX_MODIFIED;
        assert_eq!(presentable_status(&status), "[staged]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_staged_when_new_in_index() {
        let status = git2::Status::INDEX_NEW;
        assert_eq!(presentable_status(&status), "[staged]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_partially_staged_when_modified_locally_and_in_index() {
        let status = git2::Status::WT_MODIFIED | git2::Status::INDEX_MODIFIED;
        assert_eq!(
            presentable_status(&status),
            "[partially staged]".to_string()
        );
    }

    #[test]
    pub fn presentable_status_returns_partially_staged_when_new_locally_and_in_index() {
        let status = git2::Status::WT_NEW | git2::Status::INDEX_NEW;
        assert_eq!(
            presentable_status(&status),
            "[partially staged]".to_string()
        );
    }
}
