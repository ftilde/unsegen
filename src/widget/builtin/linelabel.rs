//! A single line text label.
use super::super::{ColDemand, Demand2D, RenderingHints, RowDemand, Widget};
use base::{Cursor, Window};
use widget::count_grapheme_clusters;

/// A single line text label.
///
/// The label does not provide any means for user interaction, but the label text can be changed
/// programmatically.
///
/// If multiple lines are added to the label, there is no guarantee that further lines will be
/// displayed.
pub struct LineLabel {
    text: String,
}
impl LineLabel {
    /// Create a label with the specified content. `text` should not contain multiple lines.
    pub fn new<S: Into<String>>(text: S) -> Self {
        LineLabel { text: text.into() }
    }

    /// Change the content of the label. `text` should not contain multiple lines.
    pub fn set<S: Into<String>>(&mut self, text: S) {
        self.text = text.into();
    }
}

impl Widget for LineLabel {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: ColDemand::exact(count_grapheme_clusters(&self.text)),
            height: RowDemand::exact(1),
        }
    }
    fn draw(&self, mut window: Window, _: RenderingHints) {
        let mut cursor = Cursor::new(&mut window);
        cursor.write(&self.text);
    }
}
