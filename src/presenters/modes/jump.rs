use crate::errors::*;
use crate::models::application::modes::JumpMode;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, mode: &mut JumpMode, view: &mut View) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().ok_or(BUFFER_MISSING)?;
    let data = buf.data();

    mode.reset_display();

    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, Some(mode))?;

    presenter.print_status_line(&[
        StatusLineData {
            content: " JUMP ".to_string(),
            style: Style::Default,
            colors: Colors::Inverted,
        },
        buffer_status,
    ]);

    presenter.set_cursor(None);

    presenter.present()?;

    Ok(())
}
