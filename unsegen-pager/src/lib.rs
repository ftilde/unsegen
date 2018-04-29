extern crate syntect;
extern crate unsegen;

mod highlighting;
mod decorating;

pub use highlighting::*;
pub use decorating::*;

pub use syntect::highlighting::{Theme, ThemeSet};
pub use syntect::parsing::{SyntaxDefinition, SyntaxSet};

use unsegen::base::{Cursor, GraphemeCluster, ModifyMode, StyleModifier, Window, WrappingMode,
                    basic_types::*, ranges::*};
use unsegen::widget::{layout_linearly, Demand, Demand2D, RenderingHints, Widget};
use unsegen::input::{OperationResult, Scrollable};

use std::cmp::{max, min};

pub trait PagerLine {
    fn get_content(&self) -> &str;
}

impl PagerLine for String {
    fn get_content(&self) -> &str {
        self.as_str()
    }
}

// PagerContent ---------------------------------------------------------------
pub struct PagerContent<L: PagerLine, D: LineDecorator> {
    storage: Vec<L>,
    highlight_info: HighlightInfo,
    pub decorator: D,
}

impl<L: PagerLine> PagerContent<L, NoDecorator<L>> {
    pub fn from_lines(storage: Vec<L>) -> Self {
        PagerContent {
            storage: storage,
            highlight_info: HighlightInfo::none(),
            decorator: NoDecorator::default(),
        }
    }
}

impl PagerContent<String, NoDecorator<String>> {
    pub fn from_file<F: AsRef<::std::path::Path>>(file_path: F) -> ::std::io::Result<Self> {
        use std::io::Read;
        let mut file = ::std::fs::File::open(file_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        Ok(PagerContent {
            storage: contents.lines().map(|s| s.to_owned()).collect::<Vec<_>>(),
            highlight_info: HighlightInfo::none(),
            decorator: NoDecorator::default(),
        })
    }
}

impl<L, D> PagerContent<L, D>
where
    L: PagerLine,
    D: LineDecorator<Line = L>,
{
    pub fn with_highlighter<HN: Highlighter>(self, highlighter: HN) -> PagerContent<L, D> {
        let highlight_info = highlighter.highlight(self.storage.iter().map(|l| l as &PagerLine));
        PagerContent {
            storage: self.storage,
            highlight_info: highlight_info,
            decorator: self.decorator,
        }
    }
}

impl<L> PagerContent<L, NoDecorator<L>>
where
    L: PagerLine,
{
    pub fn with_decorator<DN: LineDecorator<Line = L>>(self, decorator: DN) -> PagerContent<L, DN> {
        PagerContent {
            storage: self.storage,
            highlight_info: self.highlight_info,
            decorator: decorator,
        }
    }
}

impl<L, D> PagerContent<L, D>
where
    L: PagerLine,
    D: LineDecorator<Line = L>,
{
    pub fn view<'a, I: Into<LineIndex>, R: RangeArgument<I>>(
        &'a self,
        range: R,
    ) -> Box<DoubleEndedIterator<Item = (LineIndex, &'a L)> + 'a>
    where
        Self: ::std::marker::Sized,
    {
        // Not exactly sure, why this is needed... we only store a reference?!
        let start: LineIndex = match range.start() {
            // Always inclusive
            Bound::Unbound => LineIndex::new(0),
            Bound::Inclusive(i) => i.into(),
            Bound::Exclusive(i) => i.into() + 1,
        };
        let end: LineIndex = match range.end() {
            // Always exclusive
            Bound::Unbound => LineIndex::new(self.storage.len()),
            Bound::Inclusive(i) => i.into() + 1,
            Bound::Exclusive(i) => i.into(),
        };
        let ustart = start.raw_value();
        let uend = self.storage.len().min(end.raw_value());
        let urange = ustart..uend;
        Box::new(
            urange
                .clone()
                .into_iter()
                .zip(self.storage[urange].iter())
                .map(|(i, l)| (LineIndex::new(i), l)),
        )
    }

    pub fn view_line<I: Into<LineIndex>>(&self, line: I) -> Option<&L> {
        self.storage.get(line.into().raw_value())
    }
}

#[derive(Debug)]
pub enum PagerError {
    NoLineWithIndex(LineIndex),
    NoLineWithPredicate,
    NoContent,
}

// Pager -----------------------------------------------------------------------

pub struct Pager<L, D = NoDecorator<L>>
where
    L: PagerLine,
    D: LineDecorator,
{
    pub content: Option<PagerContent<L, D>>,
    current_line: LineIndex,
}

impl<L, D> Pager<L, D>
where
    L: PagerLine,
    D: LineDecorator<Line = L>,
{
    pub fn new() -> Self {
        Pager {
            content: None,
            current_line: LineIndex::new(0),
        }
    }

    pub fn load(&mut self, content: PagerContent<L, D>) {
        self.content = Some(content);

        // Go back to last available line
        let mut new_current_line = self.current_line;
        while !self.line_exists(new_current_line) {
            new_current_line -= 1;
        }
        self.current_line = new_current_line;
    }

    fn line_exists<I: Into<LineIndex>>(&mut self, line: I) -> bool {
        let line: LineIndex = line.into();
        if let Some(ref mut content) = self.content {
            line.raw_value() < content.storage.len()
        } else {
            false
        }
    }

    pub fn go_to_line<I: Into<LineIndex>>(&mut self, line: I) -> Result<(), PagerError> {
        let line: LineIndex = line.into();
        if self.line_exists(line) {
            self.current_line = line;
            Ok(())
        } else {
            Err(PagerError::NoLineWithIndex(line))
        }
    }

    pub fn go_to_line_if<F: Fn(LineIndex, &L) -> bool>(
        &mut self,
        predicate: F,
    ) -> Result<(), PagerError> {
        let line = if let Some(ref mut content) = self.content {
            content
                .view(LineIndex::new(0)..)
                .find(|&(index, ref line)| predicate(index.into(), line))
                .map(|(index, _)| index)
                .ok_or(PagerError::NoLineWithPredicate)
        } else {
            Err(PagerError::NoContent)
        };
        line.and_then(|index| self.go_to_line(index))
    }

    pub fn current_line_index(&self) -> LineIndex {
        self.current_line
    }

    pub fn current_line(&self) -> Option<&L> {
        if let Some(ref content) = self.content {
            content.storage.get(self.current_line_index().raw_value())
        } else {
            None
        }
    }
}

impl<L, D> Widget for Pager<L, D>
where
    L: PagerLine,
    D: LineDecorator<Line = L>,
{
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(1),
            height: Demand::at_least(1),
        }
    }
    fn draw(&self, window: Window, _: RenderingHints) {
        if let Some(ref content) = self.content {
            let height: Height = window.get_height();
            // The highlighter might need a minimum number of lines to figure out the syntax:
            // TODO: make this configurable?
            let min_highlight_context = 40;
            let num_adjacent_lines_to_load = max(height.into(), min_highlight_context / 2);
            let min_line = self.current_line
                .checked_sub(num_adjacent_lines_to_load)
                .unwrap_or(LineIndex::new(0));
            let max_line = self.current_line + num_adjacent_lines_to_load;

            // Split window
            let decorator_demand = content
                .decorator
                .horizontal_space_demand(content.view(min_line..max_line));
            let split_pos = layout_linearly(
                window.get_width(),
                Width::new(0).unwrap(),
                &[decorator_demand, Demand::at_least(1)],
            )[0];

            let (mut decoration_window, mut content_window) = window
                .split_h(split_pos.from_origin())
                .expect("valid split pos");

            // Fill background with correct color
            let bg_style = content.highlight_info.default_style();
            content_window.set_default_style(bg_style.apply_to_default());
            content_window.fill(GraphemeCluster::space());

            let mut cursor = Cursor::new(&mut content_window)
                .position(ColIndex::new(0), RowIndex::new(0))
                .wrapping_mode(WrappingMode::Wrap);

            let num_line_wraps_until_current_line = {
                content
                    .view(min_line..self.current_line)
                    .map(|(_, line)| (cursor.num_expected_wraps(line.get_content()) + 1) as i32)
                    .sum::<i32>()
            };
            let num_line_wraps_from_current_line = {
                content
                    .view(self.current_line..max_line)
                    .map(|(_, line)| (cursor.num_expected_wraps(line.get_content()) + 1) as i32)
                    .sum::<i32>()
            };

            let centered_current_line_start_pos: RowIndex = (height / (2 as usize)).from_origin();
            let best_current_line_pos_for_bottom = max(
                centered_current_line_start_pos,
                height.from_origin() - num_line_wraps_from_current_line,
            );
            let required_start_pos = min(
                RowIndex::new(0),
                best_current_line_pos_for_bottom - num_line_wraps_until_current_line,
            );

            cursor.set_position(ColIndex::new(0), required_start_pos);

            for (line_index, line) in content.view(min_line..max_line) {
                let line_content = line.get_content();
                let base_style = if line_index == self.current_line {
                    StyleModifier::new().invert(ModifyMode::Toggle).bold(true)
                } else {
                    StyleModifier::none()
                };

                let (_, start_y) = cursor.get_position();
                let mut last_change_pos = 0;
                for &(change_pos, style) in content.highlight_info.get_info_for_line(line_index) {
                    cursor.write(&line_content[last_change_pos..change_pos]);

                    cursor.set_style_modifier(style.on_top_of(&base_style));
                    last_change_pos = change_pos;
                }
                cursor.write(&line_content[last_change_pos..]);

                cursor.set_style_modifier(base_style);
                cursor.fill_and_wrap_line();
                let (_, end_y) = cursor.get_position();

                let range_start_y = min(max(start_y, RowIndex::new(0)), height.from_origin());
                let range_end_y = min(max(end_y, RowIndex::new(0)), height.from_origin());
                content.decorator.decorate(
                    &line,
                    line_index,
                    self.current_line,
                    decoration_window.create_subwindow(.., range_start_y..range_end_y),
                );
                //decoration_window.create_subwindow(.., range_start_y..range_end_y).fill('X');
            }
        }
    }
}
impl<L, D> Scrollable for Pager<L, D>
where
    L: PagerLine,
    D: LineDecorator<Line = L>,
{
    fn scroll_backwards(&mut self) -> OperationResult {
        if self.current_line > LineIndex::new(0) {
            self.current_line -= 1;
            Ok(())
        } else {
            Err(())
        }
    }
    fn scroll_forwards(&mut self) -> OperationResult {
        let new_line = self.current_line + 1;
        self.go_to_line(new_line).map_err(|_| ())
    }
    fn scroll_to_beginning(&mut self) -> OperationResult {
        if self.current_line == LineIndex::new(0) {
            Err(())
        } else {
            self.current_line = LineIndex::new(0);
            Ok(())
        }
    }
    fn scroll_to_end(&mut self) -> OperationResult {
        if let Some(ref content) = self.content {
            if content.storage.is_empty() {
                return Err(());
            }
            let last_line = LineIndex::new(content.storage.len()-1);
            if self.current_line == last_line {
                Err(())
            } else {
                self.current_line = last_line;
                Ok(())
            }
        } else {
            Err(())
        }
    }
}
