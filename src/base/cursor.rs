//! A Cursor can be used to render text to Windows and Window-like types.
use super::{
    ColDiff, ColIndex, GraphemeCluster, Height, IndexRange, RowDiff, RowIndex, Style,
    StyleModifier, StyledGraphemeCluster, Width, Window,
};
use std::cmp::max;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

/// Defines how a cursor behaves when arriving at the right-hand border of the CursorTarget.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrappingMode {
    Wrap,
    NoWrap,
}

/// Something that can be written to using a Cursor. A most prominent example would be a Window.
pub trait CursorTarget {
    /// Return the actual width of the window. Writing to a column outside of this range is not
    /// possible.
    fn get_width(&self) -> Width;
    /// Return the soft or "desired" width of the target. In most cases this is equal to the width.
    /// An example where this is not the case would be a terminal with builtin wrapping of lines
    /// where it is some cases desirable for the cursor to wrap at the width of the terminal, even
    /// though there is no (conceptual) limit of line length.
    fn get_soft_width(&self) -> Width {
        self.get_width()
    }
    /// Return the maximum height of the target. Writing to a row outside of this range is not
    /// possible.
    fn get_height(&self) -> Height;

    /// Get the (mutable) cell at the specified position. The implementor must ensure that in the
    /// range for x \in [0, Width) and y \in [0, Height) a valid cluster is returned.
    fn get_cell_mut(&mut self, x: ColIndex, y: RowIndex) -> Option<&mut StyledGraphemeCluster>;

    /// Get the cell at the specified position. The implementor must ensure that in the range for
    /// x \in [0, Width) and y \in [0, Height) a valid cluster is returned.
    fn get_cell(&self, x: ColIndex, y: RowIndex) -> Option<&StyledGraphemeCluster>;

    /// Return the default style that characters of this target should be printed as. This serves
    /// as the base for further style modifications while writing to the target.
    fn get_default_style(&self) -> Style;
}

//FIXME: compile time evaluation, see https://github.com/rust-lang/rust/issues/24111
//Update(02/2019): const fn are partially stable, but are not allowed for types with trait bounds other that
//`Sized`.
//pub const UNBOUNDED_WIDTH: Width = Width::new(2147483647).unwrap();//i32::max_value() as u32;
//pub const UNBOUNDED_HEIGHT: Height = Height::new(2147483647).unwrap();//i32::max_value() as u32;
/// A symbolic value that can be used to specify that a cursor target does not have a maximum
/// width.
pub const UNBOUNDED_WIDTH: i32 = 2147483647; //i32::max_value() as u32;
/// A symbolic value that can be used to specify that a cursor target does not have a maximum
/// height.
pub const UNBOUNDED_HEIGHT: i32 = 2147483647; //i32::max_value() as u32;

/// The actual state of a Cursor in contrast to a Cursor instance itself, which also stored a
/// reference to the target it writes to.
pub struct CursorState {
    wrapping_mode: WrappingMode,
    style_modifier: StyleModifier,
    x: ColIndex,
    y: RowIndex,
    line_start_column: ColIndex,
    tab_column_width: Width,
}

impl Default for CursorState {
    fn default() -> Self {
        CursorState {
            wrapping_mode: WrappingMode::NoWrap,
            style_modifier: StyleModifier::new(),
            x: ColIndex::new(0),
            y: RowIndex::new(0),
            line_start_column: ColIndex::new(0),
            tab_column_width: Width::new(4).unwrap(),
        }
    }
}

/// Something that can be used to easily write text to a CursorTarget (e.g., a Window).
pub struct Cursor<'c, 'g: 'c, T: 'c + CursorTarget = Window<'g>> {
    window: &'c mut T,
    _dummy: ::std::marker::PhantomData<&'g ()>,
    state: CursorState,
}

impl<'c, 'g: 'c, T: 'c + CursorTarget> Cursor<'c, 'g, T> {
    /// Create a cursor to act on the specified window. The cursor initially resides at location
    /// (0,0) and only uses the style of the target.
    pub fn new(target: &'c mut T) -> Self {
        Self::from_state(target, CursorState::default())
    }

    /// Construct a cursor from the given state to act on the specified target.
    pub fn from_state(target: &'c mut T, state: CursorState) -> Self {
        Cursor {
            window: target,
            _dummy: ::std::marker::PhantomData::default(),
            state: state,
        }
    }

    /// Destroy the cursor and retrieve the current state. This is useful for storing the state
    /// between `draw`-like calls.
    pub fn into_state(self) -> CursorState {
        self.state
    }

    pub fn position(mut self, x: ColIndex, y: RowIndex) -> Self {
        self.move_to(x, y);
        self
    }

    pub fn get_position(&self) -> (ColIndex, RowIndex) {
        (self.state.x, self.state.y)
    }

    pub fn get_col(&self) -> ColIndex {
        self.state.x
    }

    pub fn get_row(&self) -> RowIndex {
        self.state.y
    }

    /// Move the cursor by the specifed amount in x- and y-direction.
    /// # Examples:
    ///
    /// ```
    /// use unsegen::base::*;
    ///
    /// let mut w = ExtentEstimationWindow::with_width(Width::new(20).unwrap());
    /// let mut cursor = Cursor::new(&mut w).position(ColIndex::new(27), RowIndex::new(37));
    ///
    /// cursor.move_by(ColDiff::new(10), RowDiff::new(-10));
    ///
    /// assert_eq!(cursor.get_col(), ColIndex::new(37));
    /// assert_eq!(cursor.get_row(), RowIndex::new(27));
    /// ```
    pub fn move_by(&mut self, x: ColDiff, y: RowDiff) {
        self.state.x += x;
        self.state.y += y;
    }

    pub fn move_to(&mut self, x: ColIndex, y: RowIndex) {
        self.state.x = x;
        self.state.y = y;
    }

    pub fn move_to_x(&mut self, x: ColIndex) {
        self.state.x = x;
    }

    pub fn move_to_y(&mut self, y: RowIndex) {
        self.state.y = y;
    }

    /// Move left, but skip empty clusters, also wrap to the line above if wrapping is active
    pub fn move_left(&mut self) {
        loop {
            if self.state.wrapping_mode == WrappingMode::Wrap
                && self.state.x <= self.state.line_start_column
            {
                self.move_by(0.into(), RowDiff::new(-1));
                let right_most_column: ColIndex = (self.window.get_soft_width() - 1).from_origin();
                self.move_to_x(right_most_column);
            } else {
                self.state.x = self.state.x - 1;
            }

            // Exit conditions:
            // - outside of window
            if self.state.y < 0 {
                break;
            }
            // on a non-zero width cluster
            let current = self.get_current_cell_mut();
            if current.is_some() && current.unwrap().grapheme_cluster.width() > 0 {
                break;
            }
        }
    }

    /// Move right, but skip empty clusters, also wrap to the line below if wrapping is active
    pub fn move_right(&mut self) {
        loop {
            if self.state.wrapping_mode == WrappingMode::Wrap
                && self.state.x > self.window.get_width().from_origin()
            {
                self.wrap_line();
            } else {
                self.state.x = self.state.x + 1;
            }

            // Exit conditions:
            // - outside of window
            if self.state.y >= self.window.get_height().from_origin() {
                break;
            }
            // on a non-zero width cluster
            let current = self.get_current_cell_mut();
            if current.is_some() && current.unwrap().grapheme_cluster.width() > 0 {
                break;
            }
        }
    }

    pub fn set_wrapping_mode(&mut self, wm: WrappingMode) {
        self.state.wrapping_mode = wm;
    }

    pub fn wrapping_mode(mut self, wm: WrappingMode) -> Self {
        self.set_wrapping_mode(wm);
        self
    }

    /// Set the column to which to cursor will jump automatically when wrapping at the end.
    /// Furthermore, when moving left, the cursor will wrap upwards if crossing over the start
    /// column.
    ///
    /// The default value is, of course, 0.
    ///
    /// # Examples:
    ///
    /// ```
    /// use unsegen::base::*;
    ///
    /// let mut w = ExtentEstimationWindow::with_width(Width::new(20).unwrap());
    /// let mut cursor = Cursor::new(&mut w)
    ///     .wrapping_mode(WrappingMode::Wrap)
    ///     .line_start_column(ColIndex::new(5))
    ///     .position(ColIndex::new(5), RowIndex::new(1));
    ///
    /// cursor.move_left();
    /// assert_eq!(cursor.get_col(), ColIndex::new(19));
    ///
    /// cursor.move_right();
    /// assert_eq!(cursor.get_col(), ColIndex::new(5));
    /// ```
    pub fn line_start_column(mut self, column: ColIndex) -> Self {
        self.set_line_start_column(column);
        self
    }

    pub fn set_line_start_column(&mut self, column: ColIndex) {
        self.state.line_start_column = column;
    }

    /// Move the start column by the specified amount.
    pub fn move_line_start_column(&mut self, d: ColDiff) {
        self.state.line_start_column += d;
    }

    /// Set the style modifier that will be used when writing cells to the target.
    /// The modifier will be applied to the base style of the target before writing to a cell.
    pub fn style_modifier(mut self, style_modifier: StyleModifier) -> Self {
        self.set_style_modifier(style_modifier);
        self
    }

    pub fn set_style_modifier(&mut self, style_modifier: StyleModifier) {
        self.state.style_modifier = style_modifier;
    }

    pub fn get_style_modifier(&mut self) -> StyleModifier {
        self.state.style_modifier
    }

    /// Apply the specified modifier *on_top* of the current modifier.
    pub fn apply_style_modifier(&mut self, style_modifier: StyleModifier) {
        self.state.style_modifier = style_modifier.on_top_of(self.state.style_modifier);
    }

    /// Set how far a tab character ('\t') will move the cursor to the right.
    pub fn set_tab_column_width(&mut self, width: Width) {
        self.state.tab_column_width = width;
    }

    /// Emulate a "backspace" action, i.e., move the cursor one character to the left and replace
    /// the character under the cursor with a space.
    pub fn backspace(&mut self) {
        self.move_left();

        let style = if let Some(c) = self.get_current_cell() {
            c.style
        } else {
            self.active_style()
        };

        self.write_cluster(GraphemeCluster::space(), &style)
            .expect("Cursor should be on screen");
        self.move_left();
    }

    /// Clear all characters within the current line according to the specified range, i.e.,
    /// replace them with spaces.
    fn clear_line_in_range(&mut self, range: Range<ColIndex>) {
        let style = self.active_style();
        let saved_x = self.state.x;
        for x in IndexRange(range.start..range.end) {
            self.move_to_x(x);
            self.write_cluster(GraphemeCluster::space(), &style)
                .expect("range should be in window size");
        }
        self.state.x = saved_x;
    }

    /// Clear all character to the left of the cursor in the current line including the cursor
    /// position itself.
    pub fn clear_line_left(&mut self) {
        let end = self.state.x + 1;
        self.clear_line_in_range(0.into()..end);
    }

    /// Clear all character to the right of the cursor in the current line including the cursor
    /// position itself.
    pub fn clear_line_right(&mut self) {
        let start = self.state.x;
        let end = self.window.get_soft_width().from_origin();
        self.clear_line_in_range(start..end);
    }

    /// Clear all characters in the current line.
    pub fn clear_line(&mut self) {
        let end = self.window.get_soft_width().from_origin();
        self.clear_line_in_range(0.into()..end);
    }

    /// Fill the remainder of the line with spaces and wrap to the beginning of the next line.
    pub fn fill_and_wrap_line(&mut self) {
        if self.window.get_height() == 0 {
            return;
        }
        let w = self.window.get_soft_width().from_origin();
        while self.state.x <= 0 || self.state.x % w != 0 {
            self.write(" ");
        }
        self.wrap_line();
    }

    /// Wrap to the beginning of the current line.
    pub fn wrap_line(&mut self) {
        self.state.y += 1;
        self.carriage_return();
    }

    /// Move the cursor to the beginning of the current line (but do not change the row position).
    pub fn carriage_return(&mut self) {
        self.state.x = self.state.line_start_column;
    }

    fn active_style(&self) -> Style {
        self.state
            .style_modifier
            .apply(self.window.get_default_style())
    }

    /// Calculate the number of wraps that are expected when writing the given text to the
    /// terminal, but do not write the text itself.
    pub fn num_expected_wraps(&self, line: &str) -> usize {
        if self.state.wrapping_mode == WrappingMode::Wrap {
            let num_chars = line.graphemes(true).count();
            let virtual_x_pos: i32 = (self.state.x + num_chars as i32).into();
            let w: i32 = self.window.get_width().into();
            max(0, (virtual_x_pos / w) as usize)
        } else {
            0
        }
    }

    /// Create a cluster representing a tab character for the curren tab width.
    fn create_tab_cluster(width: Width) -> GraphemeCluster {
        use std::iter::FromIterator;
        let tab_string =
            String::from_iter(::std::iter::repeat(" ").take(width.raw_value() as usize));
        GraphemeCluster::from_str_unchecked(tab_string)
    }

    /// (Mutably) get the cell under the current cursor position.
    pub fn get_current_cell_mut(&mut self) -> Option<&mut StyledGraphemeCluster> {
        if self.state.x < 0 || self.state.y < 0 {
            None
        } else {
            self.window.get_cell_mut(self.state.x, self.state.y)
        }
    }

    /// Get the cell under the current cursor position.
    pub fn get_current_cell(&self) -> Option<&StyledGraphemeCluster> {
        if self.state.x < 0 || self.state.y < 0 {
            None
        } else {
            self.window.get_cell(self.state.x, self.state.y)
        }
    }

    /// Write a grapheme cluster to the target assuming that there is enough space to write it
    /// without any wrapping.
    fn write_grapheme_cluster_unchecked(&mut self, cluster: GraphemeCluster, style: Style) {
        let target_cluster_x = self.state.x;
        let y = self.state.y;
        let old_target_cluster_width = {
            let target_cluster = self.get_current_cell_mut().expect("in bounds");
            let w: Width = Width::new(target_cluster.grapheme_cluster.width() as i32)
                .expect("width is non-negative");
            *target_cluster = StyledGraphemeCluster {
                grapheme_cluster: cluster,
                style,
            };
            w
        };
        if old_target_cluster_width != 1 {
            // Find start of wide cluster which will be (partially) overwritten
            let mut current_x = target_cluster_x;
            let mut current_width = old_target_cluster_width;
            while current_width == 0 {
                current_x -= 1;
                current_width = Width::new(
                    self.window
                        .get_cell_mut(current_x, y)
                        .expect("finding wide cluster start: read in bounds")
                        .grapheme_cluster
                        .width() as i32,
                )
                .expect("width is non-negative");
            }

            // Clear all cells (except the newly written one)
            let start_cluster_x = current_x;
            let start_cluster_width = current_width;
            for x_to_clear in
                IndexRange(start_cluster_x.into()..(start_cluster_x + start_cluster_width))
            {
                if x_to_clear != target_cluster_x {
                    self.window
                        .get_cell_mut(x_to_clear, y)
                        .expect("overwrite cluster cells in bounds")
                        .grapheme_cluster
                        .clear();
                }
            }
        }
        // This should cover (almost) all cases where we overwrite wide grapheme clusters.
        // Unfortunately, with the current design it is possible to split windows exactly at a
        // multicell wide grapheme cluster, e.g.: [f,o,o,b,a,r] => [f,o,沐,,a,r] => [f,o,沐|,a,r]
        // Now, when writing to to [f,o,沐| will trigger an out of bound access
        // => "overwrite cluster cells in bounds" will fail
        //
        // Alternatively: writing to |,a,r] will cause an under/overflow in
        // current_x -= 1;
        //
        // I will call this good for now, as these problems will likely not (or only rarely) arise
        // in pratice. If they do... we have to think of something...
    }

    /// Write a grapheme cluster to the target at the specified position. The cursor will be
    /// advanced accordingly. Wrapping and width of the terminal are handled as well.
    fn write_cluster(
        &mut self,
        grapheme_cluster: GraphemeCluster,
        style: &Style,
    ) -> Result<(), ()> {
        if !self.window.get_height().origin_range_contains(self.state.y) {
            // We are below the window already, no space left to write anything
            return Err(());
        }

        let cluster_width: Width =
            Width::new(grapheme_cluster.width() as i32).expect("width is non-negative");

        let space_in_line = self.remaining_space_in_line();
        if space_in_line < cluster_width {
            // Overwrite spaces that we could not fill with our (too wide) grapheme cluster
            for _ in 0..({
                let s: i32 = space_in_line.into();
                s
            }) {
                self.write_grapheme_cluster_unchecked(GraphemeCluster::space(), style.clone());
                self.state.x += 1;
            }
            if self.state.wrapping_mode == WrappingMode::Wrap {
                self.wrap_line();
                if self.remaining_space_in_line() < cluster_width {
                    // Still no space for the cluster after line wrap: We have to give up.
                    // There is no way we can write our cluster anywhere.
                    return Err(());
                }
            } else {
                // We do not wrap, so we are outside of the window now
                return Err(());
            }
        }
        if self.window.get_width().origin_range_contains(self.state.x)
            && self.window.get_height().origin_range_contains(self.state.y)
        {
            if cluster_width == 0 {
                self.get_current_cell_mut()
                    .expect("cursor in bounds")
                    .grapheme_cluster
                    .merge_zero_width(grapheme_cluster);
                return Ok(());
            }

            self.write_grapheme_cluster_unchecked(grapheme_cluster, style.clone());
        }
        self.state.x += 1;
        if cluster_width > 1 && self.window.get_height().origin_range_contains(self.state.y) {
            for _ in 1..cluster_width.into() {
                if self.window.get_width().origin_range_contains(self.state.x) {
                    self.write_grapheme_cluster_unchecked(GraphemeCluster::empty(), style.clone());
                }
                self.state.x += 1;
            }
        }
        Ok(())
    }

    /// Calculate the number of remaining cells in the current line.
    fn remaining_space_in_line(&self) -> Width {
        let x: ColIndex = self.state.x;
        let w: ColIndex = self.window.get_width().from_origin();
        if w < x {
            Width::new(0).unwrap()
        } else {
            (w - x).try_into_positive().expect("w >= x")
        }
    }

    /// Write a preformatted slice of styled grapheme clusters to the target at the current cursor
    /// position.
    ///
    /// Be very careful when using this function. Although safety is guaranteed, the program can
    /// easily panic when using this function in any wrong way.
    ///
    /// A safe way to obtain a valid argument for this function would be to implement a
    /// CursorTarget with a Vec<StyledGraphemeCluster> as a backing store and write to that target
    /// using a cursor. Any slice whose endpoints are defined by cursor positions which occured
    /// while writing to this target is valid.
    ///
    /// # Panics
    ///
    /// Panics if the number of clusters does not equals its total width
    pub fn write_preformatted(&mut self, clusters: &[StyledGraphemeCluster]) {
        if self.window.get_width() == 0 || self.window.get_height() == 0 {
            return;
        }

        assert!(
            clusters
                .iter()
                .map(|s| s.grapheme_cluster.width())
                .sum::<usize>()
                == clusters.len(),
            "Invalid preformated cluster slice!"
        );

        for cluster in clusters.iter() {
            if self
                .write_cluster(cluster.grapheme_cluster.clone(), &cluster.style)
                .is_err()
            {
                break;
            }
        }
    }

    /// Write a string to the target at the curren cursor position.
    pub fn write(&mut self, text: &str) {
        if self.window.get_width() == 0 || self.window.get_height() == 0 {
            return;
        }
        let style = self.active_style();

        let mut line_it = text.split('\n').peekable(); //.lines() swallows a terminal newline
        while let Some(line) = line_it.next() {
            for mut grapheme_cluster in GraphemeCluster::all_from_str(line) {
                match grapheme_cluster.as_str() {
                    "\t" => {
                        let tw = self.state.tab_column_width.from_origin();
                        let x = self.state.x;
                        let width = (tw - (x % tw)).try_into_positive().unwrap();
                        grapheme_cluster = Self::create_tab_cluster(width)
                    }
                    "\r" => {
                        self.carriage_return();
                        continue;
                    }
                    _ => {}
                }
                if self.write_cluster(grapheme_cluster, &style).is_err() {
                    break;
                }
            }
            if line_it.peek().is_some() {
                self.wrap_line();
            }
        }
    }

    pub fn writeln(&mut self, text: &str) {
        self.write(text);
        self.wrap_line();
    }

    /// Save the current state of the cursor. The current state will be restored when the returned
    /// CursorRestorer is dropped.
    ///
    /// # Examples:
    ///
    /// ```
    /// use unsegen::base::*;
    ///
    /// let mut w = ExtentEstimationWindow::with_width(Width::new(20).unwrap());
    /// let mut cursor = Cursor::new(&mut w)
    ///     .position(ColIndex::new(0), RowIndex::new(0));
    ///
    /// let old_style = cursor.get_style_modifier();
    /// {
    ///     let mut cursor = cursor.save().col().style_modifier();
    ///     cursor.apply_style_modifier(StyleModifier::new().bold(BoolModifyMode::Toggle));
    ///     cursor.write("testing, testing, oioioi!");
    /// }
    /// assert_eq!(cursor.get_col(), ColIndex::new(0));
    /// assert_eq!(cursor.get_style_modifier(), old_style);
    /// ```
    pub fn save<'a>(&'a mut self) -> CursorRestorer<'a, 'c, 'g, T> {
        CursorRestorer::new(self)
    }
}

impl<'c, 'g: 'c, T: 'c + CursorTarget> ::std::fmt::Write for Cursor<'c, 'g, T> {
    fn write_str(&mut self, s: &str) -> ::std::fmt::Result {
        self.write(s);
        Ok(())
    }
}

/// Guard value used to restore desired state to a cursor at the end of the scope. Created using
/// Cursor::save.
#[must_use]
pub struct CursorRestorer<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> {
    cursor: &'a mut Cursor<'c, 'g, T>,
    saved_style_modifier: Option<StyleModifier>,
    saved_line_start_column: Option<ColIndex>,
    saved_pos_x: Option<ColIndex>,
    saved_pos_y: Option<RowIndex>,
}

impl<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> CursorRestorer<'a, 'c, 'g, T> {
    fn new(cursor: &'a mut Cursor<'c, 'g, T>) -> Self {
        CursorRestorer {
            cursor: cursor,
            saved_style_modifier: None,
            saved_line_start_column: None,
            saved_pos_x: None,
            saved_pos_y: None,
        }
    }

    /// Save the current style modifier of the underlying cursor and restore it when this struct is
    /// dropped.
    pub fn style_modifier(mut self) -> Self {
        self.saved_style_modifier = Some(self.cursor.state.style_modifier);
        self
    }

    /// Save the current line start column of the underlying cursor and restore it when this struct
    /// is dropped.
    pub fn line_start_column(mut self) -> Self {
        self.saved_line_start_column = Some(self.cursor.state.line_start_column);
        self
    }

    /// Save the current column position of the underlying cursor and restore it when this struct
    /// is dropped.
    pub fn col(mut self) -> Self {
        self.saved_pos_x = Some(self.cursor.state.x);
        self
    }

    /// Save the current row position of the underlying cursor and restore it when this struct is
    /// dropped.
    pub fn row(mut self) -> Self {
        self.saved_pos_y = Some(self.cursor.state.y);
        self
    }
}

impl<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> ::std::ops::Drop for CursorRestorer<'a, 'c, 'g, T> {
    fn drop(&mut self) {
        if let Some(saved) = self.saved_style_modifier {
            self.cursor.state.style_modifier = saved;
        }
        if let Some(saved) = self.saved_line_start_column {
            self.cursor.state.line_start_column = saved;
        }
        if let Some(saved) = self.saved_pos_x {
            self.cursor.state.x = saved;
        }
        if let Some(saved) = self.saved_pos_y {
            self.cursor.state.y = saved;
        }
    }
}

impl<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> ::std::ops::DerefMut
    for CursorRestorer<'a, 'c, 'g, T>
{
    fn deref_mut(&mut self) -> &mut Cursor<'c, 'g, T> {
        &mut self.cursor
    }
}

impl<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> ::std::ops::Deref for CursorRestorer<'a, 'c, 'g, T> {
    type Target = Cursor<'c, 'g, T>;
    fn deref(&self) -> &Cursor<'c, 'g, T> {
        &self.cursor
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use base::test::FakeTerminal;

    fn test_cursor<S: Fn(&mut Cursor), F: Fn(&mut Cursor)>(
        window_dim: (u32, u32),
        after: &str,
        setup: S,
        action: F,
    ) {
        let mut term = FakeTerminal::with_size(window_dim);
        {
            let mut window = term.create_root_window();
            window.fill(GraphemeCluster::try_from('_').unwrap());
            let mut cursor = Cursor::new(&mut window);
            setup(&mut cursor);
            action(&mut cursor);
        }
        term.assert_looks_like(after);
    }
    #[test]
    fn test_cursor_simple() {
        test_cursor((5, 1), "_____", |_| {}, |c| c.write(""));
        test_cursor((5, 1), "t____", |_| {}, |c| c.write("t"));
        test_cursor((5, 1), "te___", |_| {}, |c| c.write("te"));
        test_cursor((5, 1), "tes__", |_| {}, |c| c.write("tes"));
        test_cursor((5, 1), "test_", |_| {}, |c| c.write("test"));
        test_cursor((5, 1), "testy", |_| {}, |c| c.write("testy"));
    }

    #[test]
    fn test_cursor_no_wrap() {
        test_cursor((2, 2), "__|__", |_| {}, |c| c.write(""));
        test_cursor((2, 2), "t_|__", |_| {}, |c| c.write("t"));
        test_cursor((2, 2), "te|__", |_| {}, |c| c.write("te"));
        test_cursor((2, 2), "te|__", |_| {}, |c| c.write("tes"));
        test_cursor((2, 2), "te|__", |_| {}, |c| c.write("test"));
        test_cursor((2, 2), "te|__", |_| {}, |c| c.write("testy"));
    }

    #[test]
    fn test_cursor_wrap() {
        test_cursor(
            (2, 2),
            "__|__",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write(""),
        );
        test_cursor(
            (2, 2),
            "t_|__",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("t"),
        );
        test_cursor(
            (2, 2),
            "te|__",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("te"),
        );
        test_cursor(
            (2, 2),
            "te|s_",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("tes"),
        );
        test_cursor(
            (2, 2),
            "te|st",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("test"),
        );
        test_cursor(
            (2, 2),
            "te|st",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("testy"),
        );
    }

    #[test]
    fn test_cursor_tabs() {
        test_cursor(
            (5, 1),
            "  x__",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("\tx"),
        );
        test_cursor(
            (5, 1),
            "x x__",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("x\tx"),
        );
        test_cursor(
            (5, 1),
            "xx  x",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("xx\tx"),
        );
        test_cursor(
            (5, 1),
            "xxx x",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("xxx\tx"),
        );
        test_cursor(
            (5, 1),
            "    x",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("\t\tx"),
        );
        test_cursor(
            (5, 1),
            "     ",
            |c| c.set_tab_column_width(Width::new(2).unwrap()),
            |c| c.write("\t\t\tx"),
        );
    }

    #[test]
    fn test_cursor_wide_cluster() {
        test_cursor((5, 1), "沐___", |_| {}, |c| c.write("沐"));
        test_cursor((5, 1), "沐沐_", |_| {}, |c| c.write("沐沐"));
        test_cursor((5, 1), "沐沐 ", |_| {}, |c| c.write("沐沐沐"));

        test_cursor(
            (3, 2),
            "沐_|___",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("沐"),
        );
        test_cursor(
            (3, 2),
            "沐 |沐_",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("沐沐"),
        );
        test_cursor(
            (3, 2),
            "沐 |沐 ",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write("沐沐沐"),
        );
        test_cursor(
            (2, 2),
            "  |沐",
            |c| c.set_wrapping_mode(WrappingMode::Wrap),
            |c| c.write(" 沐 沐"),
        );
    }

    #[test]
    fn test_cursor_wide_cluster_overwrite() {
        test_cursor(
            (5, 1),
            "X ___",
            |_| {},
            |c| {
                c.write("沐");
                c.move_to(ColIndex::new(0), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            " X___",
            |_| {},
            |c| {
                c.write("沐");
                c.move_to(ColIndex::new(1), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "XYZ _",
            |_| {},
            |c| {
                c.write("沐沐");
                c.move_to(ColIndex::new(0), RowIndex::new(0));
                c.write("XYZ");
            },
        );
        test_cursor(
            (5, 1),
            " XYZ_",
            |_| {},
            |c| {
                c.write("沐沐");
                c.move_to(ColIndex::new(1), RowIndex::new(0));
                c.write("XYZ");
            },
        );
        test_cursor(
            (5, 1),
            "沐XYZ",
            |_| {},
            |c| {
                c.write("沐沐沐");
                c.move_to(ColIndex::new(2), RowIndex::new(0));
                c.write("XYZ");
            },
        );
    }

    #[test]
    fn test_cursor_tabs_overwrite() {
        test_cursor(
            (5, 1),
            "X   _",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.move_to(ColIndex::new(0), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            " X  _",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.move_to(ColIndex::new(1), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "  X _",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.move_to(ColIndex::new(2), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "   X_",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.move_to(ColIndex::new(3), RowIndex::new(0));
                c.write("X");
            },
        );
    }
}
