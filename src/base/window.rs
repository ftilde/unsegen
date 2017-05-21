use super::{
    StyledGraphemeCluster,
    Style,
    StyleModifier,
    GraphemeCluster,
};
use ndarray::{
    ArrayViewMut,
    Axis,
    Ix,
    Ix2,
};
use std::cmp::max;
use base::ranges::{
    Bound,
    RangeArgument,
};
use ::unicode_segmentation::UnicodeSegmentation;

type CharMatrixView<'w> = ArrayViewMut<'w, StyledGraphemeCluster, Ix2>;
pub struct Window<'w> {
    values: CharMatrixView<'w>,
    default_style: Style,
}


impl<'w> Window<'w> {
    pub fn new(values: CharMatrixView<'w>, default_style: Style) -> Self {
        Window {
            values: values,
            default_style: default_style,
        }
    }

    pub fn get_width(&self) -> u32 {
        self.values.dim().1 as u32
    }

    pub fn get_height(&self) -> u32 {
        self.values.dim().0 as u32
    }

    pub fn clone_mut<'a>(&'a mut self) -> Window<'a> {
        let mat_view_clone = self.values.view_mut();
        Window {
            values: mat_view_clone,
            default_style: self.default_style,
        }
    }

    pub fn create_subwindow<'a, WX: RangeArgument<u32>, WY: RangeArgument<u32>>(&'a mut self, x_range: WX, y_range: WY) -> Window<'a> {
        let x_range_start = match x_range.start() {
            Bound::Unbound => 0,
            Bound::Inclusive(i) => i,
            Bound::Exclusive(i) => i-1,
        };
        let x_range_end = match x_range.end() {
            Bound::Unbound => self.get_width(),
            Bound::Inclusive(i) => i-1,
            Bound::Exclusive(i) => i,
        };
        let y_range_start = match y_range.start() {
            Bound::Unbound => 0,
            Bound::Inclusive(i) => i,
            Bound::Exclusive(i) => i-1,
        };
        let y_range_end = match y_range.end() {
            Bound::Unbound => self.get_height(),
            Bound::Inclusive(i) => i-1,
            Bound::Exclusive(i) => i,
        };
        assert!(x_range_start <= x_range_end, "Invalid x_range: start > end");
        assert!(y_range_start <= y_range_end, "Invalid y_range: start > end");
        assert!(x_range_end <= self.get_width(), "Invalid x_range: end > width");
        assert!(y_range_end <= self.get_height(), "Invalid y_range: end > height");

        let sub_mat = self.values.slice_mut(s![y_range_start as isize..y_range_end as isize, x_range_start as isize..x_range_end as isize]);
        Window {
            values: sub_mat,
            default_style: self.default_style,
        }
    }

    pub fn split_v(self, split_pos: u32) -> (Self, Self) {
        assert!(split_pos <= self.get_height(), "Invalid split_pos {} for window of height {}", split_pos, self.get_height());

        let (first_mat, second_mat) = self.values.split_at(Axis(0), split_pos as Ix);
        let w_u = Window {
            values: first_mat,
            default_style: self.default_style,
        };
        let w_d = Window {
            values: second_mat,
            default_style: self.default_style,
        };
        (w_u, w_d)
    }

    pub fn split_h(self, split_pos: u32) -> (Self, Self) {
        assert!(split_pos <= self.get_width(), "Invalid split_pos {} for window of width {}", split_pos, self.get_width());

        let (first_mat, second_mat) = self.values.split_at(Axis(1), split_pos as Ix);
        let w_l = Window {
            values: first_mat,
            default_style: self.default_style,
        };
        let w_r = Window {
            values: second_mat,
            default_style: self.default_style,
        };
        (w_l, w_r)
    }

    pub fn fill(&mut self, c: GraphemeCluster) {
        let cluster_width = c.width();
        let template = StyledGraphemeCluster::new(c, self.default_style);
        let empty = StyledGraphemeCluster::new(unsafe {GraphemeCluster::empty()}, self.default_style);
        let space = StyledGraphemeCluster::new(GraphemeCluster::space(), self.default_style);
        let right_border = (self.get_width() - (self.get_width() % cluster_width as u32)) as usize;
        for ((_, x), cell) in self.values.indexed_iter_mut() {
            if x >= right_border {
                *cell = space.clone();
            } else if x % cluster_width == 0 {
                *cell = template.clone();
            } else {
                *cell = empty.clone();
            }
        }
    }

    pub fn clear(&mut self) {
        self.fill(GraphemeCluster::space());
    }

    pub fn set_default_style(&mut self, style: Style) {
        self.default_style = style;
    }

    pub fn modify_default_style(&mut self, modifier: &StyleModifier) {
        modifier.modify(&mut self.default_style);
    }

    pub fn default_style(&self) -> &Style {
        &self.default_style
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WrappingMode {
    Wrap,
    NoWrap,
}


pub struct Cursor<'c, 'w: 'c> {
    window: &'c mut Window<'w>,
    wrapping_mode: WrappingMode,
    style_modifier: StyleModifier,
    x: i32,
    y: i32,
    line_start_column: i32,
    tab_column_width: u32,
}

impl<'c, 'w> Cursor<'c, 'w> {
    pub fn new(window: &'c mut Window<'w>) -> Self {
        Cursor {
            window: window,
            wrapping_mode: WrappingMode::NoWrap,
            style_modifier: StyleModifier::none(),
            x: 0,
            y: 0,
            line_start_column: 0,
            tab_column_width: 4,
        }
    }

    pub fn set_position(&mut self, x: i32, y: i32) {
        self.x = x;
        self.y = y;
    }

    pub fn position(mut self, x: i32, y: i32) -> Self {
        self.set_position(x, y);
        self
    }

    pub fn get_position(&self) -> (i32, i32) {
        (self.x, self.y)
    }

    pub fn move_by(&mut self, x: i32, y: i32) {
        self.x += x;
        self.y += y;
    }

    pub fn set_wrapping_mode(&mut self, wm: WrappingMode) {
        self.wrapping_mode = wm;
    }

    pub fn wrapping_mode(mut self, wm: WrappingMode) -> Self {
        self.set_wrapping_mode(wm);
        self
    }

    pub fn set_line_start_column(&mut self, column: i32) {
        self.line_start_column = column;
    }

    pub fn line_start_column(mut self, column: i32) -> Self{
        self.set_line_start_column(column);
        self
    }

    pub fn set_style_modifier(&mut self, style_modifier: StyleModifier) {
        self.style_modifier = style_modifier;
    }

    pub fn set_tab_column_width(&mut self, width: u32) {
        self.tab_column_width = width;
    }

    pub fn fill_and_wrap_line(&mut self) {
        while self.x < self.window.get_width() as i32 {
            self.write(" ");
        }
        self.wrap_line();
    }

    pub fn wrap_line(&mut self) {
        self.y += 1;
        self.x = self.line_start_column;
    }


    fn active_style(&self) -> Style {
        self.style_modifier.apply(&self.window.default_style)
    }

    pub fn num_expected_wraps(&self, line: &str) -> u32 {
        if self.wrapping_mode == WrappingMode::Wrap {
            let num_chars = line.graphemes(true).count();
            max(0, ((num_chars as i32 + self.x) / (self.window.get_width() as i32)) as u32)
        } else {
            0
        }
    }

    fn create_tab_cluster(width: u32) -> GraphemeCluster {
        use std::iter::FromIterator;
        let tab_string = String::from_iter(::std::iter::repeat(" ").take(width as usize));
        unsafe {
            GraphemeCluster::from_str_unchecked(tab_string)
        }
    }

    fn write_grapheme_cluster_unchecked(&mut self, cluster: GraphemeCluster) {
        let style = self.active_style();
        let target_cluster_x = self.x as Ix;
        let y = self.y as Ix;
        let old_target_cluster_width = {
            let target_cluster = self.window.values.get_mut((y, target_cluster_x)).expect("in bounds");
            let w = target_cluster.grapheme_cluster.width();
            *target_cluster = StyledGraphemeCluster::new(cluster, style);
            w
        };
        if old_target_cluster_width != 1 {
            // Find start of wide cluster which will be (partially) overwritten
            let mut current_x = target_cluster_x;
            let mut current_width = old_target_cluster_width;
            while current_width == 0 {
                current_x -= 1;
                current_width = self.window.values.get_mut((y, current_x)).expect("finding wide cluster start: read in bounds").grapheme_cluster.width();
            }

            // Clear all cells (except the newly written one)
            let start_cluster_x = current_x;
            let start_cluster_width = current_width;
            for x_to_clear in start_cluster_x..start_cluster_x+start_cluster_width {
                if x_to_clear != target_cluster_x {
                    self.window.values.get_mut((y, x_to_clear)).expect("overwrite cluster cells in bounds").grapheme_cluster.clear();
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

    fn remaining_space_in_line(&self) -> i32 {
        self.window.get_width() as i32 - self.x
    }

    pub fn write(&mut self, text: &str) {
        if self.window.get_width() == 0 || self.window.get_height() == 0 {
            return;
        }

        let mut line_it = text.lines().peekable();
        while let Some(line) = line_it.next() {
            for mut grapheme_cluster in GraphemeCluster::all_from_str(line) {
                if grapheme_cluster.as_str() == "\t" {
                    let width = self.tab_column_width - ((self.x as u32) % self.tab_column_width);
                    grapheme_cluster = Self::create_tab_cluster(width)
                }
                let cluster_width = grapheme_cluster.width() as i32;

                let space_in_line = self.remaining_space_in_line();
                if space_in_line < cluster_width {
                    // Overwrite spaces that we could not fill with our (too wide) grapheme cluster
                    for _ in 0..space_in_line {
                        self.write_grapheme_cluster_unchecked(GraphemeCluster::space());
                        self.x += 1;
                    }
                    if self.wrapping_mode == WrappingMode::Wrap {
                        self.wrap_line();
                        if self.remaining_space_in_line() < cluster_width {
                            // Still no space for the cluster after line wrap: We have to give up.
                            // There is no way we can write our cluster anywhere.
                            break;
                        }
                    } else {
                        // We do not wrap, so we are outside of the window now
                        break;
                    }
                }
                if     0 <= self.x && (self.x as u32) < self.window.get_width()
                    && 0 <= self.y && (self.y as u32) < self.window.get_height() {

                    self.write_grapheme_cluster_unchecked(grapheme_cluster);
                }
                self.x += 1;
                // TODO: This still probably does not work if we _overwrite_ wide clusters.
                if cluster_width > 1 && 0 <= self.y && (self.y as u32) < self.window.get_height() {
                    for _ in 1..cluster_width {
                        if 0 <= self.x && (self.x as u32) < self.window.get_width() {
                            self.write_grapheme_cluster_unchecked(unsafe {
                                GraphemeCluster::empty()
                            });
                        }
                        self.x += 1;
                    }
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

    pub fn push_style<'a>(&'a mut self, style_modifier: StyleModifier) -> CursorStyleStack<'a, 'c, 'w> {
        CursorStyleStack::new(self, style_modifier)
    }
}

impl<'c, 'w> ::std::fmt::Write for Cursor<'c, 'w> {
    fn write_str(&mut self, s: &str) -> ::std::fmt::Result {
        self.write(s);
        Ok(())
    }
}

pub struct CursorStyleStack<'a, 'c: 'a, 'w: 'c> {
    cursor: &'a mut Cursor<'c, 'w>,
    previous_style_modifier: StyleModifier,
}

impl<'a, 'c: 'a, 'w: 'c> CursorStyleStack<'a, 'c, 'w> {
    pub fn new(cursor: &'a mut Cursor<'c, 'w>, pushed_style_modifier: StyleModifier) -> Self {
        let current_style_modifier = cursor.style_modifier;
        let combined_style_modifier = current_style_modifier.if_not(pushed_style_modifier);
        cursor.style_modifier = combined_style_modifier;
        CursorStyleStack {
            cursor: cursor,
            previous_style_modifier: current_style_modifier,
        }
    }
}

impl<'a, 'c: 'a, 'w: 'c> ::std::ops::Drop for CursorStyleStack<'a, 'c, 'w> {
    fn drop(&mut self) {
        self.cursor.style_modifier = self.previous_style_modifier;
    }
}

impl<'a, 'c: 'a, 'w: 'c> ::std::ops::DerefMut for CursorStyleStack<'a, 'c, 'w> {
    fn deref_mut(&mut self) -> &mut Cursor<'c, 'w> {
        &mut self.cursor
    }
}


impl<'a, 'c: 'a, 'w: 'c> ::std::ops::Deref for CursorStyleStack<'a, 'c, 'w> {
    type Target = Cursor<'c, 'w>;
    fn deref(&self) -> &Cursor<'c, 'w> {
        &self.cursor
    }
}


#[cfg(test)]
mod test {
    use base::test::FakeTerminal;
    use super::*;

    fn test_cursor<S: Fn(&mut Cursor), F: Fn(&mut Cursor)>(window_dim: (Ix, Ix), after: &str, setup: S, action: F) {
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
        test_cursor((2, 2), "__|__", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write(""));
        test_cursor((2, 2), "t_|__", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("t"));
        test_cursor((2, 2), "te|__", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("te"));
        test_cursor((2, 2), "te|s_", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("tes"));
        test_cursor((2, 2), "te|st", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("test"));
        test_cursor((2, 2), "te|st", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("testy"));
    }

    #[test]
    fn test_cursor_tabs() {
        test_cursor((5, 1), "  x__", |c| c.set_tab_column_width(2), |c| c.write("\tx"));
        test_cursor((5, 1), "x x__", |c| c.set_tab_column_width(2), |c| c.write("x\tx"));
        test_cursor((5, 1), "xx  x", |c| c.set_tab_column_width(2), |c| c.write("xx\tx"));
        test_cursor((5, 1), "xxx x", |c| c.set_tab_column_width(2), |c| c.write("xxx\tx"));
        test_cursor((5, 1), "    x", |c| c.set_tab_column_width(2), |c| c.write("\t\tx"));
        test_cursor((5, 1), "     ", |c| c.set_tab_column_width(2), |c| c.write("\t\t\tx"));
    }

    #[test]
    fn test_cursor_wide_cluster() {
        test_cursor((5, 1), "沐___", |_| {}, |c| c.write("沐"));
        test_cursor((5, 1), "沐沐_", |_| {}, |c| c.write("沐沐"));
        test_cursor((5, 1), "沐沐 ", |_| {}, |c| c.write("沐沐沐"));

        test_cursor((3, 2), "沐_|___", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("沐"));
        test_cursor((3, 2), "沐 |沐_", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("沐沐"));
        test_cursor((3, 2), "沐 |沐 ", |c| c.set_wrapping_mode(WrappingMode::Wrap), |c| c.write("沐沐沐"));
    }

    #[test]
    fn test_cursor_wide_cluster_overwrite() {
        test_cursor((5, 1), "X ___", |_| {}, |c| { c.write("沐"); c.set_position(0,0); c.write("X"); });
        test_cursor((5, 1), " X___", |_| {}, |c| { c.write("沐"); c.set_position(1,0); c.write("X"); });
        test_cursor((5, 1), "XYZ _", |_| {}, |c| { c.write("沐沐"); c.set_position(0,0); c.write("XYZ"); });
        test_cursor((5, 1), " XYZ_", |_| {}, |c| { c.write("沐沐"); c.set_position(1,0); c.write("XYZ"); });
        test_cursor((5, 1), "沐XYZ", |_| {}, |c| { c.write("沐沐沐"); c.set_position(2,0); c.write("XYZ"); });
    }

    #[test]
    fn test_cursor_tabs_overwrite() {
        test_cursor((5, 1), "X   _", |c| c.set_tab_column_width(4), |c| { c.write("\t"); c.set_position(0,0); c.write("X"); });
        test_cursor((5, 1), " X  _", |c| c.set_tab_column_width(4), |c| { c.write("\t"); c.set_position(1,0); c.write("X"); });
        test_cursor((5, 1), "  X _", |c| c.set_tab_column_width(4), |c| { c.write("\t"); c.set_position(2,0); c.write("X"); });
        test_cursor((5, 1), "   X_", |c| c.set_tab_column_width(4), |c| { c.write("\t"); c.set_position(3,0); c.write("X"); });
    }
}