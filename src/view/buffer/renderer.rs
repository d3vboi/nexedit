use crate::errors::*;
use crate::models::application::Preferences;
use crate::view::buffer::line_numbers::*;
use crate::view::buffer::{LexemeMapper, MappedLexeme, RenderState};
use crate::view::color::to_rgb_color;
use crate::view::terminal::{Cell, Terminal, TerminalBuffer};
use crate::view::{Colors, RGBColor, Style, RENDER_CACHE_FREQUENCY};
use scribe::buffer::{Buffer, Position, Range};
use scribe::util::LineIterator;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;
use syntect::highlighting::Style as ThemeStyle;
use syntect::highlighting::{HighlightIterator, Highlighter, Theme};
use syntect::parsing::{ScopeStack, SyntaxSet};
use unicode_segmentation::UnicodeSegmentation;

pub struct BufferRenderer<'a, 'p> {
    buffer: &'a Buffer,
    buffer_position: Position,
    cursor_position: Option<Position>,
    gutter_width: usize,
    highlights: Option<&'a [Range]>,
    stylist: Highlighter<'a>,
    current_style: ThemeStyle,
    line_numbers: LineNumbers,
    preferences: &'a Preferences,
    render_cache: &'a Rc<RefCell<HashMap<usize, RenderState>>>,
    screen_position: Position,
    scroll_offset: usize,
    syntax_set: &'a SyntaxSet,
    terminal: &'a dyn Terminal,
    terminal_buffer: &'a mut TerminalBuffer<'p>,
    theme: &'a Theme,
}

impl<'a, 'p> BufferRenderer<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        buffer: &'a Buffer,
        highlights: Option<&'a [Range]>,
        scroll_offset: usize,
        terminal: &'a dyn Terminal,
        theme: &'a Theme,
        preferences: &'a Preferences,
        render_cache: &'a Rc<RefCell<HashMap<usize, RenderState>>>,
        syntax_set: &'a SyntaxSet,
        terminal_buffer: &'a mut TerminalBuffer<'p>,
    ) -> BufferRenderer<'a, 'p> {
        let line_numbers = LineNumbers::new(buffer, Some(scroll_offset));
        let gutter_width = line_numbers.width() + 1;

        let stylist = Highlighter::new(theme);
        let current_style = stylist.get_default();

        BufferRenderer {
            buffer,
            cursor_position: None,
            gutter_width,
            highlights,
            stylist,
            current_style,
            line_numbers,
            buffer_position: Position { line: 0, offset: 0 },
            preferences,
            render_cache,
            screen_position: Position { line: 0, offset: 0 },
            scroll_offset,
            syntax_set,
            terminal,
            terminal_buffer,
            theme,
        }
    }

    fn on_cursor_line(&self) -> bool {
        self.buffer_position.line == self.buffer.cursor.line
    }

    fn print_rest_of_line(&mut self) {
        let on_cursor_line = self.on_cursor_line();
        let guide_offsets = self.length_guide_offsets();

        for offset in self.screen_position.offset..self.terminal.width() {
            let colors = if on_cursor_line || guide_offsets.contains(&offset) {
                Colors::Focused
            } else {
                Colors::Default
            };

            self.print(
                Position {
                    line: self.screen_position.line,
                    offset,
                },
                Style::Default,
                colors,
                " ",
            );
        }
    }

    fn length_guide_offsets(&self) -> Vec<usize> {
        self.preferences
            .line_length_guides()
            .into_iter()
            .map(|offset| self.gutter_width + offset)
            .collect()
    }

    fn advance_to_next_line(&mut self) {
        if self.inside_visible_content() {
            self.set_cursor();
            self.print_rest_of_line();

            self.screen_position.line += 1;
        }

        self.buffer_position.line += 1;
        self.buffer_position.offset = 0;

        self.print_line_number();
    }

    fn set_cursor(&mut self) {
        if self.inside_visible_content() && *self.buffer.cursor == self.buffer_position {
            self.cursor_position = Some(self.screen_position);
        }
    }

    fn current_char_style(&self, token_color: RGBColor) -> (Style, Colors) {
        let (style, colors) = match self.highlights {
            Some(highlight_ranges) => {
                for range in highlight_ranges {
                    if range.includes(&self.buffer_position) {
                        if range.includes(&self.buffer.cursor) {
                            return (Style::Bold, Colors::SelectMode);
                        } else {
                            return (Style::Inverted, Colors::Default);
                        }
                    }
                }

                if self.on_cursor_line() {
                    (Style::Default, Colors::CustomFocusedForeground(token_color))
                } else {
                    (Style::Default, Colors::CustomForeground(token_color))
                }
            }
            None => {
                if self.on_cursor_line() {
                    (Style::Default, Colors::CustomFocusedForeground(token_color))
                } else {
                    (Style::Default, Colors::CustomForeground(token_color))
                }
            }
        };

        (style, colors)
    }

    fn print_lexeme<L: Into<Cow<'p, str>>>(&mut self, lexeme: L) {
        for character in lexeme.into().graphemes(true) {
            if character == "\n" {
                continue;
            }

            self.set_cursor();

            let token_color = to_rgb_color(self.current_style.foreground);
            let (style, color) = self.current_char_style(token_color);

            if self.preferences.line_wrapping()
                && self.screen_position.offset == self.terminal.width()
            {
                self.screen_position.line += 1;
                self.screen_position.offset = self.gutter_width;
                self.print(self.screen_position, style, color, character.to_string());
                self.screen_position.offset += 1;
                self.buffer_position.offset += 1;
            } else if character == "\t" {
                let buffer_tab_stop =
                    self.next_tab_stop(self.screen_position.offset - self.gutter_width);
                let mut screen_tab_stop = buffer_tab_stop + self.gutter_width;

                if screen_tab_stop > self.terminal.width() {
                    screen_tab_stop = self.terminal.width();
                }

                for _ in self.screen_position.offset..screen_tab_stop {
                    self.print(self.screen_position, style, color, " ");
                    self.screen_position.offset += 1;
                }
                self.buffer_position.offset += 1;
            } else {
                self.print(self.screen_position, style, color, character.to_string());
                self.screen_position.offset += 1;
                self.buffer_position.offset += 1;
            }

            self.set_cursor();
        }
    }

    fn before_visible_content(&mut self) -> bool {
        self.buffer_position.line < self.scroll_offset
    }

    fn after_visible_content(&self) -> bool {
        self.screen_position.line >= (self.terminal.height() - 1)
    }

    fn inside_visible_content(&mut self) -> bool {
        !self.before_visible_content() && !self.after_visible_content()
    }

    pub fn render(
        &mut self,
        lines: LineIterator<'p>,
        mut lexeme_mapper: Option<&mut dyn LexemeMapper>,
    ) -> Result<Option<Position>> {
        self.terminal.set_cursor(None);
        self.print_line_number();

        let highlighter = Highlighter::new(self.theme);
        let syntax_definition = self
            .buffer
            .syntax_definition
            .as_ref()
            .ok_or("Buffer has no syntax definition")?;

        let (cached_line_no, mut state) = self
            .cached_render_state()
            .unwrap_or((0, RenderState::new(&highlighter, syntax_definition)));
        let (focused_style, blurred_style) = self.mapper_styles();

        'print: for (line_no, line) in lines {
            if line_no >= cached_line_no {
                if line_no % RENDER_CACHE_FREQUENCY == 0 && line_no > 0 {
                    self.render_cache
                        .borrow_mut()
                        .insert(line_no, state.clone());
                }

                let events = state
                    .parse
                    .parse_line(line, self.syntax_set)
                    .chain_err(|| BUFFER_PARSE_FAILED)?;
                let styled_lexemes =
                    HighlightIterator::new(&mut state.highlight, &events, line, &highlighter);

                for (style, lexeme) in styled_lexemes {
                    if self.before_visible_content() {
                        continue;
                    }

                    if self.after_visible_content() {
                        break 'print;
                    }

                    if let Some(ref mut mapper) = lexeme_mapper {
                        let mapped_lexemes = mapper.map(lexeme, self.buffer_position);
                        for mapped_lexeme in mapped_lexemes {
                            match mapped_lexeme {
                                MappedLexeme::Focused(value) => {
                                    self.current_style = focused_style;
                                    self.print_lexeme(value.to_string());
                                }
                                MappedLexeme::Blurred(value) => {
                                    self.current_style = blurred_style;
                                    self.print_lexeme(value.to_string());
                                }
                            }
                        }
                    } else {
                        self.current_style = style;
                        self.print_lexeme(lexeme);
                    }
                }
            }

            if has_trailing_newline(line) {
                self.advance_to_next_line();
            }
        }

        self.set_cursor();

        self.print_rest_of_line();

        Ok(self.cursor_position)
    }

    fn print_line_number(&mut self) {
        if !self.inside_visible_content() {
            return;
        };

        let line_number = self.line_numbers.next().unwrap();

        let weight = if self.on_cursor_line() {
            Style::Bold
        } else {
            Style::Default
        };

        self.print(
            Position {
                line: self.screen_position.line,
                offset: 0,
            },
            weight,
            Colors::Focused,
            line_number,
        );

        let gap_color = if self.on_cursor_line() {
            Colors::Focused
        } else {
            Colors::Default
        };
        self.print(
            Position {
                line: self.screen_position.line,
                offset: self.line_numbers.width(),
            },
            weight,
            gap_color,
            " ",
        );

        self.screen_position.offset = self.line_numbers.width() + 1;
    }

    fn next_tab_stop(&self, offset: usize) -> usize {
        (offset / self.preferences.tab_width(self.buffer.path.as_ref()) + 1)
            * self.preferences.tab_width(self.buffer.path.as_ref())
    }

    fn mapper_styles(&self) -> (ThemeStyle, ThemeStyle) {
        let focused_style = self.stylist.style_for_stack(
            ScopeStack::from_str("keyword")
                .unwrap_or_default()
                .as_slice(),
        );
        let blurred_style = self.stylist.style_for_stack(
            ScopeStack::from_str("comment")
                .unwrap_or_default()
                .as_slice(),
        );

        (focused_style, blurred_style)
    }

    fn cached_render_state(&self) -> Option<(usize, RenderState)> {
        self.render_cache
            .borrow()
            .iter()
            .filter(|(k, _)| **k < self.scroll_offset)
            .max_by(|(k1, _), (k2, _)| k1.cmp(k2))
            .map(|(k, v)| (*k, v.clone()))
    }

    fn print<C>(&mut self, position: Position, style: Style, colors: Colors, content: C)
    where
        C: Into<Cow<'p, str>>,
    {
        self.terminal_buffer.set_cell(
            position,
            Cell {
                content: content.into(),
                style,
                colors,
            },
        );
    }
}

fn has_trailing_newline(line: &str) -> bool {
    line.chars().last().map(|c| c == '\n').unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::{BufferRenderer, LexemeMapper, MappedLexeme};
    use crate::models::application::Preferences;
    use crate::view::terminal::*;
    use scribe::buffer::Position;
    use scribe::util::LineIterator;
    use scribe::{Buffer, Workspace};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use syntect::highlighting::ThemeSet;
    use yaml_rust::yaml::YamlLoader;

    #[test]
    fn tabs_beyond_terminal_width_dont_panic() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t\t\t");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let data = YamlLoader::load_from_str("tab_width: 100")
            .unwrap()
            .into_iter()
            .nth(0)
            .unwrap();
        let preferences = Preferences::new(Some(data));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();
    }

    #[test]
    fn aligned_tabs_expand_to_correct_number_of_spaces() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t\txy");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let data = YamlLoader::load_from_str("tab_width: 2")
            .unwrap()
            .into_iter()
            .nth(0)
            .unwrap();
        let preferences = Preferences::new(Some(data));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        let expected_content = " 1      xy";
        assert_eq!(
            &terminal_buffer.content()[0..expected_content.len()],
            expected_content
        );
    }

    #[test]
    fn unaligned_tabs_expand_to_correct_number_of_spaces() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\t \txy");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let data = YamlLoader::load_from_str("tab_width: 2")
            .unwrap()
            .into_iter()
            .nth(0)
            .unwrap();
        let preferences = Preferences::new(Some(data));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        let expected_content = " 1      xy";
        assert_eq!(
            &terminal_buffer.content()[0..expected_content.len()],
            expected_content
        );
    }

    #[test]
    fn render_wraps_lines_correctly() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("nexedit\nsecond line\n");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        let expected_content = " 1  nexedit  \n 2  second\n     line \n 3        ";
        assert_eq!(
            &terminal_buffer.content()[0..expected_content.len()],
            expected_content
        );
    }

    struct TestMapper {}
    impl LexemeMapper for TestMapper {
        fn map<'a, 'b>(&'a mut self, _: &str, _: Position) -> Vec<MappedLexeme<'a>> {
            vec![MappedLexeme::Focused("mapped")]
        }
    }

    #[test]
    fn render_uses_lexeme_mapper() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("original");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, Some(&mut TestMapper {}))
        .unwrap();

        let expected_content = " 1  mapped";
        assert_eq!(
            &terminal_buffer.content()[0..expected_content.len()],
            expected_content
        );
    }

    #[test]
    fn render_returns_cursor_position_when_at_the_start_of_an_empty_line() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.insert("\n");
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);

        let cursor_position = BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            0,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &Rc::new(RefCell::new(HashMap::new())),
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        assert_eq!(cursor_position, Some(Position { line: 0, offset: 4 }));
    }

    #[test]
    fn render_caches_state_using_correct_frequency_excluding_first_line() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();

        for _ in 0..500 {
            buffer.insert("line\n");
        }
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);
        let render_cache = Rc::new(RefCell::new(HashMap::new()));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            495,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &render_cache,
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        assert_eq!(render_cache.borrow().keys().count(), 5);
    }

    #[test]
    fn render_uses_cached_state() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.path = Some(PathBuf::from("test.rs"));

        for _ in 0..500 {
            buffer.insert("line\n");
        }
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);
        let render_cache = Rc::new(RefCell::new(HashMap::new()));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            95,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &render_cache,
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        assert_eq!(render_cache.borrow().keys().count(), 1);
        let initial_cache = render_cache.borrow().values().nth(0).unwrap().clone();

        workspace.current_buffer.as_mut().unwrap().insert("\"");

        let data2 = workspace.current_buffer.as_ref().unwrap().data();
        let lines2 = LineIterator::new(&data2);

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            495,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &render_cache,
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines2, None)
        .unwrap();

        assert_eq!(render_cache.borrow().keys().count(), 5);
        for value in render_cache.borrow().values() {
            assert_eq!(value, &initial_cache);
        }
    }

    #[test]
    fn render_skips_lines_correctly_when_using_cached_state() {
        let mut workspace = Workspace::new(Path::new("."), None).unwrap();
        let mut buffer = Buffer::new();
        buffer.path = Some(PathBuf::from("test.rs"));

        for _ in 0..203 {
            buffer.insert("line\n");
        }
        workspace.add_buffer(buffer);

        let data = workspace.current_buffer.as_ref().unwrap().data();
        let lines = LineIterator::new(&data);
        let terminal = build_terminal().unwrap();
        let mut terminal_buffer = TerminalBuffer::new(terminal.width(), terminal.height());
        let theme_set = ThemeSet::load_defaults();
        let preferences = Preferences::new(None);
        let render_cache = Rc::new(RefCell::new(HashMap::new()));

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            95,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &render_cache,
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines, None)
        .unwrap();

        assert_eq!(render_cache.borrow().keys().count(), 1);
        terminal.clear();

        workspace.current_buffer.as_mut().unwrap().insert("\"");
        let data2 = workspace.current_buffer.as_ref().unwrap().data();
        let lines2 = LineIterator::new(&data2);

        BufferRenderer::new(
            workspace.current_buffer.as_ref().unwrap(),
            None,
            200,
            &**terminal,
            &theme_set.themes["base16-ocean.dark"],
            &preferences,
            &render_cache,
            &workspace.syntax_set,
            &mut terminal_buffer,
        )
        .render(lines2, None)
        .unwrap();

        let expected_content = " 201  line\n 202  line\n 203  line\n 204      ";
        assert_eq!(
            &terminal_buffer.content()[0..expected_content.len()],
            expected_content
        );
    }
}
