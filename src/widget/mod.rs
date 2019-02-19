//! Widget abstraction and some basic Widgets useful for creating basic building blocks of text
//! user interfaces.
//!
//! ```no_run //tests do not provide a fully functional terminal
//! use unsegen::base::*;
//! use unsegen::widget::*;
//! use unsegen::widget::widgets::*;
//! use std::io::stdout;
//!
//! struct MyWidget {
//!     layout: VerticalLayout,
//!     prompt: PromptLine,
//!     buffer: LogViewer,
//! }
//!
//! impl Widget for MyWidget {
//!    fn space_demand(&self) -> Demand2D {
//!        let widgets: Vec<&Widget> = vec![&self.prompt, &self.buffer];
//!        self.layout.space_demand(widgets.as_slice())
//!    }
//!    fn draw(&self, window: Window, hints: RenderingHints) {
//!        let widgets: Vec<(&Widget, RenderingHints)> =
//!            vec![(&self.prompt, hints), (&self.buffer, hints.active(false))];
//!        self.layout.draw(window, widgets.as_slice());
//!    }
//! }
//!
//!
//! fn main() {
//!     let stdout = stdout();
//!     let mut term = Terminal::new(stdout.lock());
//!     let mut widget = MyWidget {
//!         layout: VerticalLayout::new(
//!             SeparatingStyle::AlternatingStyle(StyleModifier::new().invert(true))
//!         ),
//!         prompt: PromptLine::with_prompt(" > ".to_owned()),
//!         buffer: LogViewer::new(),
//!     };
//!
//!     loop {
//!         // Put application logic here: read input, chain behavior, react to other stuff
//!         {
//!             let win = term.create_root_window();
//!             widget.draw(win, RenderingHints::new().active(true));
//!         }
//!         term.present();
//!     }
//! }
//! ```
pub mod layouts;
pub mod widget;
pub mod widgets;

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
