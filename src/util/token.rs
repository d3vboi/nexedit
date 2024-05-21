use crate::util::movement_lexer;
use luthor::token::Category;
use scribe::buffer::{Buffer, Position};

#[derive(Clone, Copy, PartialEq)]
pub enum Direction {
    Forward,
    Backward,
}

pub fn adjacent_token_position(
    buffer: &Buffer,
    whitespace: bool,
    direction: Direction,
) -> Option<Position> {
    let mut line = 0;
    let mut offset = 0;
    let mut previous_position = Position { line: 0, offset: 0 };
    let tokens = movement_lexer::lex(&buffer.data());
    for token in tokens {
        let position = Position { line, offset };
        if position > *buffer.cursor && direction == Direction::Forward {
            if whitespace {
                return Some(position);
            } else {
                match token.category {
                    Category::Whitespace => (),
                    _ => {
                        return Some(position);
                    }
                }
            }
        }

        match token.lexeme.split('\n').count() {
            1 => {
                offset += token.lexeme.len()
            }
            n => {
                line += n - 1;
                offset = token.lexeme.split('\n').last().unwrap().len();
            }
        };

        let next_position = Position { line, offset };
        if next_position >= *buffer.cursor && direction == Direction::Backward {
            match token.category {
                Category::Whitespace => {
                    return Some(previous_position);
                }
                _ => {
                    return Some(position);
                }
            }
        }

        previous_position = position;
    }

    None
}
