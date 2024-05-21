pub use self::selectable_vec::SelectableVec;

pub mod movement_lexer;
pub mod reflow;
mod selectable_vec;
pub mod token;

use crate::errors::*;
use crate::models::Application;
use scribe::buffer::{Buffer, LineRange, Position, Range};

pub fn inclusive_range(line_range: &LineRange, buffer: &mut Buffer) -> Range {
    let data = buffer.data();
    let next_line = line_range.end() + 1;
    let line_count = buffer.line_count();
    let end_position = if line_count > next_line {
        Position {
            line: next_line,
            offset: 0,
        }
    } else {
        match data.lines().nth(line_range.end()) {
            Some(line_content) => {
                Position {
                    line: line_range.end(),
                    offset: line_content.len(),
                }
            }
            None => Position {
                line: line_range.end(),
                offset: 0,
            },
        }
    };

    Range::new(
        Position {
            line: line_range.start(),
            offset: 0,
        },
        end_position,
    )
}

pub fn add_buffer(buffer: Buffer, app: &mut Application) -> Result<()> {
    app.workspace.add_buffer(buffer);
    app.view
        .initialize_buffer(app.workspace.current_buffer.as_mut().unwrap())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use scribe::buffer::{LineRange, Position, Range};
    use scribe::Buffer;

    #[test]
    fn inclusive_range_works_correctly_without_trailing_newline() {
        let mut buffer = Buffer::new();
        buffer.insert("nexedit");
        let range = LineRange::new(1, 1);

        assert_eq!(
            super::inclusive_range(&range, &mut buffer),
            Range::new(
                Position { line: 1, offset: 0 },
                Position { line: 1, offset: 6 }
            )
        );
    }

    #[test]
    fn inclusive_range_works_correctly_with_trailing_newline() {
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\n");
        let range = LineRange::new(1, 1);

        assert_eq!(
            super::inclusive_range(&range, &mut buffer),
            Range::new(
                Position { line: 1, offset: 0 },
                Position { line: 2, offset: 0 }
            )
        );
    }
}
