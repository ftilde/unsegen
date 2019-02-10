//! Types related to the visual representation (i.e., style) of text when drawn to the terminal.
//! This includes formatting (bold, italic, ...) and colors.
use std::io::Write;
use termion;

/// Specifies how text is written to the terminal.
/// Specified attributes include "bold", "italic", "invert", and "underline" and can be combined
/// freely.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextFormat {
    pub bold: bool,
    pub italic: bool,
    pub invert: bool,
    pub underline: bool,

    // Make users of the library unable to construct RenderingHints from members.
    // This way we can add members in a backwards compatible way in future versions.
    #[doc(hidden)]
    _do_not_construct: (),
}

impl TextFormat {
    /// Set the attributes of the given ANSI terminal to match the current TextFormat.
    fn set_terminal_attributes<W: Write>(&self, terminal: &mut W) {
        if self.bold {
            write!(terminal, "{}", termion::style::Bold).expect("set bold style");
        } else {
            write!(terminal, "{}", termion::style::NoBold).expect("set no bold style");
        }

        if self.italic {
            write!(terminal, "{}", termion::style::Italic).expect("set italic style");
        } else {
            write!(terminal, "{}", termion::style::NoItalic).expect("set no italic style");
        }

        if self.invert {
            write!(terminal, "{}", termion::style::Invert).expect("set invert style");
        } else {
            write!(terminal, "{}", termion::style::NoInvert).expect("set no invert style");
        }

        if self.underline {
            write!(terminal, "{}", termion::style::Underline).expect("set underline style");
        } else {
            write!(terminal, "{}", termion::style::NoUnderline).expect("set no underline style");
        }
    }
}

impl Default for TextFormat {
    fn default() -> Self {
        TextFormat {
            bold: false,
            italic: false,
            invert: false,
            underline: false,
            _do_not_construct: (),
        }
    }
}

/// Specifies how to modify a bool value.
///
/// (In essence, specifies one of all possible unary boolean functions.)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoolModifyMode {
    Yes,
    No,
    Toggle,
    LeaveUnchanged,
}

impl BoolModifyMode {
    /// Combine the current value with that of the argument so that the application of the returned
    /// value is always equivalent to first applying other and then applying self to a bool.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::BoolModifyMode;
    ///
    /// let mut b = true;
    ///
    /// BoolModifyMode::False.on_top_of(BoolModifyMode::True).modify(&mut b);
    /// assert_eq!(b, false);
    ///
    /// BoolModifyMode::Toggle.on_top_of(BoolModifyMode::False).modify(&mut b);
    /// assert_eq!(b, true);
    ///
    /// BoolModifyMode::Toggle.on_top_of(BoolModifyMode::Toggle).modify(&mut b);
    /// assert_eq!(b, true);
    ///
    ///
    /// assert_eq!(BoolModifyMode::True.on_top_of(BoolModifyMode::Toggle /*or any other value*/),
    ///             BoolModifyMode::True);
    /// assert_eq!(BoolModifyMode::Toggle.on_top_of(BoolModifyMode::Toggle),
    ///             BoolModifyMode::LeaveUnchanged);
    /// ```
    ///
    fn on_top_of(&self, other: &Self) -> Self {
        match (*self, *other) {
            (BoolModifyMode::Yes, _) => BoolModifyMode::Yes,
            (BoolModifyMode::No, _) => BoolModifyMode::No,
            (BoolModifyMode::Toggle, BoolModifyMode::Yes) => BoolModifyMode::No,
            (BoolModifyMode::Toggle, BoolModifyMode::No) => BoolModifyMode::Yes,
            (BoolModifyMode::Toggle, BoolModifyMode::Toggle) => BoolModifyMode::No,
            (BoolModifyMode::Toggle, BoolModifyMode::LeaveUnchanged) => BoolModifyMode::Toggle,
            (BoolModifyMode::LeaveUnchanged, m) => m,
        }
    }
    fn modify(&self, should_invert: &mut bool) {
        match *self {
            BoolModifyMode::Yes => *should_invert = true,
            BoolModifyMode::No => *should_invert = false,
            BoolModifyMode::Toggle => *should_invert ^= true,
            BoolModifyMode::LeaveUnchanged => {}
        }
    }
}

impl ::std::convert::From<bool> for BoolModifyMode {
    fn from(on: bool) -> Self {
        if on {
            BoolModifyMode::Yes
        } else {
            BoolModifyMode::No
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextFormatModifier {
    pub bold: BoolModifyMode,
    pub italic: BoolModifyMode,
    pub invert: BoolModifyMode,
    pub underline: BoolModifyMode,
}

impl TextFormatModifier {
    pub fn new() -> Self {
        TextFormatModifier {
            bold: BoolModifyMode::LeaveUnchanged,
            italic: BoolModifyMode::LeaveUnchanged,
            invert: BoolModifyMode::LeaveUnchanged,
            underline: BoolModifyMode::LeaveUnchanged,
        }
    }
    pub fn bold<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.bold = val.into();
        self
    }
    pub fn italic<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.italic = val.into();
        self
    }
    pub fn invert<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.invert = val.into();
        self
    }
    pub fn underline<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.underline = val.into();
        self
    }
    fn on_top_of(&self, other: &TextFormatModifier) -> Self {
        TextFormatModifier {
            bold: self.bold.on_top_of(&other.bold),
            italic: self.italic.on_top_of(&other.italic),
            invert: self.invert.on_top_of(&other.invert),
            underline: self.underline.on_top_of(&other.underline),
        }
    }

    fn modify(&self, format: &mut TextFormat) {
        self.bold.modify(&mut format.bold);
        self.italic.modify(&mut format.italic);
        self.invert.modify(&mut format.invert);
        self.underline.modify(&mut format.underline);
    }
}

impl Default for TextFormatModifier {
    fn default() -> Self {
        TextFormatModifier::new()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Rgb { r: u8, g: u8, b: u8 },
    Ansi(u8),
    Black,
    Blue,
    Cyan,
    Green,
    Magenta,
    Red,
    White,
    Yellow,
    LightBlack,
    LightBlue,
    LightCyan,
    LightGreen,
    LightMagenta,
    LightRed,
    LightWhite,
    LightYellow,
}

impl Color {
    pub fn ansi_rgb(r: u8, g: u8, b: u8) -> Self {
        Color::Ansi(termion::color::AnsiValue::rgb(r, g, b).0)
    }
    pub fn ansi_grayscale(v: u8 /* < 24 */) -> Self {
        Color::Ansi(termion::color::AnsiValue::grayscale(v).0)
    }

    fn set_terminal_attributes_fg<W: Write>(&self, terminal: &mut W) -> ::std::io::Result<()> {
        use termion::color::Fg as Target;
        match self {
            &Color::Rgb { r, g, b } => write!(terminal, "{}", Target(termion::color::Rgb(r, g, b))),
            &Color::Ansi(v) => write!(terminal, "{}", Target(termion::color::AnsiValue(v))),
            &Color::Black => write!(terminal, "{}", Target(termion::color::Black)),
            &Color::Blue => write!(terminal, "{}", Target(termion::color::Blue)),
            &Color::Cyan => write!(terminal, "{}", Target(termion::color::Cyan)),
            &Color::Magenta => write!(terminal, "{}", Target(termion::color::Magenta)),
            &Color::Green => write!(terminal, "{}", Target(termion::color::Green)),
            &Color::Red => write!(terminal, "{}", Target(termion::color::Red)),
            &Color::White => write!(terminal, "{}", Target(termion::color::White)),
            &Color::Yellow => write!(terminal, "{}", Target(termion::color::Yellow)),
            &Color::LightBlack => write!(terminal, "{}", Target(termion::color::LightBlack)),
            &Color::LightBlue => write!(terminal, "{}", Target(termion::color::LightBlue)),
            &Color::LightCyan => write!(terminal, "{}", Target(termion::color::LightCyan)),
            &Color::LightMagenta => write!(terminal, "{}", Target(termion::color::LightMagenta)),
            &Color::LightGreen => write!(terminal, "{}", Target(termion::color::LightGreen)),
            &Color::LightRed => write!(terminal, "{}", Target(termion::color::LightRed)),
            &Color::LightWhite => write!(terminal, "{}", Target(termion::color::LightWhite)),
            &Color::LightYellow => write!(terminal, "{}", Target(termion::color::LightYellow)),
        }
    }
    fn set_terminal_attributes_bg<W: Write>(&self, terminal: &mut W) -> ::std::io::Result<()> {
        use termion::color::Bg as Target;
        match self {
            &Color::Rgb { r, g, b } => write!(terminal, "{}", Target(termion::color::Rgb(r, g, b))),
            &Color::Ansi(v) => write!(terminal, "{}", Target(termion::color::AnsiValue(v))),
            &Color::Black => write!(terminal, "{}", Target(termion::color::Black)),
            &Color::Blue => write!(terminal, "{}", Target(termion::color::Blue)),
            &Color::Cyan => write!(terminal, "{}", Target(termion::color::Cyan)),
            &Color::Magenta => write!(terminal, "{}", Target(termion::color::Magenta)),
            &Color::Green => write!(terminal, "{}", Target(termion::color::Green)),
            &Color::Red => write!(terminal, "{}", Target(termion::color::Red)),
            &Color::White => write!(terminal, "{}", Target(termion::color::White)),
            &Color::Yellow => write!(terminal, "{}", Target(termion::color::Yellow)),
            &Color::LightBlack => write!(terminal, "{}", Target(termion::color::LightBlack)),
            &Color::LightBlue => write!(terminal, "{}", Target(termion::color::LightBlue)),
            &Color::LightCyan => write!(terminal, "{}", Target(termion::color::LightCyan)),
            &Color::LightMagenta => write!(terminal, "{}", Target(termion::color::LightMagenta)),
            &Color::LightGreen => write!(terminal, "{}", Target(termion::color::LightGreen)),
            &Color::LightRed => write!(terminal, "{}", Target(termion::color::LightRed)),
            &Color::LightWhite => write!(terminal, "{}", Target(termion::color::LightWhite)),
            &Color::LightYellow => write!(terminal, "{}", Target(termion::color::LightYellow)),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Style {
    fg_color: Color,
    bg_color: Color,
    format: TextFormat,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            fg_color: Color::White,
            bg_color: Color::LightBlack,
            format: TextFormat::default(),
        }
    }
}

impl Style {
    pub fn new(fg_color: Color, bg_color: Color, format: TextFormat) -> Self {
        Style {
            fg_color: fg_color,
            bg_color: bg_color,
            format: format,
        }
    }

    pub fn plain() -> Self {
        Self::default()
    }

    pub fn set_terminal_attributes<W: Write>(&self, terminal: &mut W) {
        self.fg_color
            .set_terminal_attributes_fg(terminal)
            .expect("write fg_color");
        self.bg_color
            .set_terminal_attributes_bg(terminal)
            .expect("write bg_color");
        self.format.set_terminal_attributes(terminal);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StyleModifier {
    fg_color: Option<Color>,
    bg_color: Option<Color>,
    format: TextFormatModifier,
}

impl StyleModifier {
    pub fn none() -> Self {
        StyleModifier {
            fg_color: None,
            bg_color: None,
            format: TextFormatModifier::new(),
        }
    }

    pub fn new() -> Self {
        Self::none()
    }

    pub fn fg_color(mut self, fg_color: Color) -> Self {
        self.fg_color = Some(fg_color);
        self
    }

    pub fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    pub fn format(mut self, format: TextFormatModifier) -> Self {
        self.format = format;
        self
    }

    // Convenience functions to access text format
    pub fn bold<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.bold = val.into();
        self
    }
    pub fn italic<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.italic = val.into();
        self
    }
    pub fn invert<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.invert = val.into();
        self
    }
    pub fn underline<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.underline = val.into();
        self
    }

    pub fn on_top_of(&self, other: &StyleModifier) -> Self {
        StyleModifier {
            fg_color: self.fg_color.or(other.fg_color),
            bg_color: self.bg_color.or(other.bg_color),
            format: self.format.on_top_of(&other.format),
        }
    }

    pub fn apply_to_default(&self) -> Style {
        let mut style = Style::default();
        self.modify(&mut style);
        style
    }

    pub fn apply(&self, style: &Style) -> Style {
        let mut style = style.clone();
        self.modify(&mut style);
        style
    }

    pub fn modify(&self, style: &mut Style) {
        if let Some(fg) = self.fg_color {
            style.fg_color = fg;
        }
        if let Some(bg) = self.bg_color {
            style.bg_color = bg;
        }
        self.format.modify(&mut style.format);
    }
}
