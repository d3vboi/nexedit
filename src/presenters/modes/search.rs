use crate::errors::*;
use crate::models::application::modes::SearchMode;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;
use std::fmt::Write;

pub fn display(workspace: &mut Workspace, mode: &SearchMode, view: &mut View) -> Result<()> {
    let mut presenter = view.build_presenter()?;

    let buffer = workspace.current_buffer.as_ref().ok_or(BUFFER_MISSING)?;
    let data = buffer.data();
    presenter.print_buffer(
        buffer,
        &data,
        &workspace.syntax_set,
        mode.results.as_ref().map(|r| r.as_slice()),
        None,
    )?;

    let mut mode_display = String::with_capacity(10);
    write!(mode_display, " {} ", mode).unwrap();

    let mut search_input = String::with_capacity(mode.input.as_ref().unwrap_or(&String::new()).len() + 2);
    write!(search_input, " {}", mode.input.as_ref().unwrap_or(&String::new())).unwrap();

    let mut result_display = String::new();
    if !mode.insert {
        if let Some(ref results) = mode.results {
            if results.len() == 1 {
                result_display.push_str("1 match");
            } else {
                write!(result_display, "{} of {} matches", results.selected_index() + 1, results.len()).unwrap();
            }
        }
    }

    let cursor_offset = mode_display.graphemes(true).count() + search_input.graphemes(true).count();

    presenter.print_status_line(&[
        StatusLineData {
            content: mode_display,
            style: Style::Default,
            colors: Colors::SearchMode,
        },
        StatusLineData {
            content: search_input,
            style: Style::Default,
            colors: Colors::Focused,
        },
        StatusLineData {
            content: result_display,
            style: Style::Default,
            colors: Colors::Focused,
        },
    ]);

    if mode.insert {
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
