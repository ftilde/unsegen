use unsegen::base::{
    Cursor,
    CursorState,
    CursorTarget,
    ModifyMode,
    Style,
    StyleModifier,
    StyledGraphemeCluster,
    Window,
    WrappingMode,
    UNBOUNDED_EXTENT,
};
use unsegen::base::Color as UColor;
use unsegen::widget::{
    Demand,
    Demand2D,
    RenderingHints,
};
use unsegen::input::{
    Scrollable,
    OperationResult,
};
use ansi;
use ansi::{
    Attr,
    CursorStyle,
    Handler,
    TermInfo,
};

use std::fmt::Write;
use index;
use std::cmp::{
    min,
    max,
};

#[derive(Clone)]
struct Line {
    content: Vec<StyledGraphemeCluster>,
}

impl Line {
    fn empty() -> Self {
        Line {
            content: Vec::new(),
        }
    }

    fn length(&self) -> u32 {
        self.content.len() as u32
    }

    fn clear(&mut self) {
        self.content.clear();
    }

    fn height_for_width(&self, width: u32) -> u32 {
        //TODO: this might not be correct if there are wide clusters within the content, hmm...
        self.length().checked_sub(1).unwrap_or(0) as u32 / width + 1
    }

    fn get_cell_mut(&mut self, x: u32) -> Option<&mut StyledGraphemeCluster> {
        // Grow horizontally to desired position
        let missing_elements = (x as usize+ 1).checked_sub(self.content.len()).unwrap_or(0);
        self.content.extend(::std::iter::repeat(StyledGraphemeCluster::default()).take(missing_elements));

        let element = self.content.get_mut(x as usize).expect("element existent assured previously");
        Some(element)
    }

    fn get_cell(&self, x: u32) -> Option<&StyledGraphemeCluster> {
        /*
        //TODO: maybe we want to grow? problems with mutability...
        // Grow horizontally to desired position
        let missing_elements = (x as usize+ 1).checked_sub(self.content.len()).unwrap_or(0);
        self.content.extend(::std::iter::repeat(StyledGraphemeCluster::default()).take(missing_elements));
        */

        let element = self.content.get(x as usize).expect("element existent assured previously");
        Some(element)
    }
}

struct LineBuffer {
    lines: Vec<Line>,
    window_width: u32,
    default_style: Style,
}
impl LineBuffer {
    pub fn new() -> Self {
        LineBuffer {
            lines: Vec::new(),
            window_width: 0,
            default_style: Style::default(),
        }
    }

    fn height_as_displayed(&self) -> u32 {
        self.lines.iter().map(|l| l.height_for_width(self.window_width)).sum()
    }

    pub fn set_window_width(&mut self, w: u32) {
        self.window_width = w;
    }
}

impl CursorTarget for LineBuffer {
    fn get_width(&self) -> u32 {
        UNBOUNDED_EXTENT
    }
    fn get_soft_width(&self) -> u32 {
        self.window_width
    }
    fn get_height(&self) -> u32 {
        UNBOUNDED_EXTENT
    }
    fn get_cell_mut(&mut self, x: u32, y: u32) -> Option<&mut StyledGraphemeCluster> {
        // Grow vertically to desired position
        let missing_elements = (y as usize + 1).checked_sub(self.lines.len()).unwrap_or(0);
        self.lines.extend(::std::iter::repeat(Line::empty()).take(missing_elements));

        let line = self.lines.get_mut(y as usize).expect("line existence assured previously");

        line.get_cell_mut(x)
    }
    fn get_cell(&self, x: u32, y: u32) -> Option<&StyledGraphemeCluster> {
        /*
        //TODO: maybe we want to grow? problems with mutability...
        // Grow vertically to desired position
        let missing_elements = (y as usize + 1).checked_sub(self.lines.len()).unwrap_or(0);
        self.lines.extend(::std::iter::repeat(Line::empty()).take(missing_elements));
        */

        let line = self.lines.get(y as usize).expect("line existence assured previously");

        line.get_cell(x)
    }
    fn get_default_style(&self) -> &Style {
        &self.default_style
    }
}

pub struct TerminalWindow {
    window_width: u32,
    window_height: u32,
    buffer: LineBuffer,
    cursor_state: CursorState,
    scrollback_position: Option<u32>,
    scroll_step: u32,

    // Terminal state
    show_cursor: bool,
}

impl TerminalWindow {
    pub fn new() -> Self {
        TerminalWindow  {
            window_width: 0,
            window_height: 0,
            buffer: LineBuffer::new(),
            cursor_state: CursorState::default(),
            scrollback_position: None,
            scroll_step: 1,

            show_cursor: true,
        }
    }

    // position of the first (displayed) row of the buffer that will NOT be displayed
    fn current_scrollback_pos(&self) -> u32 {
        self.scrollback_position.unwrap_or(self.buffer.height_as_displayed())
    }

    pub fn set_width(&mut self, w: u32) {
        self.window_width = w;
        self.buffer.set_window_width(w);
    }

    pub fn set_height(&mut self, h: u32) {
        self.window_height = h;
    }

    pub fn get_width(&self) -> u32 {
        self.window_width
    }

    pub fn get_height(&self) -> u32 {
        self.window_height
    }

    fn with_cursor<F: FnOnce(&mut Cursor<LineBuffer>)>(&mut self, f: F) {
        let mut state = CursorState::default();
        ::std::mem::swap(&mut state, &mut self.cursor_state);
        let mut cursor = Cursor::with_state(&mut self.buffer, state);
        f(&mut cursor);
        self.cursor_state = cursor.into_state();
    }

    fn line_to_buffer_pos_y(&self, line: index::Line) -> i32 {
        max(0, self.buffer.lines.len() as i32 - self.window_height as i32) + line.0 as i32
    }
    fn col_to_buffer_pos_x(&self, col: index::Column) -> i32 {
        col.0 as i32
    }

    pub fn space_demand(&self) -> Demand2D {
        // at_least => We can grow if there is space
        Demand2D {
            width: Demand::at_least(self.window_width as u32),
            height: Demand::at_least(self.window_height as u32),
        }
    }

    pub fn draw(&mut self, mut window: Window, _: RenderingHints) {
        //temporarily change buffer to show cursor:
        if self.show_cursor {
            self.with_cursor(|cursor| {
                if let Some(cell) = cursor.get_current_cell_mut() {
                    StyleModifier::new().invert(ModifyMode::Toggle).modify(&mut cell.style);
                }
            });
        }

        let height = window.get_height();
        let width = window.get_width();

        if height == 0 || width == 0 || self.buffer.lines.is_empty() {
            return;
        }

        let scrollback_offset = (self.buffer.height_as_displayed() - self.current_scrollback_pos()) as i32;
        let minimum_y_start = height as i32 + scrollback_offset;
        let start_line = self.buffer.lines.len().checked_sub(minimum_y_start as usize).unwrap_or(0);
        let line_range = start_line..;
        let y_start = min(0, minimum_y_start - self.buffer.lines[line_range.clone()].iter().map(|line| line.height_for_width(width)).sum::<u32>() as i32);
        let mut cursor = Cursor::new(&mut window)
            .position(0, y_start as i32)
            .wrapping_mode(WrappingMode::Wrap);
        for line in self.buffer.lines[line_range].iter() {
            cursor.write_preformatted(line.content.as_slice());
            cursor.wrap_line();
        }

        //revert cursor change
        if self.show_cursor {
            self.with_cursor(|cursor| {
                if let Some(cell) = cursor.get_current_cell_mut() {
                    StyleModifier::new().invert(ModifyMode::Toggle).modify(&mut cell.style);
                }
            });
        }
    }
}

fn ansi_to_unsegen_color(ansi_color: ansi::Color) -> UColor {
    match ansi_color {
        ansi::Color::Named(c) => match c {
            ansi::NamedColor::Black => UColor::Black,
            ansi::NamedColor::Red => UColor::Red,
            ansi::NamedColor::Green => UColor::Green,
            ansi::NamedColor::Yellow => UColor::Yellow,
            ansi::NamedColor::Blue => UColor::Blue,
            ansi::NamedColor::Magenta => UColor::Magenta,
            ansi::NamedColor::Cyan => UColor::Cyan,
            ansi::NamedColor::White => UColor::White,
            ansi::NamedColor::BrightBlack => UColor::LightBlack,
            ansi::NamedColor::BrightRed => UColor::LightRed,
            ansi::NamedColor::BrightGreen => UColor::LightGreen,
            ansi::NamedColor::BrightYellow => UColor::LightYellow,
            ansi::NamedColor::BrightBlue => UColor::LightBlue,
            ansi::NamedColor::BrightMagenta => UColor::LightMagenta,
            ansi::NamedColor::BrightCyan => UColor::LightCyan,
            ansi::NamedColor::BrightWhite => UColor::LightWhite,
            ansi::NamedColor::Foreground => UColor::White, //??
            ansi::NamedColor::Background => UColor::Black, //??
            ansi::NamedColor::CursorText =>  {
                // This is kind of tricky to get...
                UColor::Black
            },
            ansi::NamedColor::Cursor => {
                // This is kind of tricky to get...
                UColor::Black
            },
            // Also not sure what to do here
            ansi::NamedColor::DimBlack => UColor::Black,
            ansi::NamedColor::DimRed => UColor::Red,
            ansi::NamedColor::DimGreen => UColor::Green,
            ansi::NamedColor::DimYellow => UColor::Yellow,
            ansi::NamedColor::DimBlue => UColor::Blue,
            ansi::NamedColor::DimMagenta => UColor::Magenta,
            ansi::NamedColor::DimCyan => UColor::Cyan,
            ansi::NamedColor::DimWhite => UColor::White,
        },
        ansi::Color::Spec(c) => {
            UColor::Rgb{r: c.r, g: c.g, b: c.b}
        },
        ansi::Color::Indexed(c) => {
            //TODO: We might in the future implement a separate color table, but for new we "reuse"
            //the table of the underlying terminal:
            UColor::Ansi(c)
        },
    }
}

macro_rules! warn_unimplemented {
    ($($arg:tt)*) => {{
        use std::io::Write;
        (write!(&mut ::std::io::stderr(), "WARN: Unimplemented ansi function \"")).expect("stderr");
        (write!(&mut ::std::io::stderr(), $($arg)*)).expect("stderr");
        (writeln!(&mut ::std::io::stderr(), "\"")).expect("stderr");
    }}
}

macro_rules! trace_ansi {
    ($($arg:tt)*) => {{
        /*
        use std::io::Write;
        (write!(&mut ::std::io::stderr(), "INFO: Ansi trace: ")).expect("stderr");
        (writeln!(&mut ::std::io::stderr(), $($arg)*)).expect("stderr");
        */
    }}
}


impl Handler for TerminalWindow {

    /// OSC to set window title
    fn set_title(&mut self, _: &str) {
        //TODO: (Although this might not make sense to implement. Do we want to display a title?)
    }

    /// Set the cursor style
    fn set_cursor_style(&mut self, _: CursorStyle) {
        //TODO
        warn_unimplemented!("set_cursor_style");
    }

    /// A character to be displayed
    fn input(&mut self, c: char) {
        self.with_cursor(|cursor| {
            write!(cursor, "{}", c).unwrap();
        });
        trace_ansi!("input '{}'", c);
    }

    /// Set cursor to position
    fn goto(&mut self, line: index::Line, col: index::Column) {
        let x = self.col_to_buffer_pos_x(col);
        let y = self.line_to_buffer_pos_y(line);
        self.with_cursor(|cursor| {
            cursor.set_position(x, y);
        });
        trace_ansi!("goto");
    }

    /// Set cursor to specific row
    fn goto_line(&mut self, line: index::Line) {
        let y = self.line_to_buffer_pos_y(line);
        self.with_cursor(|cursor| {
            cursor.move_to_y(y);
        });
        trace_ansi!("goto_line");
    }

    /// Set cursor to specific column
    fn goto_col(&mut self, col: index::Column) {
        let x = self.col_to_buffer_pos_x(col);
        self.with_cursor(|cursor| {
            cursor.move_to_x(x);
        });
        trace_ansi!("goto_col");
    }

    /// Insert blank characters in current line starting from cursor
    fn insert_blank(&mut self, _: index::Column) {
        //TODO
        warn_unimplemented!("insert_blank");
    }

    /// Move cursor up `rows`
    fn move_up(&mut self, line: index::Line) {
        self.with_cursor(|cursor| {
            cursor.move_by(0, -(line.0 as i32));
        });
        trace_ansi!("move_up");
    }

    /// Move cursor down `rows`
    fn move_down(&mut self, line: index::Line) {
        self.with_cursor(|cursor| {
            cursor.move_by(0, line.0 as i32);
        });
        trace_ansi!("move_down");
    }

    /// Identify the terminal (should write back to the pty stream)
    ///
    /// TODO this should probably return an io::Result
    fn identify_terminal<W: ::std::io::Write>(&mut self, _: &mut W) {
        //TODO
        warn_unimplemented!("identify_terminal");
    }

    /// Report device status
    fn device_status<W: ::std::io::Write>(&mut self, _: &mut W, _: usize) {
        //TODO
        warn_unimplemented!("device_status");
    }

    /// Move cursor forward `cols`
    fn move_forward(&mut self, cols: index::Column) {
        self.with_cursor(|cursor| {
            for _ in 0..cols.0 {
                cursor.move_right();
            }
        });
        trace_ansi!("move_forward {}", cols.0);
    }

    /// Move cursor backward `cols`
    fn move_backward(&mut self, cols: index::Column) {
        self.with_cursor(|cursor| {
            for _ in 0..cols.0 {
                cursor.move_left();
            }
        });
        trace_ansi!("move_backward {}", cols.0);
    }

    /// Move cursor down `rows` and set to column 1
    fn move_down_and_cr(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("move_down_and_cr");
    }

    /// Move cursor up `rows` and set to column 1
    fn move_up_and_cr(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("move_up_and_cr");
    }

    /// Put `count` tabs
    fn put_tab(&mut self, count: i64) {
        self.with_cursor(|cursor| {
            for _ in 0..count {
                write!(cursor, "\t").unwrap();
            }
        });
        trace_ansi!("put_tab {}", count);
    }

    /// Backspace `count` characters
    fn backspace(&mut self) {
        self.with_cursor(|cursor| {
            cursor.move_left();
        });
        trace_ansi!("backspace");
    }

    /// Carriage return
    fn carriage_return(&mut self) {
        self.with_cursor(|cursor| {
            cursor.carriage_return()
        });
        trace_ansi!("carriage_return");
    }

    /// Linefeed
    fn linefeed(&mut self) {
        self.with_cursor(|cursor| {
            // Slight hack:
            // Write something into the new line to force the buffer to update it's size.
            cursor.write("\n ");
            cursor.move_by(-1, 0);
        });
        trace_ansi!("linefeed");
    }

    /// Ring the bell
    fn bell(&mut self) {
        //omitted
        trace_ansi!("bell");
    }

    /// Substitute char under cursor
    fn substitute(&mut self) {
        //TODO... substitute with what?
        warn_unimplemented!("substitute");
    }

    /// Newline
    fn newline(&mut self) {
        //TODO
        warn_unimplemented!("newline");
    }

    /// Set current position as a tabstop
    fn set_horizontal_tabstop(&mut self) {
        //TODO
        warn_unimplemented!("set_horizontal_tabstop");
    }

    /// Scroll up `rows` rows
    fn scroll_up(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("scroll_up");
    }

    /// Scroll down `rows` rows
    fn scroll_down(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("scroll_down");
    }

    /// Insert `count` blank lines
    fn insert_blank_lines(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("insert_blank_lines");
    }

    /// Delete `count` lines
    fn delete_lines(&mut self, _: index::Line) {
        //TODO
        warn_unimplemented!("delete_lines");
    }

    /// Erase `count` chars in current line following cursor
    ///
    /// Erase means resetting to the default state (default colors, no content,
    /// no mode flags)
    fn erase_chars(&mut self, _: index::Column) {
        //TODO
        warn_unimplemented!("erase_chars");
    }

    /// Delete `count` chars
    ///
    /// Deleting a character is like the delete key on the keyboard - everything
    /// to the right of the deleted things is shifted left.
    fn delete_chars(&mut self, _: index::Column) {
        //TODO
        warn_unimplemented!("delete_chars");
    }

    /// Move backward `count` tabs
    fn move_backward_tabs(&mut self, _count: i64) {
        //TODO
        warn_unimplemented!("move_backward_tabs");
    }

    /// Move forward `count` tabs
    fn move_forward_tabs(&mut self, _count: i64) {
        //TODO
        warn_unimplemented!("move_forward_tabs");
    }

    /// Save current cursor position
    fn save_cursor_position(&mut self) {
        //TODO
        warn_unimplemented!("save_cursor_position");
    }

    /// Restore cursor position
    fn restore_cursor_position(&mut self) {
        //TODO
        warn_unimplemented!("restore_cursor_position");
    }

    /// Clear current line
    fn clear_line(&mut self, mode: ansi::LineClearMode) {
        self.with_cursor(|cursor| {
            match mode {
                ansi::LineClearMode::Right => {
                    cursor.clear_line_right();
                },
                ansi::LineClearMode::Left => {
                    cursor.clear_line_left();
                },
                ansi::LineClearMode::All => {
                    cursor.clear_line();
                },
            }
        });
    }

    /// Clear screen
    fn clear_screen(&mut self, mode: ansi::ClearMode) {
        let clear_range = match mode {
            ansi::ClearMode::Below => {
                trace_ansi!("clear_screen below");
                let mut range_start = 0;
                self.with_cursor(|cursor| {
                    range_start = max(0, cursor.get_pos_y()+1) as usize
                });

                self.clear_line(ansi::LineClearMode::Right);
                range_start .. self.buffer.lines.len()
            },
            ansi::ClearMode::Above => {
                trace_ansi!("clear_screen above");
                let mut range_end = ::std::usize::MAX;
                self.with_cursor(|cursor| {
                    range_end = max(0, cursor.get_pos_y()) as usize
                });
                self.clear_line(ansi::LineClearMode::Left);
                self.buffer.lines.len().checked_sub(self.window_height as usize).unwrap_or(0) .. range_end
            },
            ansi::ClearMode::All => {
                trace_ansi!("clear_screen all");
                self.buffer.lines.len().checked_sub(self.window_height as usize).unwrap_or(0) .. self.buffer.lines.len()
            },
            ansi::ClearMode::Saved => {
                warn_unimplemented!("clear_screen saved");
                return;
            },
        };
        for line in self.buffer.lines[clear_range].iter_mut() {
            line.clear();
        }
    }

    /// Clear tab stops
    fn clear_tabs(&mut self, _: ansi::TabulationClearMode) {
        //TODO
        warn_unimplemented!("clear_tabs");
    }

    /// Reset terminal state
    fn reset_state(&mut self) {
        //TODO
        warn_unimplemented!("reset_state");
    }

    /// Reverse Index
    ///
    /// Move the active position to the same horizontal position on the
    /// preceding line. If the active position is at the top margin, a scroll
    /// down is performed
    fn reverse_index(&mut self) {
        //TODO
        warn_unimplemented!("reverse_index");
    }

    /// set a terminal attribute
    fn terminal_attribute(&mut self, attr: Attr) {
        self.with_cursor(|c| {
            match attr {
                Attr::Reset => { c.set_style_modifier(StyleModifier::new()) },
                Attr::Bold => { c.apply_style_modifier(StyleModifier::new().bold(true)); },
                Attr::Dim => { /* What is this? */ warn_unimplemented!("attr Dim") },
                Attr::Italic => { c.apply_style_modifier(StyleModifier::new().italic(true)); },
                Attr::Underscore => { c.apply_style_modifier(StyleModifier::new().underline(true)); },
                Attr::BlinkSlow => { warn_unimplemented!("attr BlinkSlow") },
                Attr::BlinkFast => { warn_unimplemented!("attr BlinkFast") },
                Attr::Reverse => { c.apply_style_modifier(StyleModifier::new().invert(true)); },
                Attr::Hidden => { warn_unimplemented!("attr Hidden") },
                Attr::Strike => { warn_unimplemented!("attr Strike") },
                Attr::CancelBold => { c.apply_style_modifier(StyleModifier::new().bold(false)); },
                Attr::CancelBoldDim => { /*??*/c.apply_style_modifier(StyleModifier::new().bold(false)); },
                Attr::CancelItalic => { c.apply_style_modifier(StyleModifier::new().italic(false)); },
                Attr::CancelUnderline => { c.apply_style_modifier(StyleModifier::new().underline(false)); },
                Attr::CancelBlink => { warn_unimplemented!("attr CancelBlink") },
                Attr::CancelReverse => { c.apply_style_modifier(StyleModifier::new().invert(false)); },
                Attr::CancelHidden => { warn_unimplemented!("attr CancelHidden") },
                Attr::CancelStrike => { warn_unimplemented!("attr CancelStrike") },
                Attr::Foreground(color) => { c.apply_style_modifier(StyleModifier::new().fg_color(ansi_to_unsegen_color(color))); },
                Attr::Background(color) => { c.apply_style_modifier(StyleModifier::new().bg_color(ansi_to_unsegen_color(color))); },
            }
        });
        trace_ansi!("terminal_attribute {:?}", attr);
    }

    /// Set mode
    fn set_mode(&mut self, mode: ansi::Mode) {
        match mode {
            ansi::Mode::ShowCursor => {
                self.show_cursor = true;
                trace_ansi!("set_mode {:?}", mode);
            },
            _ => { warn_unimplemented!("set_mode {:?}", mode); },
        }
    }

    /// Unset mode
    fn unset_mode(&mut self, mode: ansi::Mode) {
        match mode {
            ansi::Mode::ShowCursor => {
                self.show_cursor = false;
                trace_ansi!("set_mode {:?}", mode);
            },
            _ => { warn_unimplemented!("set_mode {:?}", mode); },
        }
    }

    /// DECSTBM - Set the terminal scrolling region
    fn set_scrolling_region(&mut self, _: ::std::ops::Range<index::Line>) {
        //TODO
        warn_unimplemented!("set_scrolling_region");
    }

    /// DECKPAM - Set keypad to applications mode (ESCape instead of digits)
    fn set_keypad_application_mode(&mut self) {
        //TODO
        warn_unimplemented!("set_keypad_application_mode");
    }

    /// DECKPNM - Set keypad to numeric mode (digits intead of ESCape seq)
    fn unset_keypad_application_mode(&mut self) {
        //TODO
        warn_unimplemented!("unset_keypad_application_mode");
    }

    /// Set one of the graphic character sets, G0 to G3, as the active charset.
    ///
    /// 'Invoke' one of G0 to G3 in the GL area. Also refered to as shift in,
    /// shift out and locking shift depending on the set being activated
    fn set_active_charset(&mut self, _: ansi::CharsetIndex) {
        //TODO
        warn_unimplemented!("set_active_charset");
    }

    /// Assign a graphic character set to G0, G1, G2 or G3
    ///
    /// 'Designate' a graphic character set as one of G0 to G3, so that it can
    /// later be 'invoked' by `set_active_charset`
    fn configure_charset(&mut self, _: ansi::CharsetIndex, _: ansi::StandardCharset) {
        //TODO
        warn_unimplemented!("configure_charset");
    }

    /// Set an indexed color value
    fn set_color(&mut self, _: usize, _: ansi::Rgb) {
        //TODO: Implement this, once there is support for a per-terminal color table
        warn_unimplemented!("set_color");
    }

    /// Run the dectest routine
    fn dectest(&mut self) {
        //TODO
        warn_unimplemented!("dectest");
    }
}

impl TermInfo for TerminalWindow {
    fn lines(&self) -> index::Line {
        index::Line(self.get_height() as usize) //TODO: is this even correct? do we want 'unbounded'?
    }
    fn cols(&self) -> index::Column {
        index::Column(self.get_width() as usize) //TODO: see above
    }
}

impl Scrollable for TerminalWindow {
    fn scroll_forwards(&mut self) -> OperationResult {
        let current = self.current_scrollback_pos();
        let candidate = current + self.scroll_step;
        self.scrollback_position = if candidate < self.buffer.height_as_displayed() {
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
        let current = self.current_scrollback_pos();
        if current > self.window_height {
            self.scrollback_position = Some(current.checked_sub(self.scroll_step).unwrap_or(0));
            Ok(())
        } else {
            Err(())
        }
    }
}

#[cfg(test)]
impl TerminalWindow {
    fn write(&mut self, s: &str) {
        for c in s.chars() {
            self.input(c);
        }
    }
}
#[cfg(test)]
mod test {
    use unsegen::base::terminal::test::FakeTerminal;
    use super::*;
    use unsegen::base::{
        GraphemeCluster,
    };

    fn test_terminal_window<F: Fn(&mut TerminalWindow)>(window_dim: (u32, u32), after: &str, action: F) {
        let mut term = FakeTerminal::with_size(window_dim);
        {
            let mut window = term.create_root_window();
            window.fill(GraphemeCluster::try_from('_').unwrap());
            let mut tw = TerminalWindow::new();
            action(&mut tw);
            tw.draw(window, RenderingHints::default());
        }
        term.assert_looks_like(after);
    }
    #[test]
    fn test_terminal_window_simple() {
        test_terminal_window((5, 1), "_____", |w| w.write(""));
        test_terminal_window((5, 1), "t____", |w| w.write("t"));
        test_terminal_window((5, 1), "te___", |w| w.write("te"));
        test_terminal_window((5, 1), "tes__", |w| w.write("tes"));
        test_terminal_window((5, 1), "test_", |w| w.write("test"));
        test_terminal_window((5, 1), "testy", |w| w.write("testy"));
        test_terminal_window((5, 1), "o____", |w| w.write("testyo"));

        test_terminal_window((2, 2), "te|st", |w| w.write("te\nst"));
    }
}
