//! The `Widget` abstraction and some related types.
use base::basic_types::*;
use base::Window;
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
    pub fn active(self, val: bool) -> Self {
        RenderingHints {
            active: val,
            ..self
        }
    }
    pub fn blink(self, val: Blink) -> Self {
        RenderingHints { blink: val, ..self }
    }
}

/// A value from a periodic boolean signal.
///
/// Think of it like the state of an LED or cursor (block).
#[derive(Clone, Copy, Debug)]
pub enum Blink {
    On,
    Off,
}

impl Blink {
    pub fn toggled(self) -> Self {
        match self {
            Blink::On => Blink::Off,
            Blink::Off => Blink::On,
        }
    }

    pub fn toggle(&mut self) {
        *self = self.toggled();
    }
}

/// A one dimensional description of spatial demand of a widget.
///
/// A Demand always has a minimum (although it may be zero) and may have a maximum. It is required
/// that the minimum is smaller or equal to the maximum (if present).
#[derive(Eq, PartialEq, PartialOrd, Clone, Copy, Debug)]
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
        use std::ops::Add;
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

pub type ColDemand = Demand<ColDimension>;
pub type RowDemand = Demand<RowDimension>;

/// A two dimensional (rectangular) Demand (composed of `Demand`s for columns and rows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Demand2D {
    pub width: ColDemand,
    pub height: RowDemand,
}
