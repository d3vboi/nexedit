use crate::view::buffer::LineNumbers;
use crate::view::terminal::Terminal;
use scribe::buffer::Buffer;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

pub struct ScrollableRegion {
    terminal: Arc<Box<dyn Terminal + Sync + Send + 'static>>,
    line_offset: usize,
}

impl ScrollableRegion {
    pub fn new(terminal: Arc<Box<dyn Terminal + Sync + Send + 'static>>) -> ScrollableRegion {
        ScrollableRegion {
            terminal,
            line_offset: 0,
        }
    }

    pub fn scroll_into_view(&mut self, buffer: &Buffer) {
        if buffer.cursor.line <= self.line_offset {
            self.line_offset = buffer.cursor.line;
        } else {
            let starting_line = (buffer.cursor.line)
                .saturating_sub(self.preceding_line_count(buffer, self.height()));

            if starting_line > self.line_offset {
                self.line_offset = starting_line;
            }
        }
    }

    pub fn scroll_to_center(&mut self, buffer: &Buffer) {
        let limit = (self.height() as f32 / 2.0).ceil() as usize;

        self.line_offset = buffer
            .cursor
            .line
            .saturating_sub(self.preceding_line_count(buffer, limit));
    }

    pub fn line_offset(&self) -> usize {
        self.line_offset
    }

    pub fn scroll_up(&mut self, amount: usize) {
        self.line_offset = self.line_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: usize) {
        self.line_offset += amount;
    }

    fn height(&self) -> usize {
        self.terminal.height() - 1
    }

    fn preceding_line_count(&self, buffer: &Buffer, limit: usize) -> usize {
        let mut preceding_line_count = 0;

        let gutter_width = LineNumbers::new(buffer, None).width() + 1;

        let end = buffer.cursor.line + 1;
        let start = end.saturating_sub(limit);
        let line_count = end - start;

        let visual_line_counts: Vec<usize> = buffer
            .data()
            .lines()
            .skip(start)
            .take(line_count)
            .map(|line| {
                let grapheme_count = line.graphemes(true).count().max(1) as f32;
                let buffer_content_width = (self.terminal.width() - gutter_width) as f32;
                let wrapped_line_count = grapheme_count / buffer_content_width;

                wrapped_line_count.ceil() as usize
            })
            .collect();

        let mut preceding_lines = visual_line_counts.iter().rev();
        let mut consumed_height = *preceding_lines.next().unwrap_or(&0);
        for height in preceding_lines {
            consumed_height += height;

            if consumed_height > limit {
                break;
            }
            preceding_line_count += 1;
        }

        if visual_line_counts.len() < line_count && preceding_line_count < limit - 1 {
            preceding_line_count += 1;
        }

        preceding_line_count
    }
}

#[cfg(test)]
mod tests {
    use super::ScrollableRegion;
    use crate::view::terminal::*;
    use scribe::buffer::{Buffer, Position};

    #[test]
    fn scroll_into_view_correctly_handles_additonal_rendered_trailing_newline() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        buffer.insert("\n\n");
        buffer.cursor.move_to(Position { line: 2, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 0);
    }

    #[test]
    fn scroll_into_view_correctly_handles_additonal_rendered_trailing_newline_at_edge_of_region() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        for _ in 0..10 {
            buffer.insert("\n");
        }
        buffer.cursor.move_to(Position {
            line: 10,
            offset: 0,
        });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 2);
    }

    #[test]
    fn scroll_into_view_advances_region_if_line_after_current_range() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        for _ in 0..10 {
            buffer.insert("word \n");
        }
        buffer.cursor.move_to(Position { line: 9, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 1);
    }

    #[test]
    fn scroll_into_view_recedes_region_if_line_before_current_range() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_down(10);
        for _ in 0..5 {
            buffer.insert("\n");
        }
        buffer.cursor.move_to(Position { line: 5, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 5);
    }

    #[test]
    fn scroll_into_view_considers_empty_lines_when_deciding_to_advance_region() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        for _ in 0..10 {
            buffer.insert("\n");
        }
        buffer.cursor.move_to(Position { line: 9, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 1);
    }

    #[test]
    fn scroll_into_view_advances_line_offset_if_preceding_lines_wrap() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        buffer.insert("cursor");
        for _ in 0..5 {
            buffer.insert("       \n");
        }

        buffer.cursor.move_to(Position { line: 5, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 1);
    }

    #[test]
    fn scroll_into_view_advances_line_offset_if_cursor_line_and_preceding_lines_wrap() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        buffer.insert("cursor line\n");
        for _ in 0..5 {
            buffer.insert("       \n");
        }
        buffer.cursor.move_to(Position { line: 5, offset: 0 });
        region.scroll_into_view(&buffer);
        assert_eq!(region.line_offset(), 2);
    }

    #[test]
    fn scroll_to_center_sets_correct_line_offset() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        for _ in 0..20 {
            buffer.insert("\n");
        }
        buffer.cursor.move_to(Position {
            line: 20,
            offset: 0,
        });
        region.scroll_to_center(&buffer);
        assert_eq!(region.line_offset(), 16);
    }

    #[test]
    fn scroll_to_center_does_not_set_negative_offset() {
        let terminal = build_terminal().unwrap();
        let buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_to_center(&buffer);
        assert_eq!(region.line_offset(), 0);
    }

    #[test]
    fn scroll_to_center_weighs_wrapped_lines_correctly() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        let mut region = ScrollableRegion::new(terminal);
        for _ in 0..4 {
            buffer.insert("       \n");
        }
        buffer.cursor.move_to(Position { line: 4, offset: 0 });
        for _ in 0..6 {
            buffer.insert("\n");
        }
        region.scroll_to_center(&buffer);
        assert_eq!(region.line_offset(), 2);
    }

    #[test]
    fn scroll_to_center_considers_space_beyond_end_of_buffer() {
        let terminal = build_terminal().unwrap();
        let mut buffer = Buffer::new();
        for _ in 0..6 {
            buffer.insert("\n");
        }
        buffer.cursor.move_to(Position { line: 5, offset: 0 });
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_to_center(&buffer);
        assert_eq!(region.line_offset(), 1);
    }

    #[test]
    fn scroll_down_increases_line_offset_by_amount() {
        let terminal = build_terminal().unwrap();
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_down(10);
        assert_eq!(region.line_offset(), 10);
    }

    #[test]
    fn scroll_up_decreases_line_offset_by_amount() {
        let terminal = build_terminal().unwrap();
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_down(10);
        region.scroll_up(5);
        assert_eq!(region.line_offset(), 5);
    }

    #[test]
    fn scroll_up_does_not_scroll_beyond_top_of_region() {
        let terminal = build_terminal().unwrap();
        let mut region = ScrollableRegion::new(terminal);
        region.scroll_up(5);
        assert_eq!(region.line_offset(), 0);
    }
}
