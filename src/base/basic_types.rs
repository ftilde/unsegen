use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Rem, Sub, SubAssign};
use std::marker::PhantomData;
use std::iter::Sum;

/// AxisIndex (the base for ColIndex or RowIndex) is a signed integer coordinate (i.e., a
/// coordinate of a point on the terminal cell grid)
#[derive(Copy, Clone, Debug, Ord, Eq)]
pub struct AxisIndex<T: AxisDimension> {
    val: i32,
    _dim: PhantomData<T>,
}

impl<T: AxisDimension> AxisIndex<T> {
    /// Create a new AxisIndex from an i32. Any i32 value is valid.
    pub fn new(v: i32) -> Self {
        AxisIndex {
            val: v,
            _dim: Default::default(),
        }
    }

    /// Unpack the AxisDiff to receive the raw i32 value.
    pub fn raw_value(self) -> i32 {
        self.into()
    }

    /// Calculate the origin of the Index to the origin of the coordinate grid (i.e., 0).
    /// Technically this just converts an AxisIndex into an AxisDiff, but is semantically more
    /// explicit.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColIndex, ColDiff};
    /// assert_eq!(ColIndex::new(27).diff_to_origin(), ColDiff::new(27));
    /// ```
    pub fn diff_to_origin(self) -> AxisDiff<T> {
        AxisDiff::new(self.val)
    }

    /// Clamp the value into a positive or zero range
    /// Example:
    ///
    /// ```
    /// use unsegen::base::ColIndex;
    /// assert_eq!(ColIndex::new(27).positive_or_zero(), ColIndex::new(27));
    /// assert_eq!(ColIndex::new(0).positive_or_zero(), ColIndex::new(0));
    /// assert_eq!(ColIndex::new(-37).positive_or_zero(), ColIndex::new(0));
    /// ```
    pub fn positive_or_zero(self) -> AxisIndex<T> {
        AxisIndex::new(self.val.max(0))
    }
}

impl<T: AxisDimension> From<i32> for AxisIndex<T> {
    fn from(v: i32) -> Self {
        AxisIndex::new(v)
    }
}
impl<T: AxisDimension> Into<i32> for AxisIndex<T> {
    fn into(self) -> i32 {
        self.val
    }
}
impl<T: AxisDimension> Into<isize> for AxisIndex<T> {
    fn into(self) -> isize {
        self.val as isize
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Add<I> for AxisIndex<T> {
    type Output = Self;
    fn add(self, rhs: I) -> Self {
        AxisIndex::new(self.val + rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> AddAssign<I> for AxisIndex<T> {
    fn add_assign(&mut self, rhs: I) {
        *self = *self + rhs;
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Sub<I> for AxisIndex<T> {
    type Output = Self;
    fn sub(self, rhs: I) -> Self {
        AxisIndex::new(self.val - rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> SubAssign<I> for AxisIndex<T> {
    fn sub_assign(&mut self, rhs: I) {
        *self = *self - rhs;
    }
}
impl<T: AxisDimension> Sub<Self> for AxisIndex<T> {
    type Output = AxisDiff<T>;
    fn sub(self, rhs: Self) -> Self::Output {
        AxisDiff::new(self.val - rhs.val)
    }
}
impl<T: AxisDimension, I: Into<AxisIndex<T>>> Rem<I> for AxisIndex<T> {
    type Output = Self;

    fn rem(self, modulus: I) -> Self {
        Self::new(self.val % modulus.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisIndex<T>> + Copy> PartialEq<I> for AxisIndex<T> {
    fn eq(&self, other: &I) -> bool {
        let copy = *other;
        self.val == copy.into().val
    }
}
impl<T: AxisDimension, I: Into<AxisIndex<T>> + Copy> PartialOrd<I> for AxisIndex<T> {
    fn partial_cmp(&self, other: &I) -> Option<Ordering> {
        let copy = *other;
        Some(self.val.cmp(&copy.into().val))
    }
}
impl<T: AxisDimension> Neg for AxisIndex<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        AxisIndex::new(-self.val)
    }
}

/// AxisDiff (the base for ColDiff or RowDiff) specifies a difference between two coordinate points
/// on a terminal grid. (i.e., a coordinate of a vector on the terminal cell grid)
#[derive(Copy, Clone, Debug, Ord, Eq)]
pub struct AxisDiff<T: AxisDimension> {
    val: i32,
    _dim: PhantomData<T>,
}

impl<T: AxisDimension> AxisDiff<T> {
    /// Create a new AxisDiff from an i32. Any i32 value is valid.
    pub fn new(v: i32) -> Self {
        AxisDiff {
            val: v,
            _dim: Default::default(),
        }
    }

    /// Unpack the AxisDiff to receive the raw i32 value.
    pub fn raw_value(self) -> i32 {
        self.into()
    }

    /// Calculate the AxisIndex that has the specified AxisDiff to the origin (i.e., 0).
    /// Technically this just converts an AxisIndex into an AxisDiff, but is semantically more
    /// explicit.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColIndex, ColDiff};
    /// assert_eq!(ColDiff::new(27).from_origin(), ColIndex::new(27));
    /// ```
    pub fn from_origin(self) -> AxisIndex<T> {
        AxisIndex::new(self.val)
    }

    /// Try to convert the current value into a PositiveAxisDiff.
    /// If the conversion fails, the original value is returned.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColDiff, Width};
    /// assert_eq!(ColDiff::new(27).try_into_positive(), Ok(Width::new(27).unwrap()));
    /// assert_eq!(ColDiff::new(0).try_into_positive(), Ok(Width::new(0).unwrap()));
    /// assert_eq!(ColDiff::new(-37).try_into_positive(), Err(ColDiff::new(-37)));
    /// ```
    pub fn try_into_positive(self) -> Result<PositiveAxisDiff<T>, Self> {
        PositiveAxisDiff::new(self.val).map_err(|()| self)
    }

    /// Convert the current value into a PositiveAxisDiff by taking the absolute value of the axis
    /// difference.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColDiff, Width};
    /// assert_eq!(ColDiff::new(27).abs(), Width::new(27).unwrap());
    /// assert_eq!(ColDiff::new(0).abs(), Width::new(0).unwrap());
    /// assert_eq!(ColDiff::new(-37).abs(), Width::new(37).unwrap());
    /// ```
    pub fn abs(self) -> PositiveAxisDiff<T> {
        PositiveAxisDiff::new_unchecked(self.val.abs())
    }

    /// Clamp the value into a positive or zero range
    /// Example:
    ///
    /// ```
    /// use unsegen::base::ColDiff;
    /// assert_eq!(ColDiff::new(27).positive_or_zero(), ColDiff::new(27));
    /// assert_eq!(ColDiff::new(0).positive_or_zero(), ColDiff::new(0));
    /// assert_eq!(ColDiff::new(-37).positive_or_zero(), ColDiff::new(0));
    /// ```
    pub fn positive_or_zero(self) -> PositiveAxisDiff<T> {
        PositiveAxisDiff::new_unchecked(self.val.max(0))
    }
}
impl<T: AxisDimension> From<i32> for AxisDiff<T> {
    fn from(v: i32) -> Self {
        AxisDiff::new(v)
    }
}
impl<T: AxisDimension> Into<i32> for AxisDiff<T> {
    fn into(self) -> i32 {
        self.val
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Add<I> for AxisDiff<T> {
    type Output = Self;
    fn add(self, rhs: I) -> Self {
        AxisDiff::new(self.val + rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> AddAssign<I> for AxisDiff<T> {
    fn add_assign(&mut self, rhs: I) {
        *self = *self + rhs;
    }
}
impl<T: AxisDimension> Mul<i32> for AxisDiff<T> {
    type Output = Self;
    fn mul(self, rhs: i32) -> Self::Output {
        AxisDiff::new(self.val * rhs)
    }
}
impl<T: AxisDimension> Div<i32> for AxisDiff<T> {
    type Output = AxisDiff<T>;
    fn div(self, rhs: i32) -> Self::Output {
        AxisDiff::new(self.val / rhs)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Sub<I> for AxisDiff<T> {
    type Output = Self;
    fn sub(self, rhs: I) -> Self {
        AxisDiff::new(self.val - rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> SubAssign<I> for AxisDiff<T> {
    fn sub_assign(&mut self, rhs: I) {
        *self = *self - rhs;
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Rem<I> for AxisDiff<T> {
    type Output = Self;

    fn rem(self, modulus: I) -> Self {
        AxisDiff::new(self.val % modulus.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>> + Copy> PartialEq<I> for AxisDiff<T> {
    fn eq(&self, other: &I) -> bool {
        let copy = *other;
        self.val == copy.into().val
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>> + Copy> PartialOrd<I> for AxisDiff<T> {
    fn partial_cmp(&self, other: &I) -> Option<Ordering> {
        let copy = *other;
        Some(self.val.cmp(&copy.into().val))
    }
}
impl<T: AxisDimension> Neg for AxisDiff<T> {
    type Output = Self;

    fn neg(self) -> Self::Output {
        AxisDiff::new(-self.val)
    }
}

/// PositiveAxisDiff (the base for Width or Height) specifies a non-negative (or absolute)
/// difference  between two coordinate points on a terminal grid.
#[derive(Copy, Clone, Debug, Ord, Eq)]
pub struct PositiveAxisDiff<T: AxisDimension> {
    val: i32,
    _dim: PhantomData<T>,
}

impl<T: AxisDimension> PositiveAxisDiff<T> {
    /// Create a new PositiveAxisDiff from an i32.
    /// If v < 0 the behavior is unspecified.
    /// Example:
    ///
    /// ```
    /// use unsegen::base::Width;
    /// let _ = Width::new_unchecked(27); //Ok
    /// let _ = Width::new_unchecked(0); //Ok
    /// // let _ = Width::new_unchecked(-37); //Not allowed!
    /// ```
    pub fn new_unchecked(v: i32) -> Self {
        assert!(v >= 0, "Invalid value for PositiveAxisDiff");
        PositiveAxisDiff {
            val: v,
            _dim: Default::default(),
        }
    }

    /// Try to create a new PositiveAxisDiff from an i32. If v < 0 the behavior an error value is
    /// returned.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::Width;
    /// assert!(Width::new(27).is_ok());
    /// assert!(Width::new(0).is_ok());
    /// assert!(Width::new(-37).is_err());
    /// ```
    pub fn new(v: i32) -> Result<Self, ()> {
        if v >= 0 {
            Ok(PositiveAxisDiff {
                val: v,
                _dim: Default::default(),
            })
        } else {
            Err(())
        }
    }

    /// Unpack the PositiveAxisDiff to receive the raw i32 value.
    pub fn raw_value(self) -> i32 {
        self.into()
    }

    /// Calculate the AxisIndex that has the specified PositiveAxisDiff to the origin (i.e., 0).
    /// Technically this just converts an AxisIndex into an PositiveAxisDiff, but is semantically
    /// more explicit.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColIndex, Width};
    /// assert_eq!(Width::new(27).unwrap().from_origin(), ColIndex::new(27));
    /// ```
    pub fn from_origin(self) -> AxisIndex<T> {
        AxisIndex::new(self.val)
    }

    /// Check whether the given AxisIndex is in the range [0, self)
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColIndex, Width};
    /// assert!(Width::new(37).unwrap().origin_range_contains(ColIndex::new(27)));
    /// assert!(Width::new(37).unwrap().origin_range_contains(ColIndex::new(0)));
    /// assert!(!Width::new(27).unwrap().origin_range_contains(ColIndex::new(27)));
    /// assert!(!Width::new(27).unwrap().origin_range_contains(ColIndex::new(37)));
    /// assert!(!Width::new(27).unwrap().origin_range_contains(ColIndex::new(-37)));
    /// ```
    pub fn origin_range_contains(self, i: AxisIndex<T>) -> bool {
        0 <= i.val && i.val < self.val
    }

    /// Convert the positive axis index into an AxisDiff.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::{ColDiff, Width};
    /// assert_eq!(Width::new(37).unwrap().to_signed(), ColDiff::new(37));
    /// ```
    pub fn to_signed(self) -> AxisDiff<T> {
        AxisDiff::new(self.val)
    }
}
impl<T: AxisDimension> Into<i32> for PositiveAxisDiff<T> {
    fn into(self) -> i32 {
        self.val
    }
}
impl<T: AxisDimension> Into<usize> for PositiveAxisDiff<T> {
    fn into(self) -> usize {
        self.val as usize
    }
}
impl<T: AxisDimension> Into<AxisDiff<T>> for PositiveAxisDiff<T> {
    fn into(self) -> AxisDiff<T> {
        AxisDiff::new(self.val)
    }
}
impl<T: AxisDimension, I: Into<PositiveAxisDiff<T>>> Add<I> for PositiveAxisDiff<T> {
    type Output = Self;
    fn add(self, rhs: I) -> Self {
        PositiveAxisDiff::new_unchecked(self.val + rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<PositiveAxisDiff<T>>> AddAssign<I> for PositiveAxisDiff<T> {
    fn add_assign(&mut self, rhs: I) {
        *self = *self + rhs;
    }
}
impl<T: AxisDimension> Mul<i32> for PositiveAxisDiff<T> {
    type Output = AxisDiff<T>;
    fn mul(self, rhs: i32) -> Self::Output {
        AxisDiff::new(self.val * rhs)
    }
}
impl<T: AxisDimension> Mul<usize> for PositiveAxisDiff<T> {
    type Output = Self;
    fn mul(self, rhs: usize) -> Self::Output {
        PositiveAxisDiff::new_unchecked(self.val * rhs as i32)
    }
}
impl<T: AxisDimension> Div<i32> for PositiveAxisDiff<T> {
    type Output = AxisDiff<T>;
    fn div(self, rhs: i32) -> Self::Output {
        AxisDiff::new(self.val / rhs)
    }
}
impl<T: AxisDimension> Div<usize> for PositiveAxisDiff<T> {
    type Output = Self;
    fn div(self, rhs: usize) -> Self::Output {
        PositiveAxisDiff::new_unchecked(self.val / rhs as i32)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>>> Sub<I> for PositiveAxisDiff<T> {
    type Output = AxisDiff<T>;
    fn sub(self, rhs: I) -> Self::Output {
        AxisDiff::new(self.val - rhs.into().val)
    }
}
impl<T: AxisDimension, I: Into<PositiveAxisDiff<T>>> Rem<I> for PositiveAxisDiff<T> {
    type Output = Self;
    fn rem(self, modulus: I) -> Self {
        PositiveAxisDiff::new_unchecked(self.val % modulus.into().val)
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>> + Copy> PartialEq<I> for PositiveAxisDiff<T> {
    fn eq(&self, other: &I) -> bool {
        let copy = *other;
        self.val == copy.into().val
    }
}
impl<T: AxisDimension, I: Into<AxisDiff<T>> + Copy> PartialOrd<I> for PositiveAxisDiff<T> {
    fn partial_cmp(&self, other: &I) -> Option<Ordering> {
        let copy = *other;
        Some(self.val.cmp(&copy.into().val))
    }
}
impl<T: AxisDimension + PartialOrd + Ord> Sum for PositiveAxisDiff<T> {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.fold(PositiveAxisDiff::new_unchecked(0), PositiveAxisDiff::add)
    }
}
impl<T: AxisDimension> From<usize> for PositiveAxisDiff<T> {
    fn from(v: usize) -> Self {
        assert!(
            v < i32::max_value() as usize,
            "Invalid PositiveAxisDiff value"
        );
        PositiveAxisDiff::new_unchecked(v as i32)
    }
}

/// ----------------------------------------------------------------------------
/// Concrete types for concrete dimensions --------------------------------------
/// ----------------------------------------------------------------------------

/// Trait for all dimensions of a terminal grid. See RowDimension and ColDimension.
pub trait AxisDimension: Copy {}

/// The horizontal (i.e., x-) dimension of a terminal grid. See ColIndex, ColDiff, and Width.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColDimension;
impl AxisDimension for ColDimension {}

/// The vertical (i.e., y-) dimension of a terminal grid. See RowIndex, RowDiff and Height.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct RowDimension;
impl AxisDimension for RowDimension {}

/// An AxisIndex in x-dimension.
pub type ColIndex = AxisIndex<ColDimension>;

/// An AxisDiff in x-dimension.
pub type ColDiff = AxisDiff<ColDimension>;

/// A PositiveAxisDiff in x-dimension.
pub type Width = PositiveAxisDiff<ColDimension>;

/// An AxisIndex in y-dimension.
pub type RowIndex = AxisIndex<RowDimension>;

/// An AxisDiff in y-dimension.
pub type RowDiff = AxisDiff<RowDimension>;

/// A PositiveAxisDiff in y-dimension.
pub type Height = PositiveAxisDiff<RowDimension>;
