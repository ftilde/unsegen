//! Widget abstraction and some basic Widgets useful for creating basic building blocks of text
//! user interfaces.
//!
//! # Example:
//! ```no_run //tests do not provide a fully functional terminal
//!
//! use unsegen::base::*;
//! use unsegen::widget::*;
//! use unsegen::widget::builtin::*;
//! use std::io::stdout;
//!
//! struct MyWidget {
//!     prompt: PromptLine,
//!     buffer: LogViewer,
//! }
//!
//! impl MyWidget {
//!     fn as_widget<'a>(&'a self) -> impl Widget + 'a {
//!         VLayout::new().alterating(StyleModifier::new().invert(true))
//!             .widget("Some text on top")
//!             .widget(self.prompt.as_widget())
//!             .widget(self.buffer.as_widget())
//!             .widget("Some text below")
//!     }
//! }
//!
//!
//! fn main() {
//!     let stdout = stdout();
//!     let mut term = Terminal::new(stdout.lock()).unwrap();
//!     let mut widget = MyWidget {
//!         prompt: PromptLine::with_prompt(" > ".to_owned()),
//!         buffer: LogViewer::new(),
//!     };
//!
//!     loop {
//!         // Put application logic here: read input, chain behavior, react to other stuff
//!         {
//!             let win = term.create_root_window();
//!             widget.as_widget().draw(win, RenderingHints::new().active(true));
//!         }
//!         term.present();
//!     }
//! }
//! ```
pub mod builtin;
pub mod layouts;
pub mod widget;

pub use self::layouts::*;
pub use self::widget::*;
use super::base::*;

/// Count the number of grapheme clusters in the given string.
///
/// A thin convenience wrapper around unicode_segmentation.
pub fn count_grapheme_clusters(text: &str) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.graphemes(true).count()
}

/// Calculate the (monospace) width of the given string.
///
/// A thin convenience wrapper around unicode_width.
pub fn text_width(text: &str) -> Width {
    use unicode_width::UnicodeWidthStr;
    Width::new(UnicodeWidthStr::width(text) as _).unwrap()
}
