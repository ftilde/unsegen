pub mod layouts;
pub mod linestorage;
pub mod widget;
pub mod widgets;

pub use self::layouts::*;
pub use self::linestorage::*;
pub use self::widget::*;

pub fn count_grapheme_clusters(text: &str) -> usize {
    use unicode_segmentation::UnicodeSegmentation;
    text.graphemes(true).count()
}

pub fn text_width(text: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    UnicodeWidthStr::width(text)
}
