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
    cursor_style_active_blink_on: StyleModifier,
    cursor_style_active_blink_off: StyleModifier,
    cursor_style_inactive: StyleModifier,
}

impl LineEdit {
    /// Create with default cursor style: Underline position when inactive and invert on blink.
    pub fn new() -> Self {
        Self::with_cursor_styles(
            StyleModifier::new().invert(BoolModifyMode::Toggle),
            StyleModifier::new(),
            StyleModifier::new().underline(true),
        )
    }

    /// Create with the specified style for the cursor.
    ///
    /// Three styles have to be specified for the three possible states (in terms of rendering) of
    /// the cursor:
    ///
    /// 1. Active, and during an "on"-blink cycle.
    /// 2. Active, and during an "off"-blink cycle.
    /// 3. Inactive.
    pub fn with_cursor_styles(
        active_blink_on: StyleModifier,
        active_blink_off: StyleModifier,
        inactive: StyleModifier,
    ) -> Self {
        LineEdit {
            text: String::new(),
            cursor_pos: 0,
            cursor_style_active_blink_on: active_blink_on,
            cursor_style_active_blink_off: active_blink_off,
            cursor_style_inactive: inactive,
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

    /// Set (and overwrite) the current content. The cursor position (measured in grapheme clusters
    /// from the start) will be the same, if the new text is shorter or equally long. Otherwise the
    /// cursor will be positioned at the end of the line.
    pub fn replace(&mut self, text: impl Into<String>) {
        // TODO breaking change: somehow merge functionality with replace?
        self.text = text.into();
        if self.cursor_pos > count_grapheme_clusters(&self.text) {
            self.move_cursor_to_end_of_line();
        }
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
}

impl Widget for LineEdit {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(text_width(&self.text) + 1),
            height: Demand::exact(1), //TODO this is not really universal
        }
    }
    fn draw(&self, mut window: Window, hints: RenderingHints) {
        let (maybe_cursor_pos_offset, maybe_after_cursor_offset) = {
            let mut grapheme_indices = self.text.grapheme_indices(true);
            let cursor_cluster = grapheme_indices.nth(self.cursor_pos as usize);
            let next_cluster = grapheme_indices.next();
            (
                cursor_cluster.map(|c: (usize, &str)| c.0),
                next_cluster.map(|c: (usize, &str)| c.0),
            )
        };
        let right_padding = 1;
        let text_width_before_cursor =
            text_width(&self.text[0..maybe_after_cursor_offset.unwrap_or(self.text.len())]);
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
            let (until_cursor, from_cursor) = self.text.split_at(cursor_pos_offset);
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
            cursor.write(&self.text);
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                cursor.write(" ");
            }
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
