use unicode_segmentation::{Graphemes, UnicodeSegmentation};
use smallvec::SmallVec;
use std::str::FromStr;

/// A single grapheme cluster encoded in utf8. It may consist of multiple bytes or even multiple chars. For details
/// on what a grapheme cluster is, read http://utf8everywhere.org/ or similar.
#[derive(Clone, Debug, PartialEq)]
pub struct GraphemeCluster {
    // Invariant: the contents of bytes is always valid utf8!
    bytes: SmallVec<[u8; 16]>,
}

impl GraphemeCluster {
    /// Get the underlying grapheme cluster as a String slice.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// assert_eq!(GraphemeCluster::try_from('a').unwrap().as_str(), "a");
    /// ```
    pub fn as_str<'a>(&'a self) -> &'a str {
        // This is safe because bytes is always valid utf8.
        unsafe { ::std::str::from_utf8_unchecked(&self.bytes) }
    }

    /// Helper: Create grapheme cluster from bytes. slice MUST be a single valid utf8 grapheme
    /// cluster.
    fn from_bytes(slice: &[u8]) -> Self {
        let vec = SmallVec::from_slice(slice);
        GraphemeCluster { bytes: vec }
    }

    /// Create a grapheme cluster from something string-like. string MUST be a single grapheme
    /// cluster.
    pub(in base) fn from_str_unchecked<S: AsRef<str>>(string: S) -> Self {
        Self::from_bytes(&string.as_ref().as_bytes()[..])
    }

    /// Create an empty (not actually real) grapheme cluster. This is used to pad cells in terminal
    /// window grids and not visible or usable outside.
    pub(in base) fn empty() -> Self {
        Self::from_str_unchecked("")
    }

    /// Add other to the current grapheme cluster. other MUST have a width of zero.
    pub(in base) fn merge_zero_width(&mut self, other: Self) {
        assert!(other.width() == 0, "Invalid merge");
        self.bytes.extend_from_slice(&other.bytes[..]);
    }

    /// Safely create a single space character (i.e., 0x20) grapheme cluster.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// assert_eq!(GraphemeCluster::space().as_str(), " ");
    /// ```
    pub fn space() -> Self {
        Self::from_str_unchecked(" ")
    }

    /// Replace the current cluster with a single space character (i.e., 0x20)
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// let mut cluster = GraphemeCluster::try_from('a').unwrap();
    /// cluster.clear();
    /// assert_eq!(cluster.as_str(), " ");
    /// ```
    pub fn clear(&mut self) {
        *self = Self::space();
    }

    /// Try to create a grapheme cluster from a character. If c is not a single grapheme cluster, a
    /// GraphemeClusterError is returned.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// assert_eq!(GraphemeCluster::try_from('a').unwrap().as_str(), "a");
    /// ```
    pub fn try_from(c: char) -> Result<Self, GraphemeClusterError> {
        Self::from_str(c.to_string().as_ref())
    }

    /// Retrieve all grapheme clusters from the given string.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// let mut clusters = GraphemeCluster::all_from_str("ab d");
    /// assert_eq!(clusters.next(), Some(GraphemeCluster::try_from('a').unwrap()));
    /// assert_eq!(clusters.next(), Some(GraphemeCluster::try_from('b').unwrap()));
    /// assert_eq!(clusters.next(), Some(GraphemeCluster::try_from(' ').unwrap()));
    /// assert_eq!(clusters.next(), Some(GraphemeCluster::try_from('d').unwrap()));
    /// assert_eq!(clusters.next(), None);
    /// ```
    pub fn all_from_str<'a>(string: &'a str) -> GraphemeClusterIter<'a> {
        GraphemeClusterIter::new(string)
    }

    /// Calculate the unicode width of the given grapheme cluster.
    ///
    /// Example:
    ///
    /// ```
    /// use unsegen::base::GraphemeCluster;
    /// assert_eq!(GraphemeCluster::try_from('a').unwrap().width(), 1);
    /// ```
    pub fn width(&self) -> usize {
        ::unicode_width::UnicodeWidthStr::width(self.as_str())
    }
}

/// An iterator over a sequence of grapheme clusters
pub struct GraphemeClusterIter<'a> {
    graphemes: Graphemes<'a>,
}

impl<'a> GraphemeClusterIter<'a> {
    fn new(string: &'a str) -> Self {
        GraphemeClusterIter {
            graphemes: string.graphemes(true),
        }
    }
}

impl<'a> Iterator for GraphemeClusterIter<'a> {
    type Item = GraphemeCluster;
    fn next(&mut self) -> Option<Self::Item> {
        self.graphemes.next().map(|s|
            // We trust the implementation of unicode_segmentation
            GraphemeCluster::from_str_unchecked(s))
    }
}

/// An error associated with the creation of GraphemeCluster from arbitrary strings.
#[derive(Debug)]
pub enum GraphemeClusterError {
    MultipleGraphemeClusters,
    NoGraphemeCluster,
}

/*
FIXME: TryFrom is still unstable: https://github.com/rust-lang/rust/issues/33417
impl TryFrom<char> for GraphemeCluster {
    type Err = GraphemeClusterError;
    fn try_from(text: char) -> Result<Self, Self::Err> {
        let mut clusters = text.graphemes(true);
        let res = if let Some(cluster) = clusters.next() {
            Self::from_str_unchecked(cluster)
        } else {
            Err(GraphemeClusterError::NoGraphemeCluster);
        };
    }
}
*/

impl FromStr for GraphemeCluster {
    type Err = GraphemeClusterError;
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut clusters = GraphemeCluster::all_from_str(text);
        let res = clusters
            .next()
            .ok_or(GraphemeClusterError::NoGraphemeCluster);
        if clusters.next().is_none() {
            res
        } else {
            Err(GraphemeClusterError::MultipleGraphemeClusters)
        }
    }
}
