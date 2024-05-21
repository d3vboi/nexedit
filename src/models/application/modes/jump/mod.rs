mod single_character_tag_generator;
mod tag_generator;

use self::single_character_tag_generator::SingleCharacterTagGenerator;
use self::tag_generator::TagGenerator;
use crate::models::application::modes::select::SelectMode;
use crate::models::application::modes::select_line::SelectLineMode;
use crate::util::movement_lexer;
use crate::view::{LexemeMapper, MappedLexeme};
use luthor::token::Category;
use scribe::buffer::{Distance, Position};
use std::collections::HashMap;

pub enum SelectModeOptions {
    None,
    Select(SelectMode),
    SelectLine(SelectLineMode),
}

enum MappedLexemeValue {
    Tag((String, Position)),
    Text((String, Position)),
}

pub struct JumpMode {
    pub input: String,
    pub first_phase: bool,
    cursor_line: usize,
    pub select_mode: SelectModeOptions,
    tag_positions: HashMap<String, Position>,
    tag_generator: TagGenerator,
    single_characters: SingleCharacterTagGenerator,
    current_position: Position,
    mapped_lexeme_values: Vec<MappedLexemeValue>,
}

impl JumpMode {
    pub fn new(cursor_line: usize) -> JumpMode {
        JumpMode {
            input: String::new(),
            first_phase: true,
            cursor_line,
            select_mode: SelectModeOptions::None,
            tag_positions: HashMap::new(),
            tag_generator: TagGenerator::new(),
            single_characters: SingleCharacterTagGenerator::new(),
            current_position: Position { line: 0, offset: 0 },
            mapped_lexeme_values: Vec::new(),
        }
    }

    pub fn map_tag(&self, tag: &str) -> Option<&Position> {
        self.tag_positions.get(tag)
    }

    pub fn reset_display(&mut self) {
        self.tag_positions.clear();
        self.tag_generator.reset();
        self.single_characters.reset();
    }
}

impl LexemeMapper for JumpMode {
    fn map<'a>(&'a mut self, lexeme: &str, position: Position) -> Vec<MappedLexeme<'a>> {
        self.mapped_lexeme_values = Vec::new();
        self.current_position = position;

        for subtoken in movement_lexer::lex(lexeme) {
            if subtoken.category == Category::Whitespace {
                let distance = Distance::of_str(&subtoken.lexeme);

                self.mapped_lexeme_values.push(MappedLexemeValue::Text((
                    subtoken.lexeme,
                    self.current_position,
                )));

                self.current_position += distance;
            } else {
                let tag = if self.first_phase {
                    if self.current_position.line >= self.cursor_line {
                        self.single_characters.next()
                    } else {
                        None // We haven't reached the cursor yet.
                    }
                } else if subtoken.lexeme.len() > 1 {
                    self.tag_generator.next()
                } else {
                    None
                };

                match tag {
                    Some(tag) => {
                        let tag_len = tag.len();

                        self.mapped_lexeme_values
                            .push(MappedLexemeValue::Tag((tag.clone(), self.current_position)));

                        self.tag_positions.insert(tag, self.current_position);

                        self.current_position += Distance {
                            lines: 0,
                            offset: tag_len,
                        };

                        let suffix: String = subtoken.lexeme.chars().skip(tag_len).collect();
                        let suffix_len = suffix.len();

                        if suffix_len > 0 {
                            self.mapped_lexeme_values
                                .push(MappedLexemeValue::Text((suffix, self.current_position)));

                            self.current_position += Distance {
                                lines: 0,
                                offset: suffix_len,
                            };
                        }
                    }
                    None => {
                        let distance = Distance::of_str(&subtoken.lexeme);

                        self.mapped_lexeme_values.push(MappedLexemeValue::Text((
                            subtoken.lexeme,
                            self.current_position,
                        )));

                        self.current_position += distance;
                    }
                }
            }
        }

        self.mapped_lexeme_values
            .iter()
            .map(|mapped_lexeme| match *mapped_lexeme {
                MappedLexemeValue::Tag((ref lexeme, _)) => MappedLexeme::Focused(lexeme.as_str()),
                MappedLexemeValue::Text((ref lexeme, _)) => MappedLexeme::Blurred(lexeme.as_str()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::JumpMode;
    use crate::view::{LexemeMapper, MappedLexeme};
    use scribe::buffer::Position;

    #[test]
    fn map_returns_the_correct_lexemes_in_first_phase() {
        let mut jump_mode = JumpMode::new(0);

        assert_eq!(
            jump_mode.map("nexedit", Position { line: 0, offset: 0 }),
            vec![MappedLexeme::Focused("a"), MappedLexeme::Blurred("mp")]
        );

        assert_eq!(
            jump_mode.map("editor", Position { line: 0, offset: 3 }),
            vec![MappedLexeme::Focused("b"), MappedLexeme::Blurred("ditor")]
        );
    }

    #[test]
    fn map_returns_the_correct_lexemes_in_second_phase() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        assert_eq!(
            jump_mode.map("nexedit", Position { line: 0, offset: 0 }),
            vec![MappedLexeme::Focused("aa"), MappedLexeme::Blurred("p")]
        );

        assert_eq!(
            jump_mode.map("editor", Position { line: 0, offset: 3 }),
            vec![MappedLexeme::Focused("ab"), MappedLexeme::Blurred("itor")]
        );
    }

    #[test]
    fn map_splits_passed_tokens_on_whitespace() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        assert_eq!(
            jump_mode.map("do a test", Position { line: 0, offset: 0 }),
            vec![
                MappedLexeme::Focused("aa"),
                MappedLexeme::Blurred(" "),
                MappedLexeme::Blurred("a"),
                MappedLexeme::Blurred(" "),
                MappedLexeme::Focused("ab"),
                MappedLexeme::Blurred("st")
            ]
        )
    }

    #[test]
    fn map_tracks_the_positions_of_each_jump_token() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        jump_mode.map("  nexedit", Position { line: 0, offset: 0 });
        jump_mode.map("editor", Position { line: 0, offset: 5 });

        assert_eq!(
            *jump_mode.tag_positions.get("aa").unwrap(),
            Position { line: 0, offset: 2 }
        );
        assert_eq!(
            *jump_mode.tag_positions.get("ab").unwrap(),
            Position { line: 0, offset: 5 }
        );
    }

    #[test]
    fn reset_display_restarts_single_character_token_generator() {
        let mut jump_mode = JumpMode::new(0);

        assert_eq!(
            jump_mode.map("nexedit", Position { line: 0, offset: 0 }),
            vec![MappedLexeme::Focused("a"), MappedLexeme::Blurred("mp")]
        );
        jump_mode.reset_display();

        assert_eq!(
            jump_mode.map("editor", Position { line: 0, offset: 3 }),
            vec![MappedLexeme::Focused("a"), MappedLexeme::Blurred("ditor")]
        );
    }

    #[test]
    fn reset_display_restarts_double_character_token_generator() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        assert_eq!(
            jump_mode.map("nexedit", Position { line: 0, offset: 0 }),
            vec![MappedLexeme::Focused("aa"), MappedLexeme::Blurred("p")]
        );
        jump_mode.reset_display();

        assert_eq!(
            jump_mode.map("editor", Position { line: 0, offset: 3 }),
            vec![MappedLexeme::Focused("aa"), MappedLexeme::Blurred("itor")]
        );
    }

    #[test]
    fn map_can_handle_unicode_data() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        assert_eq!(
            jump_mode.map("eéditor", Position { line: 0, offset: 0 }),
            vec![MappedLexeme::Focused("aa"), MappedLexeme::Blurred("ditor")]
        );
    }

    #[test]
    fn map_tag_returns_position_when_available() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        jump_mode.map("nexedit", Position { line: 0, offset: 0 });
        jump_mode.map("editor", Position { line: 1, offset: 3 });
        assert_eq!(
            jump_mode.map_tag("ab"),
            Some(&Position { line: 1, offset: 3 })
        );
        assert_eq!(jump_mode.map_tag("none"), None);
    }

    #[test]
    fn map_splits_tokens_correctly_using_movement_lexer() {
        let mut jump_mode = JumpMode::new(0);
        jump_mode.first_phase = false;

        assert_eq!(
            jump_mode.map("nexedit_editor", Position { line: 0, offset: 0 }),
            vec![
                MappedLexeme::Focused("aa"),
                MappedLexeme::Blurred("p"),
                MappedLexeme::Blurred("_"),
                MappedLexeme::Focused("ab"),
                MappedLexeme::Blurred("itor")
            ]
        );
    }
}
