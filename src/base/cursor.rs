use super::{ColDiff, ColIndex, GraphemeCluster, Height, RowDiff, RowIndex, Style, StyleModifier,
            StyledGraphemeCluster, Width, Window};
use std::cmp::max;
use std::ops::Range;
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrappingMode {
    Wrap,
    NoWrap,
}

pub trait CursorTarget {
    fn get_width(&self) -> Width;
    fn get_soft_width(&self) -> Width {
        self.get_width()
    }
    fn get_height(&self) -> Height;
    fn get_cell_mut(&mut self, x: ColIndex, y: RowIndex) -> Option<&mut StyledGraphemeCluster>;
    fn get_cell(&self, x: ColIndex, y: RowIndex) -> Option<&StyledGraphemeCluster>;
    fn get_default_style(&self) -> &Style;
}

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
            style_modifier: StyleModifier::none(),
            x: ColIndex::new(0),
            y: RowIndex::new(0),
            line_start_column: ColIndex::new(0),
            tab_column_width: Width::new(4).unwrap(),
        }
    }
}

pub struct Cursor<'c, 'g: 'c, T: 'c + CursorTarget = Window<'g>> {
    window: &'c mut T,
    _dummy: ::std::marker::PhantomData<&'g ()>,
    state: CursorState,
}

impl<'c, 'g: 'c, T: 'c + CursorTarget> Cursor<'c, 'g, T> {
    pub fn new(window: &'c mut T) -> Self {
        Self::with_state(window, CursorState::default())
    }

    pub fn with_state(window: &'c mut T, state: CursorState) -> Self {
        Cursor {
            window: window,
            _dummy: ::std::marker::PhantomData::default(),
            state: state,
        }
    }

    pub fn into_state(self) -> CursorState {
        self.state
    }

    pub fn set_position(&mut self, x: ColIndex, y: RowIndex) {
        self.state.x = x;
        self.state.y = y;
    }

    pub fn set_position_x(&mut self, x: ColIndex) {
        self.state.x = x;
    }

    pub fn set_position_y(&mut self, y: RowIndex) {
        self.state.y = y;
    }

    pub fn position(mut self, x: ColIndex, y: RowIndex) -> Self {
        self.set_position(x, y);
        self
    }

    pub fn get_position(&self) -> (ColIndex, RowIndex) {
        (self.state.x, self.state.y)
    }

    pub fn get_pos_x(&self) -> ColIndex {
        self.state.x
    }

    pub fn get_pos_y(&self) -> RowIndex {
        self.state.y
    }

    pub fn move_by(&mut self, x: ColDiff, y: RowDiff) {
        self.state.x += x;
        self.state.y += y;
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
            if self.state.wrapping_mode == WrappingMode::Wrap && self.state.x <= 0 {
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

    pub fn set_line_start_column(&mut self, column: ColIndex) {
        self.state.line_start_column = column;
    }

    pub fn move_line_start_column(&mut self, d: ColDiff) {
        self.state.line_start_column += d;
    }

    pub fn line_start_column(mut self, column: ColIndex) -> Self {
        self.set_line_start_column(column);
        self
    }

    pub fn set_style_modifier(&mut self, style_modifier: StyleModifier) {
        self.state.style_modifier = style_modifier;
    }

    pub fn apply_style_modifier(&mut self, style_modifier: StyleModifier) {
        self.state.style_modifier = style_modifier.on_top_of(&self.state.style_modifier);
    }

    pub fn set_tab_column_width(&mut self, width: Width) {
        self.state.tab_column_width = width;
    }

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

    fn clear_line_in_range(&mut self, range: Range<ColIndex>) {
        let style = self.active_style();
        let saved_x = self.state.x;
        // FIXME: Step trait stabilization
        for x in range.start.raw_value()..range.end.raw_value() {
            let x: ColIndex = x.into();
            self.move_to_x(x);
            self.write_cluster(GraphemeCluster::space(), &style)
                .expect("range should be in window size");
        }
        self.state.x = saved_x;
    }

    pub fn clear_line_left(&mut self) {
        let end = self.state.x + 1;
        self.clear_line_in_range(0.into()..end);
    }

    pub fn clear_line_right(&mut self) {
        let start = self.state.x;
        let end = self.window.get_soft_width().from_origin();
        self.clear_line_in_range(start..end);
    }

    pub fn clear_line(&mut self) {
        let end = self.window.get_soft_width().from_origin();
        self.clear_line_in_range(0.into()..end);
    }

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

    pub fn wrap_line(&mut self) {
        self.state.y += 1;
        self.carriage_return();
    }

    pub fn carriage_return(&mut self) {
        self.state.x = self.state.line_start_column;
    }

    fn active_style(&self) -> Style {
        self.state
            .style_modifier
            .apply(self.window.get_default_style())
    }

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

    fn create_tab_cluster(width: Width) -> GraphemeCluster {
        use std::iter::FromIterator;
        let tab_string =
            String::from_iter(::std::iter::repeat(" ").take(width.raw_value() as usize));
        GraphemeCluster::from_str_unchecked(tab_string)
    }

    pub fn get_current_cell_mut(&mut self) -> Option<&mut StyledGraphemeCluster> {
        if self.state.x < 0 || self.state.y < 0 {
            None
        } else {
            self.window.get_cell_mut(self.state.x, self.state.y)
        }
    }
    pub fn get_current_cell(&self) -> Option<&StyledGraphemeCluster> {
        if self.state.x < 0 || self.state.y < 0 {
            None
        } else {
            self.window.get_cell(self.state.x, self.state.y)
        }
    }

    fn write_grapheme_cluster_unchecked(&mut self, cluster: GraphemeCluster, style: Style) {
        let target_cluster_x = self.state.x;
        let y = self.state.y;
        let old_target_cluster_width = {
            let target_cluster = self.get_current_cell_mut().expect("in bounds");
            let w: Width = Width::new(target_cluster.grapheme_cluster.width() as i32)
                .expect("width is non-negative");
            *target_cluster = StyledGraphemeCluster::new(cluster, style);
            w
        };
        if old_target_cluster_width != 1 {
            // Find start of wide cluster which will be (partially) overwritten
            let mut current_x = target_cluster_x;
            let mut current_width = old_target_cluster_width;
            while current_width == 0 {
                current_x -= 1;
                current_width = Width::new(self.window
                    .get_cell_mut(current_x, y)
                    .expect("finding wide cluster start: read in bounds")
                    .grapheme_cluster
                    .width() as i32)
                    .expect("width is non-negative");
            }

            // Clear all cells (except the newly written one)
            let start_cluster_x = current_x;
            let start_cluster_width = current_width;
            let range: Range<i32> =
                start_cluster_x.into()..(start_cluster_x + start_cluster_width).into();
            // FIXME: Step trait stabilization
            for x_to_clear in range {
                let x_to_clear: ColIndex = x_to_clear.into();
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
        // I will call this good for now, as these problems will likely not (or only rarely) arrise
        // in pratice. If they do... we have to think of something...
    }

    fn write_cluster(
        &mut self,
        grapheme_cluster: GraphemeCluster,
        style: &Style,
    ) -> Result<(), ()> {
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
            // FIXME: Step trait stabilization
            for _ in 1..cluster_width.into() {
                if self.window.get_width().origin_range_contains(self.state.x) {
                    self.write_grapheme_cluster_unchecked(GraphemeCluster::empty(), style.clone());
                }
                self.state.x += 1;
            }
        }
        Ok(())
    }

    fn remaining_space_in_line(&self) -> Width {
        let x: ColIndex = self.state.x;
        let w: ColIndex = self.window.get_width().from_origin();
        if w < x {
            Width::new(0).unwrap()
        } else {
            (w - x).try_into_positive().expect("w >= x")
        }
    }

    pub fn write_preformatted(&mut self, clusters: &[StyledGraphemeCluster]) {
        if self.window.get_width() == 0 || self.window.get_height() == 0 {
            return;
        }

        assert!(
            clusters
                .iter()
                .map(|s| s.grapheme_cluster.width())
                .sum::<usize>() == clusters.len(),
            "Invalid preformated cluster slice!"
        );

        for cluster in clusters.iter() {
            if self.write_cluster(cluster.grapheme_cluster.clone(), &cluster.style)
                .is_err()
            {
                break;
            }
        }
    }

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

#[must_use]
pub struct CursorRestorer<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> {
    cursor: &'a mut Cursor<'c, 'g, T>,
    saved_style_modifier: Option<StyleModifier>,
    saved_line_start_column: Option<ColIndex>,
    saved_pos_x: Option<ColIndex>,
    saved_pos_y: Option<RowIndex>,
}

impl<'a, 'c: 'a, 'g: 'c, T: 'c + CursorTarget> CursorRestorer<'a, 'c, 'g, T> {
    pub fn new(cursor: &'a mut Cursor<'c, 'g, T>) -> Self {
        CursorRestorer {
            cursor: cursor,
            saved_style_modifier: None,
            saved_line_start_column: None,
            saved_pos_x: None,
            saved_pos_y: None,
        }
    }

    pub fn style_modifier(mut self) -> Self {
        self.saved_style_modifier = Some(self.cursor.state.style_modifier);
        self
    }

    pub fn line_start_column(mut self) -> Self {
        self.saved_line_start_column = Some(self.cursor.state.line_start_column);
        self
    }

    pub fn pos_x(mut self) -> Self {
        self.saved_pos_x = Some(self.cursor.state.x);
        self
    }

    pub fn pos_y(mut self) -> Self {
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
    use base::test::FakeTerminal;
    use super::*;

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
    }

    #[test]
    fn test_cursor_wide_cluster_overwrite() {
        test_cursor(
            (5, 1),
            "X ___",
            |_| {},
            |c| {
                c.write("沐");
                c.set_position(ColIndex::new(0), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            " X___",
            |_| {},
            |c| {
                c.write("沐");
                c.set_position(ColIndex::new(1), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "XYZ _",
            |_| {},
            |c| {
                c.write("沐沐");
                c.set_position(ColIndex::new(0), RowIndex::new(0));
                c.write("XYZ");
            },
        );
        test_cursor(
            (5, 1),
            " XYZ_",
            |_| {},
            |c| {
                c.write("沐沐");
                c.set_position(ColIndex::new(1), RowIndex::new(0));
                c.write("XYZ");
            },
        );
        test_cursor(
            (5, 1),
            "沐XYZ",
            |_| {},
            |c| {
                c.write("沐沐沐");
                c.set_position(ColIndex::new(2), RowIndex::new(0));
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
                c.set_position(ColIndex::new(0), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            " X  _",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.set_position(ColIndex::new(1), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "  X _",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.set_position(ColIndex::new(2), RowIndex::new(0));
                c.write("X");
            },
        );
        test_cursor(
            (5, 1),
            "   X_",
            |c| c.set_tab_column_width(Width::new(4).unwrap()),
            |c| {
                c.write("\t");
                c.set_position(ColIndex::new(3), RowIndex::new(0));
                c.write("X");
            },
        );
    }
}
