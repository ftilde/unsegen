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

    // Make users of the library unable to construct Textformat from members.
    // This way we can add members in a backwards compatible way in future versions.
    #[doc(hidden)]
    _do_not_construct: (),
}

impl TextFormat {
    /// Set the attributes of the given ANSI terminal to match the current TextFormat.
    fn set_terminal_attributes<W: Write>(self, terminal: &mut W) {
        if self.bold {
            write!(terminal, "{}", termion::style::Bold).expect("set bold style");
        }

        if self.italic {
            write!(terminal, "{}", termion::style::Italic).expect("set italic style");
        }

        if self.invert {
            write!(terminal, "{}", termion::style::Invert).expect("set invert style");
        }

        if self.underline {
            write!(terminal, "{}", termion::style::Underline).expect("set underline style");
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
    True,
    False,
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
    /// assert_eq!(BoolModifyMode::LeaveUnchanged.on_top_of(BoolModifyMode::False),
    ///             BoolModifyMode::False);
    /// assert_eq!(BoolModifyMode::True.on_top_of(BoolModifyMode::Toggle /*or any other value*/),
    ///             BoolModifyMode::True);
    /// assert_eq!(BoolModifyMode::Toggle.on_top_of(BoolModifyMode::Toggle),
    ///             BoolModifyMode::LeaveUnchanged);
    /// ```
    ///
    pub fn on_top_of(self, other: Self) -> Self {
        match (self, other) {
            (BoolModifyMode::True, _) => BoolModifyMode::True,
            (BoolModifyMode::False, _) => BoolModifyMode::False,
            (BoolModifyMode::Toggle, BoolModifyMode::True) => BoolModifyMode::False,
            (BoolModifyMode::Toggle, BoolModifyMode::False) => BoolModifyMode::True,
            (BoolModifyMode::Toggle, BoolModifyMode::Toggle) => BoolModifyMode::LeaveUnchanged,
            (BoolModifyMode::Toggle, BoolModifyMode::LeaveUnchanged) => BoolModifyMode::Toggle,
            (BoolModifyMode::LeaveUnchanged, m) => m,
        }
    }

    /// Modify the target bool according to the modification mode.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::BoolModifyMode;
    ///
    /// let mut b = true;
    /// BoolModifyMode::False.modify(&mut b);
    /// assert_eq!(b, false);
    ///
    /// let mut b = false;
    /// BoolModifyMode::LeaveUnchanged.modify(&mut b);
    /// assert_eq!(b, false);
    ///
    /// let mut b = false;
    /// BoolModifyMode::Toggle.modify(&mut b);
    /// assert_eq!(b, true);
    /// ```
    ///
    pub fn modify(self, target: &mut bool) {
        match self {
            BoolModifyMode::True => *target = true,
            BoolModifyMode::False => *target = false,
            BoolModifyMode::Toggle => *target ^= true,
            BoolModifyMode::LeaveUnchanged => {}
        }
    }
}

impl ::std::convert::From<bool> for BoolModifyMode {
    fn from(on: bool) -> Self {
        if on {
            BoolModifyMode::True
        } else {
            BoolModifyMode::False
        }
    }
}

/// Specifies how to modify a text format value.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TextFormatModifier {
    pub bold: BoolModifyMode,
    pub italic: BoolModifyMode,
    pub invert: BoolModifyMode,
    pub underline: BoolModifyMode,

    // Make users of the library unable to construct TextFormatModifier from members.
    // This way we can add members in a backwards compatible way in future versions.
    #[doc(hidden)]
    _do_not_construct: (),
}

impl TextFormatModifier {
    /// Construct a new (not actually) modifier, that leaves all properties unchanged.
    pub fn new() -> Self {
        TextFormatModifier {
            bold: BoolModifyMode::LeaveUnchanged,
            italic: BoolModifyMode::LeaveUnchanged,
            invert: BoolModifyMode::LeaveUnchanged,
            underline: BoolModifyMode::LeaveUnchanged,
            _do_not_construct: (),
        }
    }
    /// Set the bold property of the TextFormatModifier
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{TextFormatModifier, BoolModifyMode};
    ///
    /// assert_eq!(TextFormatModifier::new().bold(BoolModifyMode::Toggle).bold,
    /// BoolModifyMode::Toggle);
    /// ```
    pub fn bold<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.bold = val.into();
        self
    }

    /// Set the italic property of the TextFormatModifier
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{TextFormatModifier, BoolModifyMode};
    ///
    /// assert_eq!(TextFormatModifier::new().italic(BoolModifyMode::True).italic,
    /// BoolModifyMode::True);
    /// ```
    pub fn italic<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.italic = val.into();
        self
    }

    /// Set the invert property of the TextFormatModifier
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{TextFormatModifier, BoolModifyMode};
    ///
    /// assert_eq!(TextFormatModifier::new().invert(BoolModifyMode::False).invert,
    /// BoolModifyMode::False);
    /// ```
    pub fn invert<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.invert = val.into();
        self
    }

    /// Set underline invert property of the TextFormatModifier
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{TextFormatModifier, BoolModifyMode};
    ///
    /// assert_eq!(TextFormatModifier::new().underline(BoolModifyMode::LeaveUnchanged).underline,
    /// BoolModifyMode::LeaveUnchanged);
    /// ```
    pub fn underline<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.underline = val.into();
        self
    }

    /// Combine the current value with that of the argument so that the application of the returned
    /// value is always equivalent to first applying other and then applying self to some TextFormat.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{TextFormatModifier, TextFormat, BoolModifyMode};
    ///
    /// let mut f1 = TextFormat::default();
    /// let mut f2 = f1;
    ///
    /// let m1 =
    /// TextFormatModifier::new().italic(BoolModifyMode::Toggle).bold(true).underline(false);
    /// let m2 = TextFormatModifier::new().italic(true).bold(false);
    ///
    /// m1.on_top_of(m2).modify(&mut f1);
    ///
    /// m2.modify(&mut f2);
    /// m1.modify(&mut f2);
    ///
    /// assert_eq!(f1, f2);
    /// ```
    ///
    pub fn on_top_of(self, other: TextFormatModifier) -> Self {
        TextFormatModifier {
            bold: self.bold.on_top_of(other.bold),
            italic: self.italic.on_top_of(other.italic),
            invert: self.invert.on_top_of(other.invert),
            underline: self.underline.on_top_of(other.underline),
            _do_not_construct: (),
        }
    }

    /// Modify the passed textformat according to the modification rules of self.
    pub fn modify(self, format: &mut TextFormat) {
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

/// A color that can be displayed in terminal.
///
/// Colors are either:
///     - Default (i.e., terminal color is reset)
///     - Named (Black, Yellow, LightRed, ...)
///     - Ansi (8 bit)
///     - or Rgb.
///
/// Not all terminals may support Rgb, though.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Color {
    Default,
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

impl Default for Color {
    fn default() -> Self {
        Color::Default
    }
}

impl Color {
    /// Construct an ansi color value from rgb values.
    /// r, g and b must all be < 6.
    pub fn ansi_rgb(r: u8, g: u8, b: u8) -> Self {
        assert!(r < 6, "Invalid red value");
        assert!(g < 6, "Invalid green value");
        assert!(b < 6, "Invalid blue value");
        Color::Ansi(termion::color::AnsiValue::rgb(r, g, b).0)
    }

    /// Construct a gray value ansi color.
    /// v must be < 24.
    pub fn ansi_grayscale(v: u8 /* < 24 */) -> Self {
        assert!(v < 24, "Invalid gray value");
        Color::Ansi(termion::color::AnsiValue::grayscale(v).0)
    }

    /// Set the forground color of the terminal.
    fn set_terminal_attributes_fg<W: Write>(self, terminal: &mut W) -> ::std::io::Result<()> {
        use termion::color::Fg as Target;
        match self {
            Color::Default => Ok(()),
            Color::Rgb { r, g, b } => write!(terminal, "{}", Target(termion::color::Rgb(r, g, b))),
            Color::Ansi(v) => write!(terminal, "{}", Target(termion::color::AnsiValue(v))),
            Color::Black => write!(terminal, "{}", Target(termion::color::Black)),
            Color::Blue => write!(terminal, "{}", Target(termion::color::Blue)),
            Color::Cyan => write!(terminal, "{}", Target(termion::color::Cyan)),
            Color::Magenta => write!(terminal, "{}", Target(termion::color::Magenta)),
            Color::Green => write!(terminal, "{}", Target(termion::color::Green)),
            Color::Red => write!(terminal, "{}", Target(termion::color::Red)),
            Color::White => write!(terminal, "{}", Target(termion::color::White)),
            Color::Yellow => write!(terminal, "{}", Target(termion::color::Yellow)),
            Color::LightBlack => write!(terminal, "{}", Target(termion::color::LightBlack)),
            Color::LightBlue => write!(terminal, "{}", Target(termion::color::LightBlue)),
            Color::LightCyan => write!(terminal, "{}", Target(termion::color::LightCyan)),
            Color::LightMagenta => write!(terminal, "{}", Target(termion::color::LightMagenta)),
            Color::LightGreen => write!(terminal, "{}", Target(termion::color::LightGreen)),
            Color::LightRed => write!(terminal, "{}", Target(termion::color::LightRed)),
            Color::LightWhite => write!(terminal, "{}", Target(termion::color::LightWhite)),
            Color::LightYellow => write!(terminal, "{}", Target(termion::color::LightYellow)),
        }
    }

    /// Set the background color of the terminal.
    fn set_terminal_attributes_bg<W: Write>(self, terminal: &mut W) -> ::std::io::Result<()> {
        use termion::color::Bg as Target;
        match self {
            Color::Default => Ok(()),
            Color::Rgb { r, g, b } => write!(terminal, "{}", Target(termion::color::Rgb(r, g, b))),
            Color::Ansi(v) => write!(terminal, "{}", Target(termion::color::AnsiValue(v))),
            Color::Black => write!(terminal, "{}", Target(termion::color::Black)),
            Color::Blue => write!(terminal, "{}", Target(termion::color::Blue)),
            Color::Cyan => write!(terminal, "{}", Target(termion::color::Cyan)),
            Color::Magenta => write!(terminal, "{}", Target(termion::color::Magenta)),
            Color::Green => write!(terminal, "{}", Target(termion::color::Green)),
            Color::Red => write!(terminal, "{}", Target(termion::color::Red)),
            Color::White => write!(terminal, "{}", Target(termion::color::White)),
            Color::Yellow => write!(terminal, "{}", Target(termion::color::Yellow)),
            Color::LightBlack => write!(terminal, "{}", Target(termion::color::LightBlack)),
            Color::LightBlue => write!(terminal, "{}", Target(termion::color::LightBlue)),
            Color::LightCyan => write!(terminal, "{}", Target(termion::color::LightCyan)),
            Color::LightMagenta => write!(terminal, "{}", Target(termion::color::LightMagenta)),
            Color::LightGreen => write!(terminal, "{}", Target(termion::color::LightGreen)),
            Color::LightRed => write!(terminal, "{}", Target(termion::color::LightRed)),
            Color::LightWhite => write!(terminal, "{}", Target(termion::color::LightWhite)),
            Color::LightYellow => write!(terminal, "{}", Target(termion::color::LightYellow)),
        }
    }
}

/// A style that defines how text is presented on the terminal.
///
/// Use StyleModifier to modify the style from the default/plain state.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Style {
    fg_color: Color,
    bg_color: Color,
    format: TextFormat,
}

impl Style {
    /// Create a "standard" style, i.e., no fancy colors or text attributes.
    pub fn plain() -> Self {
        Self::default()
    }

    /// Set the attributes of the given ANSI terminal to match the current Style.
    pub(crate) fn set_terminal_attributes<W: Write>(self, terminal: &mut W) {
        // Since we cannot rely on NoBold reseting the bold style (see
        // https://en.wikipedia.org/wiki/Talk:ANSI_escape_code#SGR_21%E2%80%94%60Bold_off%60_not_widely_supported)
        // we first reset _all_ styles, then reapply anything that differs from the default.
        write!(
            terminal,
            "{}{}{}",
            termion::style::Reset,
            termion::color::Fg(termion::color::Reset),
            termion::color::Bg(termion::color::Reset)
        )
        .expect("reset style");

        self.fg_color
            .set_terminal_attributes_fg(terminal)
            .expect("write fg_color");
        self.bg_color
            .set_terminal_attributes_bg(terminal)
            .expect("write bg_color");
        self.format.set_terminal_attributes(terminal);
    }
}

/// Defines a set of modifications on a style. Multiple modifiers can be combined before applying
/// them to a style.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct StyleModifier {
    fg_color: Option<Color>,
    bg_color: Option<Color>,
    format: TextFormatModifier,
}

impl StyleModifier {
    /// Construct a new (not actually) modifier, that leaves all style properties unchanged.
    pub fn new() -> Self {
        Self::default()
    }

    /// Make the modifier change the foreground color to the specified value.
    pub fn fg_color(mut self, fg_color: Color) -> Self {
        self.fg_color = Some(fg_color);
        self
    }

    /// Make the modifier change the background color to the specified value.
    pub fn bg_color(mut self, bg_color: Color) -> Self {
        self.bg_color = Some(bg_color);
        self
    }

    /// Make the modifier change the textformat of the style to the specified value.
    pub fn format(mut self, format: TextFormatModifier) -> Self {
        self.format = format;
        self
    }

    /// Make the modifier change the bold property of the textformat of the style to the specified value.
    ///
    /// This is a shortcut for using `format` using a TextFormatModifier that changes the bold
    /// property.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{StyleModifier, TextFormatModifier};
    ///
    /// let s1 = StyleModifier::new().bold(true);
    /// let s2 = StyleModifier::new().format(TextFormatModifier::new().bold(true));
    ///
    /// assert_eq!(s1, s2);
    /// ```
    pub fn bold<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.bold = val.into();
        self
    }

    /// Make the modifier change the italic property of the textformat of the style to the specified value.
    ///
    /// This is a shortcut for using `format` using a TextFormatModifier that changes the italic
    /// property.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{StyleModifier, TextFormatModifier};
    ///
    /// let s1 = StyleModifier::new().italic(true);
    /// let s2 = StyleModifier::new().format(TextFormatModifier::new().italic(true));
    ///
    /// assert_eq!(s1, s2);
    /// ```
    pub fn italic<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.italic = val.into();
        self
    }

    /// Make the modifier change the invert property of the textformat of the style to the specified value.
    ///
    /// This is a shortcut for using `format` using a TextFormatModifier that changes the invert
    /// property.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{StyleModifier, TextFormatModifier};
    ///
    /// let s1 = StyleModifier::new().invert(true);
    /// let s2 = StyleModifier::new().format(TextFormatModifier::new().invert(true));
    ///
    /// assert_eq!(s1, s2);
    /// ```
    pub fn invert<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.invert = val.into();
        self
    }

    /// Make the modifier change the underline property of the textformat of the style to the specified value.
    ///
    /// This is a shortcut for using `format` using a TextFormatModifier that changes the underline
    /// property.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::{StyleModifier, TextFormatModifier};
    ///
    /// let s1 = StyleModifier::new().underline(true);
    /// let s2 = StyleModifier::new().format(TextFormatModifier::new().underline(true));
    ///
    /// assert_eq!(s1, s2);
    /// ```
    pub fn underline<M: Into<BoolModifyMode>>(mut self, val: M) -> Self {
        self.format.underline = val.into();
        self
    }

    /// Combine the current value with that of the argument so that the application of the returned
    /// value is always equivalent to first applying other and then applying self to some Style.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    ///
    /// let mut s1 = Style::default();
    /// let mut s2 = s1;
    ///
    /// let m1 =
    /// StyleModifier::new().fg_color(Color::Red).italic(BoolModifyMode::Toggle).bold(true).underline(false);
    /// let m2 = StyleModifier::new().bg_color(Color::Blue).italic(true).bold(false);
    ///
    /// m1.on_top_of(m2).modify(&mut s1);
    ///
    /// m2.modify(&mut s2);
    /// m1.modify(&mut s2);
    ///
    /// assert_eq!(s1, s2);
    /// ```
    ///
    pub fn on_top_of(self, other: StyleModifier) -> Self {
        StyleModifier {
            fg_color: self.fg_color.or(other.fg_color),
            bg_color: self.bg_color.or(other.bg_color),
            format: self.format.on_top_of(other.format),
        }
    }

    /// Apply the modifier to a default (i.e., empty) Style. In a way, this converts the
    /// StyleModifier to a Style.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    ///
    /// let m = StyleModifier::new().fg_color(Color::Red).italic(true);
    /// let mut style = Style::default();
    ///
    /// assert_eq!(m.apply(style), m.apply_to_default());
    /// ```
    pub fn apply_to_default(self) -> Style {
        let mut style = Style::default();
        self.modify(&mut style);
        style
    }

    /// Apply this modifier to a given style and return the result. This is essentially a
    /// convenience wrapper around modify, which clones the Style.
    ///
    /// # Examples:
    /// ```
    /// use unsegen::base::*;
    ///
    /// let m = StyleModifier::new().fg_color(Color::Red).italic(BoolModifyMode::Toggle);
    /// let mut style = Style::default();
    /// let style2 = m.apply(style);
    /// m.modify(&mut style);
    ///
    /// assert_eq!(style, style2);
    /// ```
    pub fn apply(self, mut style: Style) -> Style {
        self.modify(&mut style);
        style
    }

    /// Modify the given style according to the properties of this modifier.
    pub fn modify(self, style: &mut Style) {
        if let Some(fg) = self.fg_color {
            style.fg_color = fg;
        }
        if let Some(bg) = self.bg_color {
            style.bg_color = bg;
        }
        self.format.modify(&mut style.format);
    }
}
