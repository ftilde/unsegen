//! A user-editable region of text.
use base::{BoolModifyMode, ColIndex, Cursor, LineIndex, StyleModifier, Width, Window};
use input::{Editable, Navigatable, OperationResult, Writable};
use ropey::{Rope, RopeSlice};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use widget::{text_width, Blink, Demand, Demand2D, RenderingHints, Widget};

struct Text(Rope);

impl Text {
    fn empty() -> Self {
        Text(Rope::new())
    }
    fn with_content(s: &str) -> Self {
        Text(Rope::from_str(s))
    }
    fn clusters_forward(&self, mut pos: TextPosition, n: usize) -> Result<TextPosition, ()> {
        for _ in 0..n {
            pos = self.next_grapheme_cluster(pos)?;
        }
        Ok(pos)
    }
    fn next_grapheme_cluster(&self, pos: TextPosition) -> Result<TextPosition, ()> {
        let (mut chunk, mut chunk_begin, _, _) = self.0.chunk_at_byte(pos.0);
        let mut cursor = GraphemeCursor::new(pos.0, self.0.len_bytes(), true);
        loop {
            match cursor.next_boundary(chunk, chunk_begin) {
                Ok(None) => return Err(()),
                Ok(Some(n)) => return Ok(TextPosition(n)),
                Err(GraphemeIncomplete::NextChunk) => {
                    let (c, b, _, _) = self.0.chunk_at_byte(chunk_begin + chunk.len());
                    chunk = c;
                    chunk_begin = b;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (c, _, _, _) = self.0.chunk_at_byte(n - 1);
                    cursor.provide_context(c, n - c.len());
                }
                _ => unreachable!(),
            }
        }
    }
    fn clusters_backward(&self, mut pos: TextPosition, n: usize) -> Result<TextPosition, ()> {
        for _ in 0..n {
            pos = self.prev_grapheme_cluster(pos)?;
        }
        Ok(pos)
    }
    fn prev_grapheme_cluster(&self, pos: TextPosition) -> Result<TextPosition, ()> {
        let (mut chunk, mut chunk_begin, _, _) = self.0.chunk_at_byte(pos.0);
        let mut cursor = GraphemeCursor::new(pos.0, self.0.len_bytes(), true);
        loop {
            match cursor.prev_boundary(chunk, chunk_begin) {
                Ok(None) => return Err(()),
                Ok(Some(n)) => return Ok(TextPosition(n)),
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (c, b, _, _) = self.0.chunk_at_byte(chunk_begin - 1);
                    chunk = c;
                    chunk_begin = b;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (c, _, _, _) = self.0.chunk_at_byte(n - 1);
                    cursor.provide_context(c, n - c.len());
                }
                _ => unreachable!(),
            }
        }
    }
    fn cluster_in_line(&self, pos: TextPosition) -> usize {
        let begin = self.line_begin(pos);
        self.count_clusters(begin..pos)
    }
    fn count_clusters(&self, mut range: std::ops::Range<TextPosition>) -> usize {
        assert!(range.start <= range.end);
        let mut num = 0;
        while range.start < range.end {
            range.start = self.next_grapheme_cluster(range.start).unwrap();
            num += 1;
        }
        assert_eq!(range.start, range.end);
        num
    }
    fn begin(&self) -> TextPosition {
        TextPosition::begin()
    }
    fn end(&self) -> TextPosition {
        TextPosition(self.0.len_bytes())
    }

    fn line_index(&self, pos: TextPosition) -> LineIndex {
        LineIndex::new(self.0.byte_to_line(pos.0))
    }
    fn begin_of_line(&self, line: LineIndex) -> TextPosition {
        self.as_slice().begin_of_line(line)
    }
    fn num_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn line_begin(&self, pos: TextPosition) -> TextPosition {
        self.as_slice().line_begin(pos)
    }
    fn line_end(&self, pos: TextPosition) -> TextPosition {
        self.as_slice().line_end(pos)
    }

    fn insert(&mut self, pos: TextPosition, s: &str) -> TextPosition {
        let ci = self.0.byte_to_char(pos.0);
        self.0.insert(ci, s);
        TextPosition(pos.0 + s.len())
    }

    fn remove(&mut self, pos: std::ops::Range<TextPosition>) {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        self.0.remove(ci_begin..ci_end);
    }

    fn slice(&self, pos: std::ops::Range<TextPosition>) -> TextSlice<'_> {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        TextSlice(self.0.slice(ci_begin..ci_end))
    }

    fn lines<'a>(&'a self) -> impl Iterator<Item = TextSlice<'a>> + 'a {
        self.as_slice().lines()
    }

    fn as_slice<'a>(&'a self) -> TextSlice<'a> {
        TextSlice(self.0.slice(..))
    }
}

#[derive(Clone, Copy)]
struct TextSlice<'a>(RopeSlice<'a>);
impl<'a> TextSlice<'a> {
    fn text_width(self) -> Width {
        //TODO: it's probably possible to make this more efficient...
        text_width(&self.to_string())
    }
    fn end(self) -> TextPosition {
        TextPosition(self.0.len_bytes())
    }

    fn line_begin(self, pos: TextPosition) -> TextPosition {
        let line = self.0.byte_to_line(pos.0);
        TextPosition(self.0.line_to_byte(line))
    }
    fn line_end(self, pos: TextPosition) -> TextPosition {
        let line = self.0.byte_to_line(pos.0);
        if line == self.0.len_lines() - 1 {
            //last line
            self.end()
        } else {
            let newline_len = 1;
            TextPosition(self.0.line_to_byte(line + 1) - newline_len)
        }
    }
    fn begin_of_line(self, line: LineIndex) -> TextPosition {
        TextPosition(self.0.line_to_byte(line.raw_value()))
    }
    fn slice(self, pos: std::ops::Range<TextPosition>) -> TextSlice<'a> {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        TextSlice(self.0.slice(ci_begin..ci_end))
    }

    fn lines(self) -> impl Iterator<Item = TextSlice<'a>> + 'a {
        let num_lines = self.0.len_lines();
        let mut i = 0;
        std::iter::from_fn(move || {
            if i < num_lines {
                let begin = self.begin_of_line(LineIndex::new(i));
                let end = self.line_end(begin);
                i += 1;
                Some(self.slice(begin..end))
            } else {
                None
            }
        })
    }
}
impl std::fmt::Display for TextSlice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for c in self.0.chunks() {
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Debug)]
struct TextPosition(usize /* byte position */);

impl TextPosition {
    fn begin() -> Self {
        TextPosition(0)
    }
}

/// A user-editable region of text.
///
/// In addition to the current text, the `TextEdit` has a concept of a cursor whose position can
/// change, but is always on a grapheme cluster in the current text.
pub struct TextEdit {
    text: Text,
    cursor_pos: TextPosition,
}

impl TextEdit {
    /// Create empty TextEdit
    pub fn new() -> Self {
        TextEdit {
            text: Text::empty(),
            cursor_pos: TextPosition::begin(),
        }
    }

    /// Get the current content.
    pub fn get(&self) -> String {
        self.text
            .slice(self.text.begin()..self.text.end())
            .to_string()
    }

    /// Set (and overwrite) the current content. The cursor will be placed at the very end of the
    /// text.
    pub fn set(&mut self, text: impl AsRef<str>) {
        self.text = Text::with_content(text.as_ref());
        self.move_cursor_to_end();
    }

    /// Move the cursor to the end of the last line.
    pub fn move_cursor_to_end(&mut self) {
        self.cursor_pos = self.text.end();
    }

    /// Move the cursor to the end, i.e., *behind* the last grapheme cluster of the current line.
    pub fn move_cursor_to_end_of_line(&mut self) {
        self.cursor_pos = self.text.line_end(self.cursor_pos);
    }

    /// Move the cursor to the beginning, i.e., *onto* the first grapheme cluster of the current
    /// line.
    pub fn move_cursor_to_beginning_of_line(&mut self) {
        self.cursor_pos = self.text.line_begin(self.cursor_pos);
    }

    fn move_cursor_down(&mut self) -> Result<(), ()> {
        let line = self.text.line_index(self.cursor_pos);
        if line.raw_value() + 1 < self.text.num_lines() {
            let pos_in_line = self.text.cluster_in_line(self.cursor_pos);
            let line_begin = self.text.begin_of_line(line + 1);
            let line_end = self.text.line_end(line_begin);
            let num_clusters = self.text.count_clusters(line_begin..line_end);

            let pos_in_line = pos_in_line.min(num_clusters);
            self.cursor_pos = self.text.clusters_forward(line_begin, pos_in_line).unwrap();

            Ok(())
        } else {
            Err(())
        }
    }

    fn move_cursor_right(&mut self) -> Result<(), ()> {
        if self.text.line_end(self.cursor_pos) == self.cursor_pos {
            Err(())
        } else {
            self.cursor_pos = self.text.next_grapheme_cluster(self.cursor_pos)?;
            Ok(())
        }
    }

    fn move_cursor_up(&mut self) -> Result<(), ()> {
        let line = self.text.line_index(self.cursor_pos);
        if line.raw_value() > 0 {
            let pos_in_line = self.text.cluster_in_line(self.cursor_pos);
            let line_begin = self.text.begin_of_line(line - 1);
            let line_end = self.text.line_end(line_begin);
            let num_clusters = self.text.count_clusters(line_begin..line_end);

            let pos_in_line = pos_in_line.min(num_clusters);
            self.cursor_pos = self.text.clusters_forward(line_begin, pos_in_line).unwrap();

            Ok(())
        } else {
            Err(())
        }
    }

    fn move_cursor_left(&mut self) -> Result<(), ()> {
        if self.text.line_begin(self.cursor_pos) == self.cursor_pos {
            Err(())
        } else {
            self.cursor_pos = self.text.prev_grapheme_cluster(self.cursor_pos)?;
            Ok(())
        }
    }

    /// Insert text directly *before* the current cursor position
    pub fn insert(&mut self, text: &str) {
        self.text.insert(self.cursor_pos, text);
    }

    /// Returns the byte position of the cursor in the current line
    pub fn cursor_byte_pos_in_line(&self) -> usize {
        self.cursor_pos.0 - self.text.line_begin(self.cursor_pos).0
    }

    /// Prepare for drawing as a `Widget`.
    pub fn as_widget<'a>(&'a self) -> TextEditWidget<'a> {
        TextEditWidget {
            textedit: self,
            cursor_style_active_blink_on: StyleModifier::new().invert(BoolModifyMode::Toggle),
            cursor_style_active_blink_off: StyleModifier::new(),
            cursor_style_inactive: StyleModifier::new().underline(true),
        }
    }
}

/// Note that there is no concept of moving up or down for a `TextEdit`.
impl Navigatable for TextEdit {
    fn move_up(&mut self) -> OperationResult {
        self.move_cursor_up()
    }
    fn move_down(&mut self) -> OperationResult {
        self.move_cursor_down()
    }
    fn move_left(&mut self) -> OperationResult {
        self.move_cursor_left()
    }
    fn move_right(&mut self) -> OperationResult {
        self.move_cursor_right()
    }
}

impl Writable for TextEdit {
    fn write(&mut self, c: char) -> OperationResult {
        self.insert(&c.to_string());
        self.cursor_pos = self.text.next_grapheme_cluster(self.cursor_pos).unwrap();
        Ok(())
    }
}

impl Editable for TextEdit {
    fn delete_forwards(&mut self) -> OperationResult {
        //i.e., "del" key
        let start = self.cursor_pos;
        let end = self.text.next_grapheme_cluster(start)?;
        self.text.remove(start..end);
        Ok(())
    }
    fn delete_backwards(&mut self) -> OperationResult {
        //i.e., "backspace"
        let end = self.cursor_pos;
        let start = self.text.prev_grapheme_cluster(end)?;
        self.text.remove(start..end);
        self.cursor_pos = start;
        Ok(())
    }
    fn go_to_beginning_of_line(&mut self) -> OperationResult {
        self.move_cursor_to_beginning_of_line();
        Ok(())
    }
    fn go_to_end_of_line(&mut self) -> OperationResult {
        self.move_cursor_to_end_of_line();
        Ok(())
    }
    fn clear(&mut self) -> OperationResult {
        if self.text.0.len_bytes() == 0 {
            Err(())
        } else {
            self.text = Text::empty();
            self.cursor_pos = TextPosition::begin();
            Ok(())
        }
    }
}

/// A `Widget` representing a `TextEdit`
///
/// It allows for customization of cursor styles.
pub struct TextEditWidget<'a> {
    textedit: &'a TextEdit,
    cursor_style_active_blink_on: StyleModifier,
    cursor_style_active_blink_off: StyleModifier,
    cursor_style_inactive: StyleModifier,
}

impl<'a> TextEditWidget<'a> {
    /// Define the style that the cursor will be drawn with on the "on" tick when the widget is
    /// active.
    pub fn cursor_blink_on(mut self, style: StyleModifier) -> Self {
        self.cursor_style_active_blink_on = style;
        self
    }

    /// Define the style that the cursor will be drawn with on the "off" tick when the widget is
    /// active.
    pub fn cursor_blink_off(mut self, style: StyleModifier) -> Self {
        self.cursor_style_active_blink_off = style;
        self
    }

    /// Define the style that the cursor will be drawn with when the widget is inactive.
    pub fn cursor_inactive(mut self, style: StyleModifier) -> Self {
        self.cursor_style_inactive = style;
        self
    }
}

impl<'a> Widget for TextEditWidget<'a> {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(
                self.textedit
                    .text
                    .lines()
                    .map(|l| l.text_width())
                    .max()
                    .unwrap()
                    + 1,
            ),
            height: Demand::exact(self.textedit.text.num_lines()),
        }
    }
    fn draw(&self, mut window: Window, hints: RenderingHints) {
        let height = window.get_height();
        let before_cursor = self.textedit.cursor_pos;
        let line_begin = self.textedit.text.line_begin(before_cursor);
        let line_end = self.textedit.text.line_end(before_cursor);
        let after_cursor = self
            .textedit
            .text
            .next_grapheme_cluster(before_cursor)
            .ok()
            .filter(|a| a <= &line_end);

        let right_padding = 1;
        let lower_padding = 1;

        let text_width_before_cursor = self
            .textedit
            .text
            .slice(line_begin..before_cursor)
            .text_width();
        let draw_cursor_start_pos = ::std::cmp::min(
            ColIndex::new(0),
            (window.get_width() - text_width_before_cursor - right_padding).from_origin(),
        );

        let cursor_style = match (hints.active, hints.blink) {
            (true, Blink::On) => self.cursor_style_active_blink_on,
            (true, Blink::Off) => self.cursor_style_active_blink_off,
            (false, _) => self.cursor_style_inactive,
        };

        let current_line = self.textedit.text.line_index(self.textedit.cursor_pos);
        let num_following_lines = self.textedit.text.num_lines() - current_line.raw_value() - 1;
        let cursor_row = (height - 1 - lower_padding.min(num_following_lines as i32))
            .min((current_line.raw_value() as i32).into())
            .max(0.into())
            .from_origin();

        let mut cursor = Cursor::new(&mut window).position(draw_cursor_start_pos, cursor_row);
        cursor.set_line_start_column(draw_cursor_start_pos);

        use std::fmt::Write;
        if let Some(after_cursor) = after_cursor {
            let _ = write!(
                cursor,
                "{}",
                self.textedit.text.slice(line_begin..before_cursor)
            );
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                let _ = write!(
                    cursor,
                    "{}",
                    self.textedit.text.slice(before_cursor..after_cursor)
                );
            }
            let _ = write!(
                cursor,
                "{}",
                self.textedit.text.slice(after_cursor..line_end)
            );
        } else {
            let _ = write!(cursor, "{}", self.textedit.text.slice(line_begin..line_end));
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                cursor.write(" ");
            }
        }
        cursor.wrap_line();

        cursor.move_to_x(draw_cursor_start_pos);
        for line in self
            .textedit
            .text
            .slice(self.textedit.text.begin_of_line(current_line + 1)..self.textedit.text.end())
            .lines()
        {
            if cursor.get_row() >= height.from_origin() {
                break;
            }
            let _ = writeln!(cursor, "{}", line);
        }

        cursor.move_to_y(0.into());
        cursor.move_to_x(draw_cursor_start_pos);
        let num_rows_above = cursor_row.raw_value() as usize;
        assert!(num_rows_above <= current_line.raw_value());
        let first_line_begin = self
            .textedit
            .text
            .begin_of_line(current_line - num_rows_above);
        let last_line_end = line_begin;
        if current_line.raw_value() > 0 {
            for line in self
                .textedit
                .text
                .slice(first_line_begin..last_line_end)
                .lines()
            {
                let _ = writeln!(cursor, "{}", line);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::base::grapheme_cluster::GraphemeCluster;

    use super::*;
    use base::test::FakeTerminal;

    fn test_textedit<F: Fn(&mut TextEdit)>(window_dim: (u32, u32), after: &str, action: F) {
        let mut term = FakeTerminal::with_size(window_dim);
        {
            let mut window = term.create_root_window();
            window.fill(GraphemeCluster::try_from('_').unwrap());
            let mut textedit = TextEdit::new();
            action(&mut textedit);
            textedit
                .as_widget()
                .cursor_blink_on(StyleModifier::new().bold(true))
                .draw(
                    window,
                    RenderingHints::default().active(true).blink(Blink::On),
                );
        }
        term.assert_looks_like(after);
    }

    #[test]
    fn test_single_line_simple() {
        test_textedit((5, 1), "abc* *_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
        });
        test_textedit((5, 1), "a沐c* *", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
        });
    }

    #[test]
    fn test_single_line_move() {
        test_textedit((5, 1), "abd*c*_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.write('d').unwrap();
        });
        test_textedit((5, 1), "ab沐*c*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.write('沐').unwrap();
        });
        test_textedit((5, 1), "ad*b*c_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_to_beginning_of_line();
            t.move_cursor_right().unwrap();
            t.write('d').unwrap();
        });
        test_textedit((5, 1), "abc* *_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_to_end_of_line();
        });
    }

    #[test]
    fn test_single_line_delete() {
        test_textedit((5, 1), "a*c*___", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_forwards().unwrap();
        });
        test_textedit((5, 1), "*沐***c__", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_forwards().unwrap();
        });
    }

    #[test]
    fn test_single_line_backspace() {
        test_textedit((5, 1), "*b*c___", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_backwards().unwrap();
        });
        test_textedit((5, 1), "a*c*___", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.delete_backwards().unwrap();
        });
    }

    #[test]
    fn test_single_line_long() {
        //TODO: This is broken, but probably somewhere else? window? cursor?
        //test_textedit((5, 1), " def* *", |t| {
        //    t.write('a').unwrap();
        //    t.write('b').unwrap();
        //    t.write('沐').unwrap();
        //    t.write('d').unwrap();
        //    t.write('e').unwrap();
        //    t.write('f').unwrap();
        //});
        test_textedit((5, 1), "cdef* *", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
        });
        test_textedit((5, 1), "沐de*f*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('沐').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
            t.move_cursor_left().unwrap();
        });
        test_textedit((5, 1), "*a*b沐d", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('沐').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
            t.move_cursor_to_beginning_of_line();
        });
    }

    #[test]
    fn test_multi_line_simple() {
        test_textedit((2, 5), "a_|b_|c_|d_|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
            t.write('d').unwrap();
            t.write('\n').unwrap();
            t.write('e').unwrap();
        });

        test_textedit((2, 5), "a_|b_|c_|* *_|__", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
        });

        test_textedit((2, 3), "c_|d_|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
            t.write('d').unwrap();
            t.write('\n').unwrap();
            t.write('e').unwrap();
        });
    }

    #[test]
    fn test_multi_line_long() {
        test_textedit((2, 3), "__|__|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
        });

        test_textedit((2, 3), "b_|__|d* *", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
        });

        test_textedit((2, 3), "ab|o_|c*d*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.move_cursor_left().unwrap();
        });

        test_textedit((2, 3), "ab|o_|*c*d", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_to_beginning_of_line();
        });
    }

    #[test]
    fn test_multi_line_move() {
        test_textedit((2, 3), "ab|*o*_|cd", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_to_beginning_of_line();
            t.move_cursor_up().unwrap();
        });

        test_textedit((2, 3), "ab|o* *|cd", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_up().unwrap();
            t.move_cursor_to_beginning_of_line();
            assert!(t.move_cursor_left().is_err());
            t.move_cursor_to_end_of_line();
            assert!(t.move_cursor_right().is_err());
        });
    }
}
