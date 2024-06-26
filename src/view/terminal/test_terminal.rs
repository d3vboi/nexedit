use super::Terminal;
use crate::errors::*;
use crate::input::Key;
use crate::models::application::Event;
use crate::view::{Colors, CursorType, Style};
use scribe::buffer::Position;
use std::sync::Mutex;

const WIDTH: usize = 10;
const HEIGHT: usize = 10;

pub struct TestTerminal {
    data: Mutex<[[Option<(char, Colors)>; WIDTH]; HEIGHT]>, // 2D array of chars to represent screen
    cursor: Mutex<Option<Position>>,
    key_sent: Mutex<bool>,
}

impl TestTerminal {
    pub fn new() -> TestTerminal {
        TestTerminal {
            data: Mutex::new([[None; WIDTH]; HEIGHT]),
            cursor: Mutex::new(None),
            key_sent: Mutex::new(false),
        }
    }

    pub fn content(&self) -> String {
        let mut data = String::new();
        let mut last_row_with_data = 0;
        let mut last_column_with_data = 0;

        for (y, row) in self.data.lock().unwrap().iter().enumerate() {
            for (x, cell) in row.iter().enumerate() {
                if let Some((c, _)) = *cell {
                    for _ in last_row_with_data..y {
                        data.push('\n');
                        last_column_with_data = 0;
                    }

                    for _ in last_column_with_data..x {
                        data.push(' ');
                    }

                    data.push(c);

                    last_row_with_data = y;

                    last_column_with_data = x + 1;
                }
            }
        }

        data
    }
}

impl Terminal for TestTerminal {
    fn listen(&self) -> Option<Event> {
        let mut key_sent = self.key_sent.lock().unwrap();
        if *key_sent {
            None
        } else {
            *key_sent = true;
            Some(Event::Key(Key::Char('A')))
        }
    }
    fn clear(&self) {
        for row in self.data.lock().unwrap().iter_mut() {
            *row = [None; WIDTH];
        }
    }
    fn present(&self) {}
    fn width(&self) -> usize {
        WIDTH
    }
    fn height(&self) -> usize {
        HEIGHT
    }
    fn set_cursor(&self, position: Option<Position>) {
        let mut cursor = self.cursor.lock().unwrap();
        *cursor = position;
    }
    fn set_cursor_type(&self, _: CursorType) {}
    fn suspend(&self) {}
    fn print(&self, position: &Position, _: Style, colors: Colors, content: &str) -> Result<()> {
        if position.line >= self.height() {
            return Ok(());
        }

        let mut data = self.data.lock().unwrap();
        let string_content = format!("{}", content);

        for (i, c) in string_content.chars().enumerate() {
            if i + position.offset >= WIDTH {
                break;
            }

            data[position.line][i + position.offset] = Some((c, colors));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::TestTerminal;
    use crate::view::terminal::Terminal;
    use crate::view::{Colors, Style};
    use scribe::buffer::Position;

    #[test]
    fn print_sets_terminal_data_correctly() {
        let terminal = Box::new(TestTerminal::new());
        terminal
            .print(
                &Position { line: 0, offset: 0 },
                Style::Default,
                Colors::Default,
                &"data",
            )
            .unwrap();

        assert_eq!(terminal.content(), "data");
    }

    #[test]
    fn data_uses_newlines_and_spaces_to_represent_structure() {
        let terminal = Box::new(TestTerminal::new());

        terminal
            .print(
                &Position { line: 0, offset: 2 },
                Style::Default,
                Colors::Default,
                &"some",
            )
            .unwrap();
        terminal
            .print(
                &Position { line: 2, offset: 5 },
                Style::Default,
                Colors::Default,
                &"data",
            )
            .unwrap();

        assert_eq!(terminal.content(), "  some\n\n     data");
    }
}
