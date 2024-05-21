use crate::errors::*;
use crate::models::application::modes::PathMode;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

pub fn display(workspace: &mut Workspace, mode: &PathMode, view: &mut View) -> Result<()> {
    let mut presenter = view.build_presenter()?;

    let buffer = workspace.current_buffer.as_ref().ok_or(BUFFER_MISSING)?;
    let data = buffer.data();
    presenter.print_buffer(buffer, &data, &workspace.syntax_set, None, None)?;

    let mode_display = format!(" {} ", mode);
    let search_input = format!(" {}", mode.input);

    let cursor_offset = mode_display.graphemes(true).count() + search_input.graphemes(true).count();

    presenter.print_status_line(&[
        StatusLineData {
            content: mode_display,
            style: Style::Default,
            colors: Colors::PathMode,
        },
        StatusLineData {
            content: search_input,
            style: Style::Default,
            colors: Colors::Focused,
        },
    ]);

    {
        let cursor_line = presenter.height() - 1;
        presenter.set_cursor(Some(Position {
            line: cursor_line,
            offset: cursor_offset,
        }));
    }

    presenter.set_cursor_type(CursorType::BlinkingBar);

    presenter.present()?;

    Ok(())
}
