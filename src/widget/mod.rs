//! Widget abstraction and some basic Widgets useful for creating basic building blocks of text
//! user interfaces.
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
