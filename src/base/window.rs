//! Types associated with Windows, i.e., rectangular views into a terminal buffer.
use super::{CursorTarget, GraphemeCluster, Style, StyleModifier};
use base::basic_types::*;
use base::cursor::{UNBOUNDED_HEIGHT, UNBOUNDED_WIDTH};
use ndarray::{Array, ArrayViewMut, Axis, Ix, Ix2};
use std::cmp::max;
use std::fmt;
use std::ops::{Bound, RangeBounds};

/// A GraphemeCluster with an associated style.
#[derive(Clone, Debug, PartialEq)]
pub struct StyledGraphemeCluster {
    pub grapheme_cluster: GraphemeCluster,
    pub style: Style,
}

impl StyledGraphemeCluster {
    /// Create a StyledGraphemeCluster from its components.
    pub fn new(grapheme_cluster: GraphemeCluster, style: Style) -> Self {
        StyledGraphemeCluster {
            grapheme_cluster: grapheme_cluster,
            style: style,
        }
    }
}

impl Default for StyledGraphemeCluster {
    fn default() -> Self {
        Self::new(GraphemeCluster::space(), Style::default())
    }
}

pub(in base) type CharMatrix = Array<StyledGraphemeCluster, Ix2>;

/// An owned buffer representing a Window.
#[derive(PartialEq)]
pub struct WindowBuffer {
    storage: CharMatrix,
}

impl WindowBuffer {
    /// Create a new WindowBuffer with the specified width and height.
    pub fn new(width: Width, height: Height) -> Self {
        WindowBuffer {
            storage: CharMatrix::default(Ix2(height.into(), width.into())),
        }
    }

    /// Create a WindowBuffer directly from a CharMatrix struct.
    pub(in base) fn from_storage(storage: CharMatrix) -> Self {
        WindowBuffer { storage: storage }
    }

    /// View the WindowBuffer as a Window.
    /// Use this method if you want to modify the contents of the buffer.
    pub fn as_window<'a>(&'a mut self) -> Window<'a> {
        Window::new(self.storage.view_mut())
    }

    /// Get the underlying CharMatrix storage.
    pub(in base) fn storage(&self) -> &CharMatrix {
        &self.storage
    }
}

type CharMatrixView<'w> = ArrayViewMut<'w, StyledGraphemeCluster, Ix2>;

/// A rectangular view into a terminal buffer, i.e., a grid of grapheme clusters.
///
/// Moreover, a window always has a default style that is applied to all characters that are
/// written to it. By default this is a "plain" style that does not change color or text format.
///
/// Side note: Grapheme clusters do not always have a width of a singular cell, and thus
/// things can quite complicated. Therefore, Multi-width characters occupy more than once cell in a
/// single window. If any of the cells is overwritten, potentially left-over cells are overwritten
/// with space characters.
pub struct Window<'w> {
    values: CharMatrixView<'w>,
    default_style: Style,
}

impl<'w> ::std::fmt::Debug for Window<'w> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let w: usize = self.get_width().into();
        let h: usize = self.get_height().into();
        write!(f, "Window {{ w: {}, h: {} }}", w, h)
    }
}

impl<'w> Window<'w> {
    /// Create a window from the underlying CharMatrixView and set a default (non-modifying) style.
    fn new(values: CharMatrixView<'w>) -> Self {
        Window {
            values: values,
            default_style: Style::default(),
        }
    }

    /// Get the extent of the window in the specified dimension (i.e., its width or height)
    pub fn get_extent<D: AxisDimension>(&self) -> PositiveAxisDiff<D> {
        PositiveAxisDiff::new(D::get_dimension_value(self.values.dim()) as i32).unwrap()
    }

    /// Get the width (i.e., number of columns) that the window occupies.
    pub fn get_width(&self) -> Width {
        self.get_extent::<ColDimension>()
    }

    /// Get the height (i.e., number of rows) that the window occupies.
    pub fn get_height(&self) -> Height {
        self.get_extent::<RowDimension>()
    }

    /// Create a subview of the window.
    ///
    /// # Examples:
    /// ```
    /// # use unsegen::base::terminal::test::FakeTerminal;
    /// # let mut term = FakeTerminal::with_size((5,5));
    /// use unsegen::base::{ColIndex, RowIndex, GraphemeCluster};
    ///
    /// let mut win = term.create_root_window();
    /// {
    ///     let mut w = win.create_subwindow(
    ///         ColIndex::new(0) .. ColIndex::new(2),
    ///         RowIndex::new(2) .. RowIndex::new(4)
    ///         );
    ///     w.fill(GraphemeCluster::try_from('A').unwrap());
    /// }
    /// {
    ///     let mut w = win.create_subwindow(
    ///         ColIndex::new(2) .. ColIndex::new(4),
    ///         RowIndex::new(0) .. RowIndex::new(2)
    ///         );
    ///     w.fill(GraphemeCluster::try_from('B').unwrap());
    /// }
    /// ```
    ///
    /// # Panics:
    ///
    /// Panics on invalid ranges, i.e., if:
    /// start > end, start < 0, or end > [width of the window]
    pub fn create_subwindow<'a, WX: RangeBounds<ColIndex>, WY: RangeBounds<RowIndex>>(
        &'a mut self,
        x_range: WX,
        y_range: WY,
    ) -> Window<'a> {
        let x_range_start = match x_range.start_bound() {
            Bound::Unbounded => ColIndex::new(0),
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i - 1,
        };
        let x_range_end = match x_range.end_bound() {
            Bound::Unbounded => self.get_width().from_origin(),
            Bound::Included(i) => *i - 1,
            Bound::Excluded(i) => *i,
        };
        let y_range_start = match y_range.start_bound() {
            Bound::Unbounded => RowIndex::new(0),
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i - 1,
        };
        let y_range_end = match y_range.end_bound() {
            Bound::Unbounded => self.get_height().from_origin(),
            Bound::Included(i) => *i - 1,
            Bound::Excluded(i) => *i,
        };
        assert!(x_range_start <= x_range_end, "Invalid x_range: start > end");
        assert!(y_range_start <= y_range_end, "Invalid y_range: start > end");
        assert!(
            x_range_end <= self.get_width().from_origin(),
            "Invalid x_range: end > width"
        );
        assert!(x_range_start >= 0, "Invalid x_range: start < 0");
        assert!(
            y_range_end <= self.get_height().from_origin(),
            "Invalid y_range: end > height"
        );
        assert!(y_range_start >= 0, "Invalid y_range: start < 0");

        let sub_mat = self.values.slice_mut(s![
            y_range_start.into()..y_range_end.into(),
            x_range_start.into()..x_range_end.into()
        ]);
        Window {
            values: sub_mat,
            default_style: self.default_style,
        }
    }

    /// Split the window horizontally or vertically into two halves.
    ///
    /// If the split position is invalid (i.e., larger than the height/width of the window or
    /// negative), the original window is returned untouched as the error value. split_pos defines
    /// the first row of the second window.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    ///
    /// let mut wb = WindowBuffer::new(Width::new(5).unwrap(), Height::new(5).unwrap());
    /// {
    ///     let win = wb.as_window();
    ///     let (w1, w2) = win.split(RowIndex::new(3)).unwrap();
    ///     assert_eq!(w1.get_height(), Height::new(3).unwrap());
    ///     assert_eq!(w1.get_width(), Width::new(5).unwrap());
    ///     assert_eq!(w2.get_height(), Height::new(2).unwrap());
    ///     assert_eq!(w2.get_width(), Width::new(5).unwrap());
    /// }
    /// {
    ///     let win = wb.as_window();
    ///     let (w1, w2) = win.split(ColIndex::new(3)).unwrap();
    ///     assert_eq!(w1.get_height(), Height::new(5).unwrap());
    ///     assert_eq!(w1.get_width(), Width::new(3).unwrap());
    ///     assert_eq!(w2.get_height(), Height::new(5).unwrap());
    ///     assert_eq!(w2.get_width(), Width::new(2).unwrap());
    /// }
    /// ```
    pub fn split<D: AxisDimension>(self, split_pos: AxisIndex<D>) -> Result<(Self, Self), Self> {
        if (self.get_extent() + PositiveAxisDiff::<D>::new(1).unwrap())
            .origin_range_contains(split_pos)
        {
            let (first_mat, second_mat) = self
                .values
                .split_at(Axis(D::NDARRAY_AXIS_NUMBER), split_pos.raw_value() as Ix);
            let w_u = Window {
                values: first_mat,
                default_style: self.default_style,
            };
            let w_d = Window {
                values: second_mat,
                default_style: self.default_style,
            };
            Ok((w_u, w_d))
        } else {
            Err(self)
        }
    }

    /// Fill the window with the specified GraphemeCluster.
    ///
    /// The style is defined by the default style of the window.
    /// If the grapheme cluster is wider than 1 cell, any left over cells are filled with space
    /// characters.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// let mut wb = WindowBuffer::new(Width::new(5).unwrap(), Height::new(5).unwrap());
    /// wb.as_window().fill(GraphemeCluster::try_from('X').unwrap());
    /// // Every cell of wb now contains an 'X'.
    ///
    /// wb.as_window().fill(GraphemeCluster::try_from('山').unwrap());
    /// // Every row of wb now contains two '山', while the last column cotains spaces.
    /// ```
    pub fn fill(&mut self, c: GraphemeCluster) {
        let cluster_width = c.width();
        let template = StyledGraphemeCluster::new(c, self.default_style);
        let empty = StyledGraphemeCluster::new(GraphemeCluster::empty(), self.default_style);
        let space = StyledGraphemeCluster::new(GraphemeCluster::space(), self.default_style);
        let w: i32 = self.get_width().into();
        let right_border = (w - (w % cluster_width as i32)) as usize;
        for ((_, x), cell) in self.values.indexed_iter_mut() {
            if x >= right_border.into() {
                *cell = space.clone();
            } else if x % cluster_width == 0 {
                *cell = template.clone();
            } else {
                *cell = empty.clone();
            }
        }
    }

    /// Fill the window with space characters.
    ///
    /// The style (i.e., the background color) is defined by the default style of the window.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// let mut wb = WindowBuffer::new(Width::new(5).unwrap(), Height::new(5).unwrap());
    /// wb.as_window().clear();
    /// // Every cell of wb now contains a ' '.
    /// ```
    pub fn clear(&mut self) {
        self.fill(GraphemeCluster::space());
    }

    /// Specify the new default style of the window. This style will be applied to all grapheme
    /// clusters written to the window.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// let mut wb = WindowBuffer::new(Width::new(5).unwrap(), Height::new(5).unwrap());
    /// let mut win = wb.as_window();
    /// win.set_default_style(StyleModifier::new().fg_color(Color::Red).bg_color(Color::Blue).apply_to_default());
    /// win.clear();
    /// // wb is now cleared and has a blue background.
    /// ```
    pub fn set_default_style(&mut self, style: Style) {
        self.default_style = style;
    }

    /// Specify the new default style of the window. This style will be applied to all grapheme
    /// clusters written to the window.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// let mut wb = WindowBuffer::new(Width::new(5).unwrap(), Height::new(5).unwrap());
    /// let mut win = wb.as_window();
    /// win.set_default_style(StyleModifier::new().fg_color(Color::Red).bg_color(Color::Blue).apply_to_default());
    /// win.clear();
    /// // wb is now cleared an has a blue background.
    ///
    /// win.modify_default_style(StyleModifier::new().bg_color(Color::Yellow));
    /// win.clear();
    /// // wb is now cleared an has a yellow background.
    ///
    /// assert_eq!(*win.default_style(),
    ///     StyleModifier::new().fg_color(Color::Red).bg_color(Color::Yellow).apply_to_default())
    /// ```
    pub fn modify_default_style(&mut self, modifier: StyleModifier) {
        modifier.modify(&mut self.default_style);
    }

    /// Get the current default style of the window.
    ///
    /// Change the default style using modify_default_style or set_default_style.
    pub fn default_style(&self) -> &Style {
        &self.default_style
    }
}

impl<'a> CursorTarget for Window<'a> {
    fn get_width(&self) -> Width {
        self.get_width()
    }
    fn get_height(&self) -> Height {
        self.get_height()
    }
    fn get_cell_mut(&mut self, x: ColIndex, y: RowIndex) -> Option<&mut StyledGraphemeCluster> {
        if x < 0 || y < 0 {
            None
        } else {
            let x: isize = x.into();
            let y: isize = y.into();
            self.values.get_mut((y as usize, x as usize))
        }
    }
    fn get_cell(&self, x: ColIndex, y: RowIndex) -> Option<&StyledGraphemeCluster> {
        if x < 0 || y < 0 {
            None
        } else {
            let x: isize = x.into();
            let y: isize = y.into();
            self.values.get((y as usize, x as usize))
        }
    }
    fn get_default_style(&self) -> Style {
        self.default_style
    }
}

/// A dummy window that does not safe any of the content written to it, but records the maximal
/// coordinates used.
///
/// This is therefore suitable for determining the required space demand of a
/// widget if nothing or not enough is known about the content of a widget.
pub struct ExtentEstimationWindow {
    some_value: StyledGraphemeCluster,
    default_style: Style,
    width: Width,
    extent_x: Width,
    extent_y: Height,
}

impl ExtentEstimationWindow {
    /// Create an ExtentEstimationWindow with a fixed width
    pub fn with_width(width: Width) -> Self {
        let style = Style::default();
        ExtentEstimationWindow {
            some_value: StyledGraphemeCluster::new(GraphemeCluster::space().into(), style),
            default_style: style,
            width: width,
            extent_x: Width::new(0).unwrap(),
            extent_y: Height::new(0).unwrap(),
        }
    }

    /// Create an ExtentEstimationWindow with an unbounded width
    pub fn unbounded() -> Self {
        Self::with_width(Width::new(UNBOUNDED_WIDTH).unwrap())
    }

    /// Get the width of the window required to display the contents written to the window.
    pub fn extent_x(&self) -> Width {
        self.extent_x
    }

    /// Get the height of the window required to display the contents written to the window.
    pub fn extent_y(&self) -> Height {
        self.extent_y
    }

    fn reset_value(&mut self) {
        self.some_value =
            StyledGraphemeCluster::new(GraphemeCluster::space().into(), self.default_style);
    }
}

impl CursorTarget for ExtentEstimationWindow {
    fn get_width(&self) -> Width {
        self.width
    }
    fn get_height(&self) -> Height {
        Height::new(UNBOUNDED_HEIGHT).unwrap()
    }
    fn get_cell_mut(&mut self, x: ColIndex, y: RowIndex) -> Option<&mut StyledGraphemeCluster> {
        self.extent_x = max(self.extent_x, (x.diff_to_origin() + 1).positive_or_zero());
        self.extent_y = max(self.extent_y, (y.diff_to_origin() + 1).positive_or_zero());
        self.reset_value();
        if x < self.width.from_origin() {
            Some(&mut self.some_value)
        } else {
            None
        }
    }

    fn get_cell(&self, x: ColIndex, _: RowIndex) -> Option<&StyledGraphemeCluster> {
        if x < self.width.from_origin() {
            Some(&self.some_value)
        } else {
            None
        }
    }
    fn get_default_style(&self) -> Style {
        self.default_style
    }
}
