//! Basic numeric semantic wrapper types for use in other parts of the library.
use std::cmp::Ordering;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Range, Rem, Sub, SubAssign};
use std::marker::PhantomData;
use std::iter::Sum;
use std::fmt;

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
    /// # Examples:
    ///
    /// ```
    /// use unsegen::base::{ColIndex, ColDiff};
    /// assert_eq!(ColIndex::new(27).diff_to_origin(), ColDiff::new(27));
    /// ```
    pub fn diff_to_origin(self) -> AxisDiff<T> {
        AxisDiff::new(self.val)
    }

    /// Clamp the value into a positive or zero range
    ///
    /// # Examples:
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

/// Wrapper for Ranges of AxisIndex to make them iterable.
/// This should be removed once [#42168](https://github.com/rust-lang/rust/issues/42168) is stabilized.
pub struct IndexRange<T: AxisDimension>(pub Range<AxisIndex<T>>);

impl<T: AxisDimension> Iterator for IndexRange<T> {
    type Item = AxisIndex<T>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.0.start < self.0.end {
            let res = self.0.start;
            self.0.start += 1;
            Some(res)
        } else {
            None
        }
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
    /// # Examples:
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
    /// # Examples:
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
    /// # Examples:
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
    ///
    /// # Examples:
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
    ///
    /// # Examples:
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
    /// # Examples:
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
    /// # Examples:
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
    /// # Examples:
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
    /// # Examples:
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

// ----------------------------------------------------------------------------
// Concrete types for concrete dimensions -------------------------------------
// ----------------------------------------------------------------------------

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

// ----------------------------------------------------------------------------
// Wrapper types for line numbering -------------------------------------------
// ----------------------------------------------------------------------------

/// A type for enumerating lines by index (rather than by number), i.e., starting from 0.
/// Conversions between LineNumber and LineIndex are always safe.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug, Hash)]
pub struct LineIndex(usize);
impl LineIndex {
    /// Create a new LineIndex from a raw value.
    pub fn new(val: usize) -> Self {
        LineIndex(val)
    }

    /// Unpack the LineIndex and yield the underlying value.
    pub fn raw_value(self) -> usize {
        self.0
    }

    /// Checked integer subtraction. Computes self - rhs, returning None if the result is invalid
    /// (i.e., smaller than 0).
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::LineIndex;
    /// assert_eq!(LineIndex::new(37).checked_sub(27), Some(LineIndex::new(10)));
    /// assert_eq!(LineIndex::new(27).checked_sub(37), None);
    /// ```
    pub fn checked_sub(&self, rhs: usize) -> Option<LineIndex> {
        let index = self.0;
        index.checked_sub(rhs).map(LineIndex)
    }
}

impl Into<usize> for LineIndex {
    fn into(self) -> usize {
        let LineIndex(index) = self;
        index
    }
}

impl From<LineNumber> for LineIndex {
    fn from(LineNumber(raw_number): LineNumber) -> Self {
        // This is safe, as LineNumber (per invariant) is >= 1
        LineIndex::new(raw_number - 1)
    }
}
impl Add<usize> for LineIndex {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        let raw_index: usize = self.into();
        LineIndex::new(raw_index + rhs)
    }
}
impl AddAssign<usize> for LineIndex {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs;
    }
}
impl Sub<usize> for LineIndex {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self {
        let raw_index: usize = self.into();
        LineIndex::new(raw_index - rhs)
    }
}
impl SubAssign<usize> for LineIndex {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs;
    }
}
impl fmt::Display for LineIndex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// A type for enumerating lines by number (rather than by index), i.e., starting from 1.
/// Conversions between LineNumber and LineIndex are always safe.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Debug, Hash)]
pub struct LineNumber(usize); //Invariant: value is always >= 1
impl LineNumber {
    /// Create a new LineNumber from a raw value.
    ///
    /// # Panics:
    ///
    /// Panics if val is 0 (as line numbers start from 1).
    pub fn new(val: usize) -> Self {
        assert!(val > 0, "Invalid LineNumber: Number == 0");
        LineNumber(val)
    }

    /// Unpack the LineNumber and yield the underlying value.
    pub fn raw_value(self) -> usize {
        self.0
    }

    /// Checked integer subtraction. Computes self - rhs, returning None if the result is invalid
    /// (i.e., smaller than 1).
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::LineNumber;
    /// assert_eq!(LineNumber::new(37).checked_sub(27), Some(LineNumber::new(10)));
    /// assert_eq!(LineNumber::new(27).checked_sub(37), None);
    /// assert_eq!(LineNumber::new(1).checked_sub(1), None);
    /// assert_eq!(LineNumber::new(2).checked_sub(1), Some(LineNumber::new(1)));
    /// ```
    pub fn checked_sub(&self, rhs: usize) -> Option<LineNumber> {
        let index = self.0 - 1; // Safe according to invariant: self.0 >= 1
        index.checked_sub(rhs).map(|i| LineNumber(i + 1))
    }
}

impl Into<usize> for LineNumber {
    fn into(self) -> usize {
        self.0
    }
}
impl From<LineIndex> for LineNumber {
    fn from(LineIndex(raw_index): LineIndex) -> Self {
        LineNumber::new(raw_index + 1)
    }
}
impl Add<usize> for LineNumber {
    type Output = Self;
    fn add(self, rhs: usize) -> Self {
        let raw_number: usize = self.into();
        LineNumber::new(raw_number + rhs)
    }
}
impl AddAssign<usize> for LineNumber {
    fn add_assign(&mut self, rhs: usize) {
        *self = *self + rhs;
    }
}
impl Sub<usize> for LineNumber {
    type Output = Self;
    fn sub(self, rhs: usize) -> Self {
        let raw_number: usize = self.into();
        LineNumber::new(raw_number - rhs)
    }
}
impl SubAssign<usize> for LineNumber {
    fn sub_assign(&mut self, rhs: usize) {
        *self = *self - rhs;
    }
}
impl fmt::Display for LineNumber {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}
