//! A user-editable line of text.
use base::basic_types::*;
use base::{BoolModifyMode, Cursor, StyleModifier, Window};
use input::{Editable, Navigatable, OperationResult, Writable};
use unicode_segmentation::UnicodeSegmentation;
use widget::{
    count_grapheme_clusters, text_width, Blink, Demand, Demand2D, RenderingHints, Widget,
};

/// A user-editable line of text.
///
/// In addition to the current text, the `LineEdit` has a concept of a cursor whose position can
/// change, but is always on a grapheme cluster in the current text.
pub struct LineEdit {
    text: String,
    cursor_pos: usize,
}

impl LineEdit {
    /// Create empty LineEdit
    pub fn new() -> Self {
        LineEdit {
            text: String::new(),
            cursor_pos: 0,
        }
    }

    /// Get the current content.
    pub fn get(&self) -> &str {
        &self.text
    }

    /// Set (and overwrite) the current content. The cursor will be placed at the very end of the
    /// line.
    pub fn set(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.move_cursor_to_end_of_line();
    }

    /// Move the cursor to the end, i.e., *behind* the last grapheme cluster.
    pub fn move_cursor_to_end_of_line(&mut self) {
        self.cursor_pos = count_grapheme_clusters(&self.text) as usize;
    }

    /// Move the cursor to the beginning, i.e., *onto* the first grapheme cluster.
    pub fn move_cursor_to_beginning_of_line(&mut self) {
        self.cursor_pos = 0;
    }

    /// Move the cursor one grapheme cluster to the right if possible.
    pub fn move_cursor_right(&mut self) -> Result<(), ()> {
        let new_pos = self.cursor_pos + 1;
        if new_pos <= count_grapheme_clusters(&self.text) as usize {
            self.cursor_pos = new_pos;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Move the cursor one grapheme cluster to the left if possible.
    pub fn move_cursor_left(&mut self) -> Result<(), ()> {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Insert text directly *before* the current cursor position
    pub fn insert(&mut self, text: &str) {
        self.text = {
            let grapheme_iter = self.text.graphemes(true);
            grapheme_iter
                .clone()
                .take(self.cursor_pos)
                .chain(Some(text))
                .chain(grapheme_iter.skip(self.cursor_pos))
                .collect()
        };
    }

    /// Returns the byte position of the cursor in the current text (obtainable by `get`)
    pub fn cursor_pos(&self) -> usize {
        self.text
            .grapheme_indices(true)
            .nth(self.cursor_pos)
            .map(|(index, _)| index)
            .unwrap_or_else(|| self.text.len())
    }

    /// Set the cursor by specifying its position as the byte position in the displayed string.
    ///
    /// If the byte position does not correspond to (the start of) a grapheme cluster in the string
    /// or the end of the string, an error is returned and the cursor position is left unchanged.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::widget::builtin::LineEdit;
    ///
    /// let mut l = LineEdit::new();
    /// l.set("löl");
    /// assert!(l.set_cursor_pos(0).is_ok()); // |löl
    /// assert!(l.set_cursor_pos(1).is_ok()); // l|öl
    /// assert!(l.set_cursor_pos(2).is_err());
    /// assert!(l.set_cursor_pos(3).is_ok()); // lö|l
    /// assert!(l.set_cursor_pos(4).is_ok()); // löl|
    /// assert!(l.set_cursor_pos(5).is_err());
    /// ```
    pub fn set_cursor_pos(&mut self, pos: usize) -> Result<(), ()> {
        if let Some(grapheme_index) = self
            .text
            .grapheme_indices(true)
            .enumerate()
            .find(|(_, (byte_index, _))| *byte_index == pos)
            .map(|(grapheme_index, _)| grapheme_index)
            .or_else(|| {
                if pos == self.text.len() {
                    Some(count_grapheme_clusters(&self.text))
                } else {
                    None
                }
            })
        {
            self.cursor_pos = grapheme_index;
            Ok(())
        } else {
            Err(())
        }
    }

    /// Erase the grapheme cluster at the specified (grapheme cluster) position.
    fn erase_symbol_at(&mut self, pos: usize) -> Result<(), ()> {
        if pos < count_grapheme_clusters(&self.text) {
            self.text = self
                .text
                .graphemes(true)
                .enumerate()
                .filter_map(|(i, s)| if i != pos { Some(s) } else { None })
                .collect();
            Ok(())
        } else {
            Err(())
        }
    }

    /// Prepare for drawing as a `Widget`.
    pub fn as_widget<'a>(&'a self) -> LineEditWidget<'a> {
        LineEditWidget {
            lineedit: self,
            cursor_style_active_blink_on: StyleModifier::new().invert(BoolModifyMode::Toggle),
            cursor_style_active_blink_off: StyleModifier::new(),
            cursor_style_inactive: StyleModifier::new().underline(true),
        }
    }
}

/// Note that there is no concept of moving up or down for a `LineEdit`.
impl Navigatable for LineEdit {
    fn move_up(&mut self) -> OperationResult {
        Err(())
    }
    fn move_down(&mut self) -> OperationResult {
        Err(())
    }
    fn move_left(&mut self) -> OperationResult {
        self.move_cursor_left()
    }
    fn move_right(&mut self) -> OperationResult {
        self.move_cursor_right()
    }
}

impl Writable for LineEdit {
    fn write(&mut self, c: char) -> OperationResult {
        if c == '\n' {
            Err(())
        } else {
            self.insert(&c.to_string());
            self.move_cursor_right()
        }
    }
}

impl Editable for LineEdit {
    fn delete_forwards(&mut self) -> OperationResult {
        //i.e., "del" key
        let to_erase = self.cursor_pos;
        self.erase_symbol_at(to_erase)
    }
    fn delete_backwards(&mut self) -> OperationResult {
        //i.e., "backspace"
        if self.cursor_pos > 0 {
            let to_erase = self.cursor_pos - 1;
            let _ = self.erase_symbol_at(to_erase);
            let _ = self.move_cursor_left();
            Ok(())
        } else {
            Err(())
        }
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
        if self.text.is_empty() {
            Err(())
        } else {
            self.text.clear();
            self.cursor_pos = 0;
            Ok(())
        }
    }
}

/// A `Widget` representing a `LineEdit`
///
/// It allows for customization of cursor styles.
pub struct LineEditWidget<'a> {
    lineedit: &'a LineEdit,
    cursor_style_active_blink_on: StyleModifier,
    cursor_style_active_blink_off: StyleModifier,
    cursor_style_inactive: StyleModifier,
}

impl<'a> LineEditWidget<'a> {
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

impl<'a> Widget for LineEditWidget<'a> {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(text_width(&self.lineedit.text) + 1),
            height: Demand::exact(1),
        }
    }
    fn draw(&self, mut window: Window, hints: RenderingHints) {
        let (maybe_cursor_pos_offset, maybe_after_cursor_offset) = {
            let mut grapheme_indices = self.lineedit.text.grapheme_indices(true);
            let cursor_cluster = grapheme_indices.nth(self.lineedit.cursor_pos as usize);
            let next_cluster = grapheme_indices.next();
            (
                cursor_cluster.map(|c: (usize, &str)| c.0),
                next_cluster.map(|c: (usize, &str)| c.0),
            )
        };
        let right_padding = 1;
        let text_width_before_cursor = text_width(
            &self.lineedit.text[0..maybe_after_cursor_offset.unwrap_or(self.lineedit.text.len())],
        );
        let draw_cursor_start_pos = ::std::cmp::min(
            ColIndex::new(0),
            (window.get_width() - text_width_before_cursor - right_padding).from_origin(),
        );

        let cursor_style = match (hints.active, hints.blink) {
            (true, Blink::On) => self.cursor_style_active_blink_on,
            (true, Blink::Off) => self.cursor_style_active_blink_off,
            (false, _) => self.cursor_style_inactive,
        };

        let mut cursor = Cursor::new(&mut window).position(draw_cursor_start_pos, RowIndex::new(0));
        if let Some(cursor_pos_offset) = maybe_cursor_pos_offset {
            let (until_cursor, from_cursor) = self.lineedit.text.split_at(cursor_pos_offset);
            cursor.write(until_cursor);
            if let Some(after_cursor_offset) = maybe_after_cursor_offset {
                let (cursor_str, after_cursor) =
                    from_cursor.split_at(after_cursor_offset - cursor_pos_offset);
                {
                    let mut cursor = cursor.save().style_modifier();
                    cursor.apply_style_modifier(cursor_style);
                    cursor.write(cursor_str);
                }
                cursor.write(after_cursor);
            } else {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                cursor.write(from_cursor);
            }
        } else {
            cursor.write(&self.lineedit.text);
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                cursor.write(" ");
            }
        }
    }
}
