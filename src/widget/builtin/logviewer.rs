//! A scrollable, append-only buffer of lines.
use base::basic_types::*;
use base::{Cursor, Window, WrappingMode};
use input::{OperationResult, Scrollable};
use std::fmt;
use std::ops::Range;
use widget::{Demand, Demand2D, RenderingHints, Widget};

/// A scrollable, append-only buffer of lines.
pub struct LogViewer {
    storage: Vec<String>, // Invariant: always holds at least one line, does not contain newlines
    scrollback_position: Option<LineIndex>,
    scroll_step: usize,
}

impl LogViewer {
    /// Create an empty `LogViewer`. Add lines by writing to the viewer as `std::io::Write`.
    pub fn new() -> Self {
        let mut storage = Vec::new();
        storage.push(String::new()); //Fullfil invariant (at least one line)
        LogViewer {
            storage: storage,
            scrollback_position: None,
            scroll_step: 1,
        }
    }

    fn num_lines_stored(&self) -> usize {
        self.storage.len() // Per invariant: no newlines in storage
    }

    fn current_line_index(&self) -> LineIndex {
        self.scrollback_position.unwrap_or(LineIndex::new(
            self.num_lines_stored().checked_sub(1).unwrap_or(0),
        ))
    }

    /// Note: Do not insert newlines into the string using this
    fn active_line_mut(&mut self) -> &mut String {
        self.storage
            .last_mut()
            .expect("Invariant: At least one line")
    }

    fn view(&self, range: Range<LineIndex>) -> &[String] {
        &self.storage[range.start.raw_value()..range.end.raw_value()]
    }

    pub fn as_widget<'a>(&'a self) -> impl Widget + 'a {
        LogViewerWidget { inner: self }
    }
}

impl fmt::Write for LogViewer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut s = s.to_owned();

        while let Some(newline_offset) = s.find('\n') {
            let mut line: String = s.drain(..(newline_offset + 1)).collect();
            line.pop(); //Remove the \n
            self.active_line_mut().push_str(&line);
            self.storage.push(String::new());
        }
        self.active_line_mut().push_str(&s);
        Ok(())
    }
}

impl Scrollable for LogViewer {
    fn scroll_forwards(&mut self) -> OperationResult {
        let current = self.current_line_index();
        let candidate = current + self.scroll_step;
        self.scrollback_position = if candidate.raw_value() < self.num_lines_stored() {
            Some(candidate)
        } else {
            None
        };
        if self.scrollback_position.is_some() {
            Ok(())
        } else {
            Err(())
        }
    }
    fn scroll_backwards(&mut self) -> OperationResult {
        let current = self.current_line_index();
        let op_res = if current.raw_value() != 0 {
            Ok(())
        } else {
            Err(())
        };
        self.scrollback_position = Some(
            current
                .checked_sub(self.scroll_step)
                .unwrap_or(LineIndex::new(0)),
        );
        op_res
    }
    fn scroll_to_beginning(&mut self) -> OperationResult {
        if Some(LineIndex::new(0)) == self.scrollback_position {
            Err(())
        } else {
            self.scrollback_position = Some(LineIndex::new(0));
            Ok(())
        }
    }
    fn scroll_to_end(&mut self) -> OperationResult {
        if self.scrollback_position.is_none() {
            Err(())
        } else {
            self.scrollback_position = None;
            Ok(())
        }
    }
}

struct LogViewerWidget<'a> {
    inner: &'a LogViewer,
}

impl<'a> Widget for LogViewerWidget<'a> {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(1),
            height: Demand::at_least(1),
        }
    }
    fn draw(&self, mut window: Window, _: RenderingHints) {
        let height = window.get_height();
        if height == 0 {
            return;
        }

        // TODO: This does not work well when lines are wrapped, but we may want scrolling farther
        // than 1 line per event
        // self.scroll_step = ::std::cmp::max(1, height.checked_sub(1).unwrap_or(1));

        let y_start = height - 1;
        let mut cursor = Cursor::new(&mut window)
            .position(ColIndex::new(0), y_start.from_origin())
            .wrapping_mode(WrappingMode::Wrap);
        let end_line = self.inner.current_line_index();
        let start_line =
            LineIndex::new(end_line.raw_value().checked_sub(height.into()).unwrap_or(0));
        for line in self.inner.view(start_line..(end_line + 1)).iter().rev() {
            let num_auto_wraps = cursor.num_expected_wraps(&line) as i32;
            cursor.move_by(ColDiff::new(0), RowDiff::new(-num_auto_wraps));
            cursor.writeln(&line);
            cursor.move_by(ColDiff::new(0), RowDiff::new(-num_auto_wraps) - 2);
        }
    }
}
