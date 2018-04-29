use unsegen::widget::{text_width, ColDemand, Demand};
use unsegen::base::{Cursor, Window};
use unsegen::base::basic_types::*;

use super::PagerLine;

pub trait LineDecorator {
    type Line: PagerLine;
    fn horizontal_space_demand<'a, 'b: 'a>(
        &'a self,
        lines: Box<DoubleEndedIterator<Item = (LineIndex, &'b Self::Line)> + 'b>,
    ) -> ColDemand;
    fn decorate(
        &self,
        line: &Self::Line,
        line_to_decorate_index: LineIndex,
        active_line_index: LineIndex,
        window: Window,
    );
}

pub struct NoDecorator<L> {
    _dummy: ::std::marker::PhantomData<L>,
}

impl<L> Default for NoDecorator<L> {
    fn default() -> Self {
        NoDecorator {
            _dummy: Default::default(),
        }
    }
}

impl<L: PagerLine> LineDecorator for NoDecorator<L> {
    type Line = L;
    fn horizontal_space_demand<'a, 'b: 'a>(
        &'a self,
        _: Box<DoubleEndedIterator<Item = (LineIndex, &'b Self::Line)> + 'b>,
    ) -> ColDemand {
        Demand::exact(0)
    }
    fn decorate(&self, _: &L, _: LineIndex, _: LineIndex, _: Window) {}
}

pub struct LineNumberDecorator<L> {
    _dummy: ::std::marker::PhantomData<L>,
}

impl<L> Default for LineNumberDecorator<L> {
    fn default() -> Self {
        LineNumberDecorator {
            _dummy: Default::default(),
        }
    }
}

impl<L: PagerLine> LineDecorator for LineNumberDecorator<L> {
    type Line = L;
    fn horizontal_space_demand<'a, 'b: 'a>(
        &'a self,
        lines: Box<DoubleEndedIterator<Item = (LineIndex, &'b Self::Line)> + 'b>,
    ) -> ColDemand {
        let max_space = lines
            .last()
            .map(|(i, _)| text_width(format!(" {} ", i).as_str()))
            .unwrap_or(0);
        Demand::from_to(0, max_space)
    }
    fn decorate(&self, _: &L, line_to_decorate_index: LineIndex, _: LineIndex, mut window: Window) {
        let width = (window.get_width() - 2).positive_or_zero();
        let line_number = LineNumber::from(line_to_decorate_index);
        let mut cursor = Cursor::new(&mut window).position(ColIndex::new(0), RowIndex::new(0));

        use std::fmt::Write;
        write!(cursor, " {:width$} ", line_number, width = width.into()).unwrap();
    }
}
