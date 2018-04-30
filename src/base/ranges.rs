//! Copy of [unstable traits and
//! types](https://doc.rust-lang.org/std/collections/range/trait.RangeArgument.html)
//!
//! These will be removed once [#30877](https://github.com/rust-lang/rust/issues/30877) is
//! stabilized.
use std::ops::{Range, RangeFrom, RangeFull, RangeTo};

pub enum Bound<T> {
    Unbound,
    Inclusive(T),
    Exclusive(T),
}
pub trait RangeArgument<T> {
    fn start(&self) -> Bound<T>;
    fn end(&self) -> Bound<T>;
}

impl<T: Copy> RangeArgument<T> for Range<T> {
    fn start(&self) -> Bound<T> {
        Bound::Inclusive(self.start)
    }
    fn end(&self) -> Bound<T> {
        Bound::Exclusive(self.end)
    }
}
impl<T: Copy> RangeArgument<T> for RangeFrom<T> {
    fn start(&self) -> Bound<T> {
        Bound::Inclusive(self.start)
    }
    fn end(&self) -> Bound<T> {
        Bound::Unbound
    }
}
impl<T: Copy> RangeArgument<T> for RangeTo<T> {
    fn start(&self) -> Bound<T> {
        Bound::Unbound
    }
    fn end(&self) -> Bound<T> {
        Bound::Exclusive(self.end)
    }
}
impl<T> RangeArgument<T> for RangeFull {
    fn start(&self) -> Bound<T> {
        Bound::Unbound
    }
    fn end(&self) -> Bound<T> {
        Bound::Unbound
    }
}
