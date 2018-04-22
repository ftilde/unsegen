extern crate syntect;
extern crate unsegen;

mod highlighting;
mod decorating;

pub use highlighting::*;
pub use decorating::*;

use unsegen::base::{Cursor, GraphemeCluster, ModifyMode, StyleModifier, Window, WrappingMode,
                    basic_types::*};
use unsegen::widget::{layout_linearly, Demand, Demand2D, LineIndex, LineStorage, RenderingHints,
                      Widget};
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
pub struct PagerContent<S: LineStorage, H: Highlighter, D: LineDecorator> {
    pub storage: S,
    highlighter: H,
    pub decorator: D,
}

impl<S> PagerContent<S, NoHighlighter, NoDecorator<S::Line>>
where
    S: LineStorage,
    S::Line: PagerLine,
{
    pub fn create(storage: S) -> Self {
        PagerContent {
            storage: storage,
            highlighter: NoHighlighter,
            decorator: NoDecorator::default(),
        }
    }
}

impl<S, D> PagerContent<S, NoHighlighter, D>
where
    S: LineStorage,
    S::Line: PagerLine,
    D: LineDecorator<Line = S::Line>,
{
    pub fn with_highlighter<HN: Highlighter>(self, highlighter: HN) -> PagerContent<S, HN, D> {
        PagerContent {
            storage: self.storage,
            highlighter: highlighter,
            decorator: self.decorator,
        }
    }
}

impl<S, H> PagerContent<S, H, NoDecorator<S::Line>>
where
    S: LineStorage,
    S::Line: PagerLine,
    H: Highlighter,
{
    pub fn with_decorator<DN: LineDecorator<Line = S::Line>>(
        self,
        decorator: DN,
    ) -> PagerContent<S, H, DN> {
        PagerContent {
            storage: self.storage,
            highlighter: self.highlighter,
            decorator: decorator,
        }
    }
}

#[derive(Debug)]
pub enum PagerError {
    NoLineWithIndex(LineIndex),
    NoLineWithPredicate,
    NoContent,
}

// Pager -----------------------------------------------------------------------

pub struct Pager<S, H = NoHighlighter, D = NoDecorator<<S as LineStorage>::Line>>
where
    S: LineStorage,
    D: LineDecorator,
    H: Highlighter,
{
    pub content: Option<PagerContent<S, H, D>>,
    current_line: LineIndex,
}

impl<S, H, D> Pager<S, H, D>
where
    S: LineStorage,
    S::Line: PagerLine,
    D: LineDecorator<Line = S::Line>,
    H: Highlighter,
{
    pub fn new() -> Self {
        Pager {
            content: None,
            current_line: LineIndex(0),
        }
    }

    pub fn load(&mut self, content: PagerContent<S, H, D>) {
        self.content = Some(content);

        // Go back to last available line
        let mut new_current_line = self.current_line;
        while !self.line_exists(new_current_line) {
            new_current_line -= 1;
        }
        self.current_line = new_current_line;
    }
    fn line_exists<L: Into<LineIndex>>(&mut self, line: L) -> bool {
        let line: LineIndex = line.into();
        if let Some(ref mut content) = self.content {
            content.storage.view(line..(line + 1)).next().is_some()
        } else {
            false
        }
    }

    pub fn go_to_line<L: Into<LineIndex>>(&mut self, line: L) -> Result<(), PagerError> {
        let line: LineIndex = line.into();
        if self.line_exists(line) {
            self.current_line = line;
            Ok(())
        } else {
            Err(PagerError::NoLineWithIndex(line))
        }
    }

    pub fn go_to_line_if<F: Fn(LineIndex, &S::Line) -> bool>(
        &mut self,
        predicate: F,
    ) -> Result<(), PagerError> {
        let line = if let Some(ref mut content) = self.content {
            content
                .storage
                .view(LineIndex(0)..)
                .find(|&(index, ref line)| predicate(index.into(), line))
                .ok_or(PagerError::NoLineWithPredicate)
        } else {
            Err(PagerError::NoContent)
        };
        line.and_then(|(index, _)| self.go_to_line(index))
    }

    pub fn current_line_index(&self) -> LineIndex {
        self.current_line
    }

    pub fn current_line(&self) -> Option<S::Line> {
        if let Some(ref content) = self.content {
            content.storage.view_line(self.current_line_index())
        } else {
            None
        }
    }
}

impl<S, H, D> Widget for Pager<S, H, D>
where
    S: LineStorage,
    S::Line: PagerLine,
    H: Highlighter,
    D: LineDecorator<Line = S::Line>,
{
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(1),
            height: Demand::at_least(1),
        }
    }
    fn draw(&self, window: Window, _: RenderingHints) {
        if let Some(ref content) = self.content {
            let mut highlighter = content.highlighter.create_instance();
            let height: Height = window.get_height();
            // The highlighter might need a minimum number of lines to figure out the syntax:
            // TODO: make this configurable?
            let min_highlight_context = 40;
            let num_adjacent_lines_to_load = max(height.into(), min_highlight_context / 2);
            let min_line = self.current_line
                .checked_sub(num_adjacent_lines_to_load)
                .unwrap_or(LineIndex(0));
            let max_line = self.current_line + num_adjacent_lines_to_load;

            // Split window
            let decorator_demand = content
                .decorator
                .horizontal_space_demand(content.storage.view(min_line..max_line));
            let split_pos = layout_linearly(
                window.get_width(),
                Width::new(0).unwrap(),
                &[decorator_demand, Demand::at_least(1)],
            )[0];

            let (mut decoration_window, mut content_window) = window
                .split_h(split_pos.from_origin())
                .expect("valid split pos");

            // Fill background with correct color
            let bg_style = highlighter.default_style();
            content_window.set_default_style(bg_style.apply_to_default());
            content_window.fill(GraphemeCluster::space());

            let mut cursor = Cursor::new(&mut content_window)
                .position(ColIndex::new(0), RowIndex::new(0))
                .wrapping_mode(WrappingMode::Wrap);

            let num_line_wraps_until_current_line = {
                content
                    .storage
                    .view(min_line..self.current_line)
                    .map(|(_, line)| (cursor.num_expected_wraps(line.get_content()) + 1) as i32)
                    .sum::<i32>()
            };
            let num_line_wraps_from_current_line = {
                content
                    .storage
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

            for (line_index, line) in content.storage.view(min_line..max_line) {
                let base_style = if line_index == self.current_line {
                    StyleModifier::new().invert(ModifyMode::Toggle).bold(true)
                } else {
                    StyleModifier::none()
                };

                let (_, start_y) = cursor.get_position();
                for (style, region) in highlighter.highlight(line.get_content()) {
                    cursor.set_style_modifier(style.on_top_of(&base_style));
                    cursor.write(&region);
                }
                cursor.set_style_modifier(base_style);
                cursor.fill_and_wrap_line();
                let (_, end_y) = cursor.get_position();

                let range_start_y = min(max(start_y, RowIndex::new(0)), height.from_origin());
                let range_end_y = min(max(end_y, RowIndex::new(0)), height.from_origin());
                content.decorator.decorate(
                    &line,
                    line_index,
                    decoration_window.create_subwindow(.., range_start_y..range_end_y),
                );
                //decoration_window.create_subwindow(.., range_start_y..range_end_y).fill('X');
            }
        }
    }
}
impl<S, H, D> Scrollable for Pager<S, H, D>
where
    S: LineStorage,
    S::Line: PagerLine,
    H: Highlighter,
    D: LineDecorator<Line = S::Line>,
{
    fn scroll_backwards(&mut self) -> OperationResult {
        if self.current_line > LineIndex(0) {
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
        if self.current_line == LineIndex(0) {
            Err(())
        } else {
            self.current_line = LineIndex(0);
            Ok(())
        }
    }
    // Using default implementation for now. We could try to do something different (but more
    // complicated) if performance is an issue
    //fn scroll_to_end(&mut self) -> OperationResult {
    //}
}
