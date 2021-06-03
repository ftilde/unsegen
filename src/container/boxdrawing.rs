//! Utility functions for unicode box characters
use base::GraphemeCluster;

/// Components of unicode box characters. A single character can contain up to 4 segments.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum LineSegment {
    Up,
    Down,
    Right,
    Left,
}
impl LineSegment {
    /// c.f. CELL_TO_CHAR lookup table
    fn to_u8(self) -> u8 {
        match self {
            LineSegment::Up => 0b00000001,
            LineSegment::Down => 0b00000100,
            LineSegment::Right => 0b00010000,
            LineSegment::Left => 0b01000000,
        }
    }
}

/// The type of a segment of a unicode box character
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum LineType {
    None,
    Thin,
    Thick,
}
impl LineType {
    /// c.f. CELL_TO_CHAR lookup table
    fn to_u8(self) -> u8 {
        match self {
            LineType::None => 0b00,
            LineType::Thin => 0b01,
            LineType::Thick => 0b10,
        }
    }
}

/// A single box character, initially empty.
#[derive(Copy, Clone, Debug)]
pub struct LineCell {
    components: u8,
}

impl LineCell {
    /// Create an empty box drawing cell (i.e., a space character)
    /// Add segments using `set`.
    pub fn empty() -> Self {
        LineCell { components: 0 }
    }

    /// Convert the cell to a grapheme cluster (always safe).
    pub fn to_grapheme_cluster(self) -> GraphemeCluster {
        GraphemeCluster::try_from(CELL_TO_CHAR[self.components as usize])
            .expect("CELL_TO_CHAR elements are single clusters")
    }

    /// Set one of the four segments of the cell to the specified type.
    pub fn set(&mut self, segment: LineSegment, ltype: LineType) -> &mut Self {
        let segment = segment.to_u8();
        let ltype = ltype.to_u8();
        let other_component_mask = !(segment * 0b11);
        self.components = (self.components & other_component_mask) | segment * ltype;
        self
    }
}

#[cfg_attr(rustfmt, rustfmt_skip)]
const CELL_TO_CHAR: [char; 256] = [
    ' ', '╵', '╹', '╳',
    '╷', '│', '╿', '╳',
    '╻', '╽', '┃', '╳',
    '╳', '╳', '╳', '╳',
    '╶', '└', '┖', '╳',
    '┌', '├', '┞', '╳',
    '┎', '┟', '┠', '╳',
    '╳', '╳', '╳', '╳',
    '╺', '┕', '┗', '╳',
    '┍', '┝', '┡', '╳',
    '┏', '┢', '┣', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╴', '┘', '┚', '╳',
    '┐', '┤', '┦', '╳',
    '┒', '┧', '┨', '╳',
    '╳', '╳', '╳', '╳',
    '─', '┴', '┸', '╳',
    '┬', '┼', '╀', '╳',
    '┰', '╁', '╂', '╳',
    '╳', '╳', '╳', '╳',
    '╼', '┶', '┺', '╳',
    '┮', '┾', '╄', '╳',
    '┲', '╆', '╊', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╸', '┙', '┛', '╳',
    '┑', '┥', '┩', '╳',
    '┓', '┪', '┫', '╳',
    '╳', '╳', '╳', '╳',
    '╾', '┵', '┹', '╳',
    '┭', '┽', '╃', '╳',
    '┱', '╅', '╉', '╳',
    '╳', '╳', '╳', '╳',
    '━', '┷', '┻', '╳',
    '┯', '┿', '╇', '╳',
    '┳', '╈', '╋', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
    '╳', '╳', '╳', '╳',
];
