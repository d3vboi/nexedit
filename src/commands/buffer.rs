use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::modes::ConfirmMode;
use crate::models::application::{Application, ClipboardContent, Mode};
use crate::util;
use crate::util::token::{adjacent_token_position, Direction};
use scribe::buffer::{Buffer, Position, Range, Token};
use std::io::Write;
use std::mem;
use std::process::Stdio;

pub fn save(app: &mut Application) -> Result {
    remove_trailing_whitespace(app)?;
    ensure_trailing_newline(app)?;

    let path = app
        .workspace
        .current_buffer
        .as_ref()
        .ok_or(BUFFER_MISSING)?
        .path
        .clone(); // clone instead of borrow as we call another command later

    if let Some(path) = path {
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .save()
            .chain_err(|| BUFFER_SAVE_FAILED)?;

        if app.preferences.borrow().format_on_save(&path) {
            format(app)?;

            app.workspace
                .current_buffer
                .as_mut()
                .unwrap()
                .save()
                .chain_err(|| BUFFER_SAVE_FAILED)?;
        }
    } else {
        commands::application::switch_to_path_mode(app)?;
        if let Mode::Path(ref mut mode) = app.mode {
            mode.save_on_accept = true;
        }
    }

    Ok(())
}

pub fn reload(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .reload()
        .chain_err(|| BUFFER_RELOAD_FAILED)
}

pub fn delete(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .delete();
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn delete_token(app: &mut Application) -> Result {
    let mut subsequent_token_on_line = false;

    if let Some(buffer) = app.workspace.current_buffer.as_ref() {
        if let Some(position) = adjacent_token_position(buffer, false, Direction::Forward) {
            if position.line == buffer.cursor.line {
                subsequent_token_on_line = true;
            }
        }
    } else {
        bail!(BUFFER_MISSING);
    }

    if subsequent_token_on_line {
        commands::application::switch_to_select_mode(app)?;
        commands::cursor::move_to_start_of_next_token(app)?;
        commands::selection::copy_and_delete(app)?;
        commands::application::switch_to_normal_mode(app)?;
        commands::view::scroll_to_cursor(app)?;
    } else {
        commands::buffer::delete_rest_of_line(app)?;
    }

    Ok(())
}

pub fn delete_current_token(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        let cursor_position = buffer.cursor.position;
        let token_start = adjacent_token_position(buffer, true, Direction::Backward).unwrap_or(cursor_position);
        let token_end = adjacent_token_position(buffer, true, Direction::Forward).unwrap_or(cursor_position);

        buffer.delete_range(Range::new(token_start, token_end));
        buffer.cursor.position = token_start;
        commands::view::scroll_to_cursor(app)?;
    } else {
        bail!(BUFFER_MISSING);
    }

    Ok(())
}


pub fn delete_current_line(app: &mut Application) -> Result {
    commands::application::switch_to_select_line_mode(app)?;
    commands::selection::copy_and_delete(app)?;
    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn copy_current_line(app: &mut Application) -> Result {
    commands::application::switch_to_select_line_mode(app)?;
    commands::selection::copy(app)?;
    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn merge_next_line(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let current_line = buffer.cursor.line;
    let data = buffer.data();

    data.lines()
        .nth(current_line + 1)
        .ok_or("No line below current line")?;

    let mut merged_lines: String = buffer
        .data()
        .lines()
        .enumerate()
        .skip(current_line)
        .take(2)
        .map(|(index, line)| {
            if index == current_line {
                format!("{} ", line)
            } else {
                line.trim_start().to_string()
            }
        })
        .collect();

    if buffer.data().lines().nth(current_line + 2).is_some() {
        merged_lines.push('\n');
    }

    buffer.start_operation_group();
    let target_position = Position {
        line: current_line,
        offset: data.lines().nth(current_line).unwrap().len(),
    };
    buffer.delete_range(Range::new(
        Position {
            line: current_line,
            offset: 0,
        },
        Position {
            line: current_line + 2,
            offset: 0,
        },
    ));
    buffer.cursor.move_to(Position {
        line: current_line,
        offset: 0,
    });
    buffer.insert(merged_lines);
    buffer.cursor.move_to(target_position);
    buffer.end_operation_group();

    Ok(())
}

pub fn close(app: &mut Application) -> Result {
    let (unmodified, empty) = if let Some(buf) = app.workspace.current_buffer.as_ref() {
        (!buf.modified(), buf.data().is_empty())
    } else {
        bail!(BUFFER_MISSING);
    };
    let confirm_mode = matches!(app.mode, Mode::Confirm(_));

    if unmodified || empty || confirm_mode {
        app.view.forget_buffer(
            app.workspace
                .current_buffer
                .as_ref()
                .ok_or(BUFFER_MISSING)?,
        )?;
        app.workspace.close_current_buffer();
    } else {
        let confirm_mode = ConfirmMode::new(close);
        app.mode = Mode::Confirm(confirm_mode);
    }

    Ok(())
}

pub fn close_others(app: &mut Application) -> Result {
    let id = app
        .workspace
        .current_buffer
        .as_ref()
        .map(|b| b.id)
        .ok_or(BUFFER_MISSING)?;
    let mut modified_buffer = false;

    loop {
        if app.workspace.current_buffer.as_ref().map(|b| b.id) == Some(id) {
            app.workspace.next_buffer();
        }

        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            if buf.id == id {
                break;
            } else if buf.modified() && !buf.data().is_empty() {
                modified_buffer = true;
            } else {
                app.view.forget_buffer(buf)?;
            }
        }

        if modified_buffer {
            let confirm_mode = ConfirmMode::new(close_others_confirm);
            app.mode = Mode::Confirm(confirm_mode);
            break;
        }

        app.workspace.close_current_buffer();
    }

    Ok(())
}

pub fn close_others_confirm(app: &mut Application) -> Result {
    if let Some(buf) = app.workspace.current_buffer.as_ref() {
        app.view.forget_buffer(buf)?;
    }
    app.workspace.close_current_buffer();
    commands::application::switch_to_normal_mode(app)?;

    Ok(())
}

pub fn backspace(app: &mut Application) -> Result {
    let mut outdent = false;

    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        if buffer.cursor.offset == 0 {
            buffer.cursor.move_up();
            buffer.cursor.move_to_end_of_line();
            buffer.delete();
        } else {
            let data = buffer.data();
            let current_line = data
                .lines()
                .nth(buffer.cursor.line)
                .ok_or(CURRENT_LINE_MISSING)?;
            if current_line.chars().all(|c| c.is_whitespace()) {
                outdent = true
            } else {
                buffer.cursor.move_left();
                buffer.delete();
            }
        }
    } else {
        bail!(BUFFER_MISSING);
    }

    if outdent {
        commands::buffer::outdent_line(app)?;
    }
    commands::view::scroll_to_cursor(app)
}

pub fn insert_char(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        if let Some(Key::Char(character)) = *app.view.last_key() {
            buffer.insert(character.to_string());
            buffer.cursor.move_right();
        } else {
            bail!("No character to insert");
        }
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn display_current_scope(app: &mut Application) -> Result {
    let scope_display_buffer = {
        let mut scope_stack = None;
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .ok_or(BUFFER_MISSING)?;
        let tokens = app
            .workspace
            .current_buffer_tokens()
            .chain_err(|| BUFFER_TOKENS_FAILED)?;
        let mut token_iter = tokens.iter().chain_err(|| BUFFER_PARSE_FAILED)?;

        for token in &mut token_iter {
            if let Token::Lexeme(lexeme) = token {
                if lexeme.position > *buffer.cursor {
                    break;
                }

                scope_stack = Some(lexeme.scope);
            }
        }

        if let Some(e) = token_iter.error {
            Err(e).chain_err(|| BUFFER_PARSE_FAILED)?;
        }

        let mut scope_display_buffer = Buffer::new();
        for scope in scope_stack.iter() {
            scope_display_buffer.insert(format!("{}\n", scope));
        }

        scope_display_buffer
    };
    util::add_buffer(scope_display_buffer, app)
}

pub fn insert_newline(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        buffer.insert("\n");

        let position = buffer.cursor.clone();
        buffer.cursor.move_down();
        buffer.cursor.move_to_start_of_line();

        let data = buffer.data();
        let end_of_current_line = data
            .lines()
            .nth(position.line)
            .map(|l| (l.as_ptr() as usize) + l.len())
            .unwrap();
        let offset = end_of_current_line - (data.as_str().as_ptr() as usize);
        let (previous_content, _) = data.split_at(offset);

        let nearest_non_blank_line = previous_content.lines().rev().find(|line| !line.is_empty());
        let indent_content = match nearest_non_blank_line {
            Some(line) => line.chars().take_while(|&c| c.is_whitespace()).collect(),
            None => String::new(),
        };

        let indent_length = indent_content.chars().count();
        buffer.insert(indent_content);
        buffer.cursor.move_to(Position {
            line: position.line + 1,
            offset: indent_length,
        });
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn indent_line(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let tab_content = app.preferences.borrow().tab_content(buffer.path.as_ref());

    let target_position = match app.mode {
        Mode::Insert => Position {
            line: buffer.cursor.line,
            offset: buffer.cursor.offset + tab_content.chars().count(),
        },
        _ => *buffer.cursor.clone(),
    };

    let lines = match app.mode {
        Mode::SelectLine(ref mode) => {
            if mode.anchor >= buffer.cursor.line {
                buffer.cursor.line..mode.anchor + 1
            } else {
                mode.anchor..buffer.cursor.line + 1
            }
        }
        _ => buffer.cursor.line..buffer.cursor.line + 1,
    };

    buffer.start_operation_group();
    for line in lines {
        buffer.cursor.move_to(Position { line, offset: 0 });
        buffer.insert(tab_content.clone());
    }
    buffer.end_operation_group();

    buffer.cursor.move_to(target_position);

    Ok(())
}

pub fn outdent_line(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let tab_content = app.preferences.borrow().tab_content(buffer.path.as_ref());

    let data = buffer.data();

    let lines = match app.mode {
        Mode::SelectLine(ref mode) => {
            if mode.anchor >= buffer.cursor.line {
                buffer.cursor.line..mode.anchor + 1
            } else {
                mode.anchor..buffer.cursor.line + 1
            }
        }
        _ => buffer.cursor.line..buffer.cursor.line + 1,
    };

    buffer.start_operation_group();

    for line in lines {
        if let Some(content) = data.lines().nth(line) {
            let mut space_char_count = 0;

            if tab_content.starts_with('\t') {
                if content.starts_with('\t') {
                    space_char_count = 1;
                }
            } else {
                for character in content.chars().take(tab_content.chars().count()) {
                    if character == ' ' {
                        space_char_count += 1;
                    } else {
                        break;
                    }
                }
            }

            if space_char_count > 0 {
                buffer.delete_range(Range::new(
                    Position { line, offset: 0 },
                    Position {
                        line,
                        offset: space_char_count,
                    },
                ));

                let target_offset = buffer.cursor.offset.saturating_sub(space_char_count);
                let target_line = buffer.cursor.line;

                buffer.cursor.move_to(Position {
                    line: target_line,
                    offset: target_offset,
                });
            }
        }
    }

    buffer.end_operation_group();

    Ok(())
}

pub fn toggle_line_comment(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let original_cursor = *buffer.cursor.clone();

    let comment_prefix = {
        let path = buffer.path.as_ref().ok_or(BUFFER_PATH_MISSING)?;
        let prefix = app
            .preferences
            .borrow()
            .line_comment_prefix(path)
            .ok_or("No line comment prefix for the current buffer")?;

        prefix + " " // implicitly add trailing space
    };

    let line_numbers = match app.mode {
        Mode::SelectLine(ref mode) => {
            if mode.anchor >= buffer.cursor.line {
                buffer.cursor.line..mode.anchor + 1
            } else {
                mode.anchor..buffer.cursor.line + 1
            }
        }
        _ => buffer.cursor.line..buffer.cursor.line + 1,
    };

    let buffer_range = Range::new(
        Position {
            line: line_numbers.start,
            offset: 0,
        },
        Position {
            line: line_numbers.end,
            offset: 0,
        },
    );

    let buffer_range_content = buffer.read(&buffer_range).ok_or(CURRENT_LINE_MISSING)?;

    let lines: Vec<(usize, &str)> = line_numbers
        .zip(buffer_range_content.split('\n')) // produces (<line number>, <line content>)
        .filter(|(_, line)| !line.trim().is_empty()) // filter out any empty (non-whitespace-only) lines
        .collect();

    let (toggle, offset) = lines
        .iter()
        .map(|(_, line)| {
            let content = line.trim_start();
            (
                content.starts_with(&comment_prefix),
                line.len() - content.len(),
            )
        })
        .fold(
            (true, std::usize::MAX),
            |(folded_toggle, folded_offset), (has_comment, offset)| {
                (folded_toggle & has_comment, folded_offset.min(offset))
            },
        );

    buffer.start_operation_group();
    if !toggle {
        add_line_comment(buffer, &lines, offset, &comment_prefix);
    } else {
        remove_line_comment(buffer, &lines, &comment_prefix);
    }
    buffer.end_operation_group();

    buffer.cursor.move_to(original_cursor);

    Ok(())
}

fn add_line_comment(buffer: &mut Buffer, lines: &[(usize, &str)], offset: usize, prefix: &str) {
    for (line_number, _) in lines {
        let target = Position {
            line: *line_number,
            offset,
        };

        buffer.cursor.move_to(target);
        buffer.insert(prefix);
    }
}

fn remove_line_comment(buffer: &mut Buffer, lines: &[(usize, &str)], prefix: &str) {
    for (line_number, line) in lines {
        let start = Position {
            line: *line_number,
            offset: line.len() - line.trim_start().len(),
        };

        let end = Position {
            line: *line_number,
            offset: start.offset + prefix.len(),
        };

        buffer.delete_range(Range::new(start, end));
    }
}

pub fn change_token(app: &mut Application) -> Result {
    commands::buffer::delete_token(app)?;
    commands::application::switch_to_insert_mode(app)?;

    Ok(())
}

pub fn delete_rest_of_line(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;

    let starting_position = *buffer.cursor;
    let target_line = buffer.cursor.line + 1;
    buffer.start_operation_group();
    buffer.delete_range(Range::new(
        starting_position,
        Position {
            line: target_line,
            offset: 0,
        },
    ));

    buffer.insert("\n");

    Ok(())
}

pub fn change_rest_of_line(app: &mut Application) -> Result {
    commands::buffer::delete_rest_of_line(app)?;
    commands::application::switch_to_insert_mode(app)?;

    Ok(())
}

pub fn start_command_group(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .start_operation_group();

    Ok(())
}

pub fn end_command_group(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .end_operation_group();

    Ok(())
}

pub fn undo(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .undo();
    commands::view::scroll_to_cursor(app).chain_err(|| "Couldn't scroll to cursor after undoing.")
}

pub fn redo(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?
        .redo();
    commands::view::scroll_to_cursor(app).chain_err(|| "Couldn't scroll to cursor after redoing.")
}

pub fn paste(app: &mut Application) -> Result {
    let insert_below = match app.mode {
        Mode::Select(_) | Mode::SelectLine(_) | Mode::Search(_) => {
            commands::selection::delete(app)
                .chain_err(|| "Couldn't delete selection prior to pasting.")?;
            false
        }
        _ => true,
    };

    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        match *app.clipboard.get_content() {
            ClipboardContent::Inline(ref content) => buffer.insert(content.clone()),
            ClipboardContent::Block(ref content) => {
                let original_cursor_position = *buffer.cursor.clone();
                let line = original_cursor_position.line;

                if insert_below {
                    buffer.cursor.move_to(Position {
                        line: line + 1,
                        offset: 0,
                    });

                    if *buffer.cursor == original_cursor_position {
                        if let Some(line_content) = buffer.data().lines().nth(line) {
                            buffer.cursor.move_to(Position {
                                line,
                                offset: line_content.len(),
                            });
                            buffer.insert(format!("\n{}", content));
                            buffer.cursor.move_to(original_cursor_position);
                        } else {
                            buffer.insert(content.clone());
                        }
                    } else {
                        buffer.insert(content.clone());
                    }
                } else {
                    buffer.insert(content.clone());
                }
            }
            ClipboardContent::None => (),
        }
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn paste_above(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;

    if let ClipboardContent::Block(ref content) = *app.clipboard.get_content() {
        let mut start_of_line = Position {
            line: buffer.cursor.line,
            offset: 0,
        };

        mem::swap(&mut *buffer.cursor, &mut start_of_line);
        buffer.insert(content.clone());
        mem::swap(&mut *buffer.cursor, &mut start_of_line);
    }

    Ok(())
}

pub fn remove_trailing_whitespace(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let mut line = 0;
    let mut offset = 0;
    let mut space_count = 0;
    let mut ranges = Vec::new();

    for character in buffer.data().chars() {
        if character == '\n' {
            if space_count > 0 {
                ranges.push(Range::new(
                    Position {
                        line,
                        offset: offset - space_count,
                    },
                    Position { line, offset },
                ));
            }

            line += 1;
            offset = 0;
            space_count = 0;
        } else {
            if character == ' ' || character == '\t' {
                space_count += 1;
            } else {
                space_count = 0;
            }

            offset += 1;
        }
    }

    if space_count > 0 {
        ranges.push(Range::new(
            Position {
                line,
                offset: offset - space_count,
            },
            Position { line, offset },
        ));
    }

    for range in ranges.into_iter().rev() {
        buffer.delete_range(range);
    }

    Ok(())
}

pub fn ensure_trailing_newline(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;

    let data = buffer.data();
    if let Some(c) = data.chars().last() {
        if c != '\n' {
            let (line_no, line) = data
                .lines()
                .enumerate()
                .last()
                .ok_or("Couldn't find the last line to insert a trailing newline")?;
            let original_position = *buffer.cursor;
            let target_position = Position {
                line: line_no,
                offset: line.chars().count(),
            };

            if buffer.cursor.move_to(target_position) {
                buffer.insert("\n");
                buffer.cursor.move_to(original_position);
            } else {
                bail!("Couldn't move to the end of the buffer and insert a newline.");
            }
        }
    } else {
        buffer.insert('\n'); // Empty buffer
    }

    Ok(())
}

pub fn insert_tab(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;
    let tab_content = app.preferences.borrow().tab_content(buffer.path.as_ref());
    let tab_content_width = tab_content.chars().count();
    buffer.insert(tab_content);

    for _ in 0..tab_content_width {
        buffer.cursor.move_right();
    }

    Ok(())
}

pub fn format(app: &mut Application) -> Result {
    let buf = app
        .workspace
        .current_buffer
        .as_mut()
        .ok_or(BUFFER_MISSING)?;

    let path = buf.path.as_ref().ok_or(BUFFER_PATH_MISSING)?;
    let mut format_command = app
        .preferences
        .borrow()
        .format_command(path)
        .ok_or(FORMAT_TOOL_MISSING)?;
    let data = buf.data();

    let mut process = format_command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .chain_err(|| "Failed to spawn format tool")?;

    let mut format_input = process.stdin.take().chain_err(|| "Failed to open stdin")?;
    std::thread::spawn(move || {
        format_input
            .write_all(data.as_bytes())
            .expect("Failed to write to stdin");
    });

    let output = process
        .wait_with_output()
        .chain_err(|| "Failed to read stdout")?;

    if output.status.success() {
        let content = String::from_utf8(output.stdout)
            .chain_err(|| "Failed to parse format tool output as UTF8")?;
        buf.replace(content);

        Ok(())
    } else {
        let error = String::from_utf8(output.stderr)
            .unwrap_or(String::from("Failed to parse stderr output as UTF8"));

        Err(Error::from(error))
            .chain_err(|| format!("Format tool failed with code {}", output.status))
    }
}

#[cfg(test)]
mod tests {
    use crate::commands;
    use crate::models::application::{ClipboardContent, Mode, Preferences};
    use crate::models::Application;
    use scribe::buffer::Position;
    use scribe::Buffer;
    use std::env;
    use std::fs::File;
    use std::io::Write;
    use std::path::Path;
    use yaml_rust::yaml::YamlLoader;

    #[test]
    fn insert_newline_uses_current_line_indentation() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        buffer.insert("    nexedit");
        let position = Position { line: 0, offset: 7 };
        buffer.cursor.move_to(position);

        app.workspace.add_buffer(buffer);
        super::insert_newline(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "    nexedit\n    "
        );

        let expected_position = Position { line: 1, offset: 4 };
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().cursor.line,
            expected_position.line
        );
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().cursor.offset,
            expected_position.offset
        );
    }

    #[test]
    fn insert_newline_uses_nearest_line_indentation_when_current_line_blank() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        buffer.insert("    nexedit\n");
        let position = Position { line: 1, offset: 0 };
        buffer.cursor.move_to(position);

        app.workspace.add_buffer(buffer);
        super::insert_newline(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "    nexedit\n\n    "
        );

        let expected_position = Position { line: 2, offset: 4 };
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().cursor.line,
            expected_position.line
        );
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().cursor.offset,
            expected_position.offset
        );
    }

    #[test]
    fn change_rest_of_line_removes_content_and_switches_to_insert_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        buffer.insert("    nexedit");
        let position = Position { line: 0, offset: 4 };
        buffer.cursor.move_to(position);

        app.workspace.add_buffer(buffer);
        super::change_rest_of_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "    \neditor"
        );

        assert!(match app.mode {
            crate::models::application::Mode::Insert => true,
            _ => false,
        });

        app.workspace.current_buffer.as_mut().unwrap().insert(" ");
        app.workspace.current_buffer.as_mut().unwrap().undo();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "    nexedit"
        );
    }

    #[test]
    fn delete_token_deletes_current_token_and_trailing_whitespace() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        super::delete_token(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "editor"
        );
    }

    #[test]
    fn delete_token_does_not_delete_newline_characters() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        super::delete_token(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\neditor"
        );
    }

    #[test]
    fn delete_current_line_deletes_current_line() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        buffer.insert("    nexedit");
        let position = Position { line: 0, offset: 4 };
        buffer.cursor.move_to(position);

        app.workspace.add_buffer(buffer);
        super::delete_current_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "editor"
        );
    }

    #[test]
    fn indent_line_inserts_two_spaces_at_start_of_line() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 2 });

        app.workspace.add_buffer(buffer);
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn indent_line_works_in_select_line_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "  nexedit"
        );
    }

    #[test]
    fn indent_line_moves_cursor_in_insert_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 2 });

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_insert_mode(&mut app).unwrap();
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 4 }
        );
    }

    #[test]
    fn indent_line_does_not_move_cursor_in_normal_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 2 });

        app.workspace.add_buffer(buffer);
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 2 }
        );
    }

    #[test]
    fn indent_line_groups_multi_line_indents_as_a_single_operation() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "  nexedit"
        );

        super::undo(&mut app).unwrap();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn indent_line_works_with_reversed_selections() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::cursor::move_down(&mut app).unwrap();
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_up(&mut app).unwrap();
        super::indent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "  nexedit"
        );
    }

    #[test]
    fn outdent_line_removes_two_spaces_from_start_of_line() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 6 });

        app.workspace.add_buffer(buffer);
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 4 }
        );
    }

    #[test]
    fn outdent_line_removes_as_much_space_as_it_can_from_start_of_line_if_less_than_full_indent() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 2 });

        app.workspace.add_buffer(buffer);
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn outdent_does_nothing_if_there_is_no_leading_whitespace() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        buffer.insert("nexedit   ");

        app.workspace.add_buffer(buffer);
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit   "
        );
    }

    #[test]
    fn outdent_line_works_in_select_line_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("  nexedit");

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn outdent_line_groups_multi_line_indents_as_a_single_operation() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("  nexedit");

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );

        super::undo(&mut app).unwrap();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "  nexedit"
        );
    }

    #[test]
    fn outdent_line_works_with_reversed_selections() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("  nexedit");

        app.workspace.add_buffer(buffer);
        commands::cursor::move_down(&mut app).unwrap();
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::cursor::move_up(&mut app).unwrap();
        super::outdent_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn remove_trailing_whitespace_works() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("  nexedit\n  \neditor ");

        app.workspace.add_buffer(buffer);
        super::remove_trailing_whitespace(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "  nexedit\n\neditor"
        );
    }

    #[test]
    fn remove_trailing_whitespace_works_with_tab() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t\tnexedit\n\t\t\neditor\t");

        app.workspace.add_buffer(buffer);
        super::remove_trailing_whitespace(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\t\tnexedit\n\neditor"
        );
    }

    #[test]
    fn save_removes_trailing_whitespace_and_adds_newlines() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit  \neditor ");

        app.workspace.add_buffer(buffer);
        super::save(&mut app).ok();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n"
        );
    }

    #[test]
    fn save_adds_newline_with_unicode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit    \n∴ editor ");
        app.workspace.add_buffer(buffer);
        super::save(&mut app).ok();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n∴ editor\n"
        );
    }

    #[test]
    fn save_switches_to_path_mode_when_path_is_missing() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer = Buffer::new();

        app.workspace.add_buffer(buffer);
        super::save(&mut app).ok();

        if let Mode::Path(_) = app.mode {
        } else {
            panic!("Failed to switch to path mode");
        }
    }

    #[test]
    fn save_sets_save_on_accept_when_switching_to_path_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer = Buffer::new();

        app.workspace.add_buffer(buffer);
        super::save(&mut app).ok();

        if let Mode::Path(ref mode) = app.mode {
            assert!(mode.save_on_accept)
        } else {
            panic!("Failed to switch to path mode");
        }
    }

    #[test]
    fn save_does_not_run_format_tool_by_default() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let data = YamlLoader::load_from_str(
            "
            types:
              rs:
                format_tool:
                  command: tr
                  options: ['a', 'b']
        ",
        )
        .unwrap();
        let preferences = Preferences::new(data.into_iter().nth(0));
        app.preferences.replace(preferences);

        let path = format!("{}/format_tool.rs", env::temp_dir().display());
        let mut temp_file = File::create(&path).unwrap();
        write!(temp_file, "nexedit\n").unwrap();
        app.workspace.open_buffer(&Path::new(&path)).unwrap();

        super::save(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n"
        );
    }

    #[test]
    fn save_runs_format_command_when_configured() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let data = YamlLoader::load_from_str(
            "
            types:
              rs:
                format_tool:
                  command: tr
                  options: ['a', 'b']
                  run_on_save: true
        ",
        )
        .unwrap();
        let preferences = Preferences::new(data.into_iter().nth(0));
        app.preferences.replace(preferences);

        let path = format!("{}/format_tool.rs", env::temp_dir().display());
        let mut temp_file = File::create(&path).unwrap();
        write!(temp_file, "nexedit\n").unwrap();
        app.workspace.open_buffer(&Path::new(&path)).unwrap();

        super::save(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "bmp editor\n"
        );

        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .reload()
            .unwrap();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "bmp editor\n"
        );
    }

    #[test]
    fn paste_inserts_at_cursor_when_pasting_inline_data() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_mode(&mut app).unwrap();
        commands::cursor::move_right(&mut app).unwrap();
        commands::selection::copy(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "anexedit"
        );
    }

    #[test]
    fn paste_inserts_on_line_below_when_pasting_block_data() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 0, offset: 2 });

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::selection::copy(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\nnexedit"
        );
    }

    #[test]
    fn paste_works_at_end_of_buffer_when_pasting_block_data() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        buffer.cursor.move_to(Position { line: 0, offset: 0 });

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::selection::copy(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\nnexedit\n"
        );
    }

    #[test]
    fn paste_works_on_trailing_newline_when_pasting_block_data() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\n");
        buffer.cursor.move_to(Position { line: 0, offset: 0 });

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::selection::copy(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        commands::cursor::move_down(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\nnexedit\n"
        );
    }

    #[test]
    fn backspace_outdents_line_if_line_is_whitespace() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\n        ");
        buffer.cursor.move_to(Position { line: 2, offset: 8 });

        app.workspace.add_buffer(buffer);
        commands::buffer::backspace(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n      "
        );
    }

    #[test]
    fn merge_next_line_joins_current_and_next_lines_with_a_space() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::buffer::merge_next_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 0, offset: 3 }
        );
    }

    #[test]
    fn merge_next_line_does_nothing_if_there_is_no_next_line() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::buffer::merge_next_line(&mut app).ok();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );

        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 0, offset: 0 }
        );
    }

    #[test]
    fn merge_next_line_works_when_the_next_line_has_a_line_after_it() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\ntest");

        app.workspace.add_buffer(buffer);
        commands::buffer::merge_next_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\ntest"
        );
    }

    #[test]
    fn merge_next_line_works_when_the_first_line_has_leading_whitespace() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\n nexedit");
        buffer.cursor.move_to(Position { line: 1, offset: 0 });

        app.workspace.add_buffer(buffer);
        commands::buffer::merge_next_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\n nexedit"
        );
    }

    #[test]
    fn merge_next_line_removes_leading_whitespace_from_second_line() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::buffer::merge_next_line(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
    }

    #[test]
    fn ensure_trailing_newline_adds_newlines_when_missing() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");

        app.workspace.add_buffer(buffer);
        commands::buffer::ensure_trailing_newline(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n"
        );
    }

    #[test]
    fn ensure_trailing_newline_does_nothing_when_already_present() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\n");

        app.workspace.add_buffer(buffer);
        commands::buffer::ensure_trailing_newline(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit\n"
        );
    }

    #[test]
    fn paste_with_inline_content_replaces_selection() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        app.clipboard
            .set_content(ClipboardContent::Inline("editor".to_string()))
            .unwrap();

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_mode(&mut app).unwrap();
        commands::cursor::move_to_end_of_line(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "editor"
        );

    }

    #[test]
    fn paste_with_block_content_replaces_selection() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        app.clipboard
            .set_content(ClipboardContent::Block("paste nexedit\n".to_string()))
            .unwrap();

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        commands::buffer::paste(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "paste nexedit"
        );

    }

    #[test]
    fn paste_above_inserts_clipboard_contents_on_a_new_line_above() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        let original_position = Position { line: 0, offset: 3 };
        buffer.insert("editor");
        buffer.cursor.move_to(original_position.clone());
        app.clipboard
            .set_content(ClipboardContent::Block("nexedit\n".to_string()))
            .unwrap();

        app.workspace.add_buffer(buffer);
        commands::buffer::paste_above(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "nexedit"
        );
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            original_position
        );
    }

    #[test]
    fn close_displays_confirmation_when_buffer_is_modified() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("data");

        app.workspace.add_buffer(buffer);
        commands::buffer::close(&mut app).unwrap();

        if let Mode::Confirm(_) = app.mode {
        } else {
            panic!("Not in confirm mode");
        }
    }

    #[test]
    fn close_skips_confirmation_when_buffer_is_empty() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer = Buffer::new();

        app.workspace.close_current_buffer();

        app.workspace.add_buffer(buffer);
        commands::buffer::close(&mut app).unwrap();

        assert!(app.workspace.current_buffer.as_ref().is_none());
    }

    #[test]
    fn close_skips_confirmation_when_buffer_is_unmodified() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer = Buffer::from_file(Path::new("LICENSE")).unwrap();

        app.workspace.close_current_buffer();

        app.workspace.add_buffer(buffer);
        commands::buffer::close(&mut app).unwrap();

        assert!(app.workspace.current_buffer.as_ref().is_none());
    }

    #[test]
    fn close_others_skips_confirmation_when_all_other_buffers_are_empty_or_unmodified() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer_1 = Buffer::new();
        let buffer_2 = Buffer::from_file(Path::new("LICENSE")).unwrap();
        let mut buffer_3 = Buffer::new();
        buffer_3.insert("three");

        app.workspace.close_current_buffer();

        app.workspace.add_buffer(buffer_1);
        app.workspace.add_buffer(buffer_2);
        app.workspace.add_buffer(buffer_3);
        commands::buffer::close_others(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "three"
        );
        app.workspace.next_buffer();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "three"
        );
    }

    #[test]
    fn close_others_displays_confirmation_before_closing_modified_buffer() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let buffer = Buffer::new();
        let mut modified_buffer = Buffer::new();
        modified_buffer.insert("data");

        app.workspace.close_current_buffer();

        app.workspace.add_buffer(modified_buffer);
        app.workspace.add_buffer(buffer);
        commands::buffer::close_others(&mut app).unwrap();

        if let Mode::Confirm(_) = app.mode {
        } else {
            panic!("Not in confirm mode");
        }

        commands::confirm::confirm_command(&mut app).unwrap();

        assert_eq!(app.workspace.current_buffer.as_ref().unwrap().data(), "");
        app.workspace.next_buffer();
        assert_eq!(app.workspace.current_buffer.as_ref().unwrap().data(), "");
    }

    #[test]
    fn close_others_works_when_current_buffer_is_last() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer_1 = Buffer::new();
        let mut buffer_2 = Buffer::new();
        let mut buffer_3 = Buffer::new();
        buffer_1.insert(""); // Empty to prevent close confirmation.
        buffer_2.insert(""); // Empty to prevent close confirmation.
        buffer_3.insert("three");

        app.workspace.add_buffer(buffer_1);
        app.workspace.add_buffer(buffer_2);
        app.workspace.add_buffer(buffer_3);

        commands::buffer::close_others(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "three"
        );
        app.workspace.next_buffer();
        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "three"
        );
    }

    #[test]
    fn close_others_works_when_current_buffer_is_not_last() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer_1 = Buffer::new();
        let mut buffer_2 = Buffer::new();
        let mut buffer_3 = Buffer::new();
        buffer_1.insert(""); // Empty to prevent close confirmation.
        buffer_2.insert("two");
        buffer_3.insert(""); // Empty to prevent close confirmation.

        app.workspace.add_buffer(buffer_1);
        app.workspace.add_buffer(buffer_2);
        app.workspace.add_buffer(buffer_3);
        app.workspace.previous_buffer();
        commands::buffer::close_others(&mut app).unwrap();

        assert_eq!(app.workspace.current_buffer.as_ref().unwrap().data(), "two");
        app.workspace.next_buffer();
        assert_eq!(app.workspace.current_buffer.as_ref().unwrap().data(), "two");
    }

    #[test]
    fn toggle_line_comment_add_single_in_normal_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\tnexedit\n\teditor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 1 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\t// nexedit\n\teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 0, offset: 1 }
        );
    }

    #[test]
    fn toggle_line_comment_add_multiple_in_select_line_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\tnexedit\n\t\teditor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 1 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 1 });

        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\t// nexedit\n\t// \teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 1, offset: 1 }
        );
    }

    #[test]
    fn toggle_line_comment_remove_single_in_normal_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t// nexedit\n\teditor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 1 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\tnexedit\n\teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 0, offset: 1 }
        );
    }

    #[test]
    fn toggle_line_comment_remove_multiple_in_select_line_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t// nexedit\n\t// \teditor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 1 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 1 });

        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\tnexedit\n\t\teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 1, offset: 1 }
        );
    }

    #[test]
    fn toggle_line_comment_remove_multiple_with_unequal_indent_in_select_line_mode() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t// nexedit\n\t\t// editor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 1 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 1 });

        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\tnexedit\n\t\teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 1, offset: 1 }
        );
    }

    #[test]
    fn toggle_line_comment_add_correctly_preserves_empty_lines() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\tnexedit\n\n\teditor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 0 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 2, offset: 0 });

        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\t// nexedit\n\n\t// editor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 2, offset: 0 }
        );
    }

    #[test]
    fn toggle_line_comment_remove_correctly_preserves_empty_lines() {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t// nexedit\n\n\t// editor\n");
        buffer.cursor.move_to(Position { line: 0, offset: 0 });
        buffer.path = Some("test.rs".into());

        app.workspace.add_buffer(buffer);
        commands::application::switch_to_select_line_mode(&mut app).unwrap();
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 2, offset: 0 });

        super::toggle_line_comment(&mut app).unwrap();

        assert_eq!(
            app.workspace.current_buffer.as_ref().unwrap().data(),
            "\tnexedit\n\n\teditor\n"
        );
        assert_eq!(
            app.workspace
                .current_buffer
                .as_ref()
                .unwrap()
                .cursor
                .position,
            Position { line: 2, offset: 0 }
        );
    }
}
