use crate::errors::*;
use crate::presenters::{current_buffer_status_line_data, git_status_line_data};
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use git2::Repository;
use scribe::buffer::Position;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    view: &mut View,
    repo: &Option<Repository>,
) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);

    if let Some(buf) = workspace.current_buffer.as_ref() {
        let data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

        let colors = if buf.modified() {
            Colors::Warning
        } else {
            Colors::Inverted
        };

        presenter.print_status_line(&[
            StatusLineData {
                content: " NORMAL ".to_string(),
                style: Style::Default,
                colors,
            },
            buffer_status,
            git_status_line_data(repo, &buf.path),
        ]);

        presenter.set_cursor_type(CursorType::Block);

        presenter.present()?;
    } else {
        let content = [
            format!("Nexedit v{}", env!("CARGO_PKG_VERSION")),
            String::from("© 2024 d3vboi"),
            String::from(" "),
            String::from("Press \"?\" to view quick start guide"),
        ];
        let line_count = content.len();
        let vertical_offset = line_count / 2;

        for (line_no, line) in content.iter().enumerate() {
            let position = Position {
                line: (presenter.height() / 2 + line_no).saturating_sub(vertical_offset),
                offset: (presenter.width() / 2).saturating_sub(line.chars().count() / 2),
            };

            presenter.print(&position, Style::Default, Colors::Default, line);
        }

        presenter.set_cursor(None);
        presenter.present()?;
    }

    Ok(())
}
