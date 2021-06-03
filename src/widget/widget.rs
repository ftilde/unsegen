//! The `Widget` abstraction and some related types.
use base::basic_types::*;
use base::{Cursor, Window, WrappingMode};
use std::cmp::max;
use std::iter::Sum;
use std::marker::PhantomData;
use std::ops::{Add, AddAssign};

/// A widget is something that can be drawn to a window.
pub trait Widget {
    /// Return the current demand for (rectangular) screen estate.
    ///
    /// The callee may report different
    /// demands on subsequent calls.
    fn space_demand(&self) -> Demand2D;

    /// Draw the widget to the given window.
    ///
    /// There is no guarantee that the window is of the size
    /// requested in `space_demand`, it can be smaller than the minimum or larger than the maximum
    /// (if specified). However, in general, the layouting algorithm tries to honor the demand of
    /// the widget.
    ///
    /// The hints give the widget some useful information on how to render.
    fn draw(&self, window: Window, hints: RenderingHints);
}

/// An extension trait to Widget which access to convenience methods that alters the behavior of
/// the wrapped widgets.
pub trait WidgetExt: Widget + Sized {
    /// Center the widget according to the specified maximum demand within the supplied window.
    /// This is only useful if the widget has a defined maximum size and the window is larger than
    /// that.
    fn centered(self) -> Centered<Self> {
        Centered(self)
    }

    /// Alter the window before letting the widget draw itself in it.
    fn with_window<F: Fn(Window, RenderingHints) -> Window>(self, f: F) -> WithWindow<Self, F> {
        WithWindow(self, f)
    }

    /// Alter the reported demand of the widget. This can be useful, for example, to force a widget
    /// to assume all space in the window or to artificially restrict the size of a widget.
    fn with_demand<F: Fn(Demand2D) -> Demand2D>(self, f: F) -> WithDemand<Self, F> {
        WithDemand(self, f)
    }
}

impl<W: Widget + Sized> WidgetExt for W {}

/// Center the widget according to the specified maximum demand within the supplied window.
/// This is only useful if the widget has a defined maximum size and the window is larger than
/// that.
///
/// This wrapper can be created using `WidgetExt::centered`.
pub struct Centered<W>(W);

impl<W: Widget> Widget for Centered<W> {
    fn space_demand(&self) -> Demand2D {
        self.0.space_demand()
    }
    fn draw(&self, mut window: Window, hints: RenderingHints) {
        let demand = self.space_demand();

        let window_height = window.get_height();
        let window_width = window.get_width();

        let max_height = demand.height.max.unwrap_or(window.get_height());
        let max_width = demand.width.max.unwrap_or(window.get_width());

        let start_row = ((window_height - max_height) / 2)
            .from_origin()
            .positive_or_zero();
        let start_col = ((window_width - max_width) / 2)
            .from_origin()
            .positive_or_zero();
        let end_row = (start_row + max_height).min(window_height.from_origin());
        let end_col = (start_col + max_width).min(window_width.from_origin());

        let window = window.create_subwindow(start_col..end_col, start_row..end_row);
        self.0.draw(window, hints);
    }
}

/// Alter the window before letting the widget draw itself in it.
///
/// This wrapper can be created using `WidgetExt::centered`.
pub struct WithWindow<W, F>(W, F);

impl<W: Widget, F: Fn(Window, RenderingHints) -> Window> Widget for WithWindow<W, F> {
    fn space_demand(&self) -> Demand2D {
        self.0.space_demand()
    }
    fn draw(&self, window: Window, hints: RenderingHints) {
        self.0.draw(self.1(window, hints), hints);
    }
}

/// Alter the reported demand of the widget. This can be useful, for example, to force a widget
/// to assume all space in the window or to artificially restrict the size of a widget.
///
/// This wrapper can be created using `WidgetExt::centered`.
pub struct WithDemand<W, F>(W, F);

impl<W: Widget, F: Fn(Demand2D) -> Demand2D> Widget for WithDemand<W, F> {
    fn space_demand(&self) -> Demand2D {
        self.1(self.0.space_demand())
    }
    fn draw(&self, window: Window, hints: RenderingHints) {
        self.0.draw(window, hints);
    }
}

impl<S: std::convert::AsRef<str>> Widget for S {
    fn space_demand(&self) -> Demand2D {
        let mut width = 0;
        let mut height = 0;
        for line in self.as_ref().lines() {
            width = width.max(crate::widget::count_grapheme_clusters(line));
            height += 1;
        }
        Demand2D {
            width: Demand::exact(width),
            height: Demand::exact(height),
        }
    }
    fn draw(&self, mut window: Window, _hints: RenderingHints) {
        let mut cursor = Cursor::new(&mut window).wrapping_mode(WrappingMode::Wrap);
        cursor.write(self.as_ref());
    }
}

/// Hints that can be used by applications to control how Widgets are rendered and used by Widgets
/// to deduce how to render to best show the current application state.
#[derive(Clone, Copy, Debug)]
pub struct RenderingHints {
    /// e.g., whether or not this Widget receives input
    pub active: bool,
    /// Periodic signal that can be used to e.g. let a cursor blink.
    pub blink: Blink,

    // Make users of the library unable to construct RenderingHints from members.
    // This way we can add members in a backwards compatible way in future versions.
    #[doc(hidden)]
    _do_not_construct: (),
}

impl Default for RenderingHints {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderingHints {
    /// Construct a default hint object.
    pub fn new() -> Self {
        RenderingHints {
            active: true,
            blink: Blink::On,
            _do_not_construct: (),
        }
    }
    /// Hint on whether the widget is active, i.e., most of the time: It receives input.
    pub fn active(self, val: bool) -> Self {
        RenderingHints {
            active: val,
            ..self
        }
    }

    /// Use this to implement blinking effects for your widget. Usually, Blink can be expected to
    /// alternate every second or so.
    pub fn blink(self, val: Blink) -> Self {
        RenderingHints { blink: val, ..self }
    }
}

/// A value from a periodic boolean signal.
///
/// Think of it like the state of an LED or cursor (block).
#[derive(Clone, Copy, Debug)]
#[allow(missing_docs)]
pub enum Blink {
    On,
    Off,
}

impl Blink {
    /// Get the alternate on/off value.
    pub fn toggled(self) -> Self {
        match self {
            Blink::On => Blink::Off,
            Blink::Off => Blink::On,
        }
    }

    /// Change to the alternate on/off value.
    pub fn toggle(&mut self) {
        *self = self.toggled();
    }
}

/// A one dimensional description of spatial demand of a widget.
///
/// A Demand always has a minimum (although it may be zero) and may have a maximum. It is required
/// that the minimum is smaller or equal to the maximum (if present).
#[derive(Eq, PartialEq, PartialOrd, Clone, Copy, Debug)]
#[allow(missing_docs)]
pub struct Demand<T: AxisDimension> {
    pub min: PositiveAxisDiff<T>,
    pub max: Option<PositiveAxisDiff<T>>,
    _dim: PhantomData<T>,
}

impl<T: AxisDimension> Add<Demand<T>> for Demand<T> {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Demand {
            min: self.min + rhs.min,
            max: if let (Some(l), Some(r)) = (self.max, rhs.max) {
                Some(l + r)
            } else {
                None
            },
            _dim: Default::default(),
        }
    }
}
impl<T: AxisDimension> AddAssign for Demand<T> {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs
    }
}
impl<T: AxisDimension + PartialOrd + Ord> Sum for Demand<T> {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(Demand::exact(0), Demand::add)
    }
}
impl<'a, T: AxisDimension + PartialOrd + Ord> Sum<&'a Demand<T>> for Demand<T> {
    fn sum<I>(iter: I) -> Demand<T>
    where
        I: Iterator<Item = &'a Demand<T>>,
    {
        iter.fold(Demand::zero(), |d1: Demand<T>, d2: &Demand<T>| d1 + *d2)
    }
}

impl<T: AxisDimension + PartialOrd + Ord> Demand<T> {
    /// A minimum and maximum demand of exactly 0.
    pub fn zero() -> Self {
        Self::exact(0)
    }

    /// A minimum and maximum demand of exactly the specified amount.
    pub fn exact<I: Into<PositiveAxisDiff<T>> + Copy>(size: I) -> Self {
        Demand {
            min: size.into(),
            max: Some(size.into()),
            _dim: Default::default(),
        }
    }
    /// An specified minimum demand, but no defined maximum.
    pub fn at_least<I: Into<PositiveAxisDiff<T>> + Copy>(size: I) -> Self {
        Demand {
            min: size.into(),
            max: None,
            _dim: Default::default(),
        }
    }
    /// A specified range of acceptable values between minimum and maximum.
    pub fn from_to<I: Into<PositiveAxisDiff<T>> + Copy>(min: I, max: I) -> Self {
        assert!(min.into() <= max.into(), "Invalid min/max");
        Demand {
            min: min.into(),
            max: Some(max.into()),
            _dim: Default::default(),
        }
    }

    /// Compute the composed maximum of two Demands. This is especially useful when building tables
    /// for example.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::widget::Demand;
    /// use unsegen::base::*;
    ///
    /// let d1 = Demand::<ColDimension>::exact(5);
    /// let d2 = Demand::<ColDimension>::at_least(0);
    ///
    /// assert_eq!(d1.max(d2), Demand::<ColDimension>::at_least(5));
    /// ```
    pub fn max(&self, other: Self) -> Self {
        Demand {
            min: max(self.min, other.min),
            max: if let (Some(l), Some(r)) = (self.max, other.max) {
                Some(max(l, r))
            } else {
                None
            },
            _dim: Default::default(),
        }
    }

    /// Replace self with the maximum of self and other (see `Demand::max`).
    pub fn max_assign(&mut self, other: Self) {
        *self = self.max(other);
    }
}

/// Horizontal Demand
pub type ColDemand = Demand<ColDimension>;
/// Vertical Demand
pub type RowDemand = Demand<RowDimension>;

/// A two dimensional (rectangular) Demand (composed of `Demand`s for columns and rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub struct Demand2D {
    pub width: ColDemand,
    pub height: RowDemand,
}

impl Demand2D {
    /// Combine two `Demand2D`s by accumulating the height and making the width accommodate both.
    ///
    /// This is useful two compute the combined  `Demand2D` of two widgets arranged on top of each
    /// other.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// use unsegen::widget::*;
    ///
    /// let d1 = Demand2D {
    ///     width: ColDemand::exact(5),
    ///     height: RowDemand::exact(5),
    /// };
    /// let d2 = Demand2D {
    ///     width: ColDemand::at_least(2),
    ///     height: RowDemand::from_to(3, 5),
    /// };
    ///
    /// assert_eq!(
    ///     d1.add_vertical(d2),
    ///     Demand2D {
    ///         width: ColDemand::at_least(5),
    ///         height: RowDemand::from_to(8, 10),
    ///     }
    /// );
    /// ```
    pub fn add_vertical(self, other: Self) -> Self {
        Demand2D {
            width: self.width.max(other.width),
            height: self.height + other.height,
        }
    }

    /// Combine two `Demand2D`s by accumulating the width and making the height accommodate both.
    ///
    /// This is useful two compute the combined  `Demand2D` of two widgets arranged on top of each
    /// other.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    /// use unsegen::widget::*;
    ///
    /// let d1 = Demand2D {
    ///     width: ColDemand::exact(5),
    ///     height: RowDemand::exact(5),
    /// };
    /// let d2 = Demand2D {
    ///     width: ColDemand::at_least(2),
    ///     height: RowDemand::from_to(3, 5),
    /// };
    ///
    /// assert_eq!(
    ///     d1.add_horizontal(d2),
    ///     Demand2D {
    ///         width: ColDemand::at_least(7),
    ///         height: RowDemand::exact(5),
    ///     }
    /// );
    /// ```
    pub fn add_horizontal(self, other: Self) -> Self {
        Demand2D {
            width: self.width + other.width,
            height: self.height.max(other.height),
        }
    }
}
