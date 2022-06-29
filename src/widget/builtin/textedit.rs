//! A user-editable region of text.
use base::{BoolModifyMode, ColIndex, Cursor, LineIndex, StyleModifier, Width, Window};
use input::{Editable, Navigatable, OperationResult, Writable};
use ropey::{Rope, RopeSlice};
use std::ops::{Bound, RangeBounds};
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};
use widget::{text_width, Blink, Demand, Demand2D, RenderingHints, Widget};

/// A part of a text that can be moved to in a `TextEdit`
#[derive(Copy, Clone)]
pub enum TextElement {
    /// The current cursor position (useful for `delete`/`get` etc.)
    CurrentPosition,
    /// The first character of a WORD
    WORDBegin,
    /// The last character of a WORD
    WORDEnd,
    /// The first character of a Word
    WordBegin,
    /// The last character of a Word
    WordEnd,
    /// Roughly, a single "character"
    GraphemeCluster,
    /// Beginning or end of the line
    LineSeparator,
    /// Beginning or end of the document
    DocumentBoundary,
    /// First character of the a sentence
    Sentence,
}

#[derive(Copy, Clone)]
enum Direction {
    Forward,
    Backward,
}

/// A text location relative to the cursor of a `TextEdit`
#[derive(Copy, Clone)]
pub struct TextTarget {
    element: TextElement,
    direction: Direction,
    count: usize,
}

impl TextTarget {
    /// Construct a target after the cursor
    pub fn forward(element: TextElement) -> Self {
        TextTarget {
            element,
            direction: Direction::Forward,
            count: 1,
        }
    }
    /// Construct a target before the cursor
    pub fn backward(element: TextElement) -> Self {
        TextTarget {
            element,
            direction: Direction::Backward,
            count: 1,
        }
    }
    /// Construct a target exactly at the cursor position
    pub fn cursor() -> Self {
        TextTarget {
            element: TextElement::CurrentPosition,
            direction: Direction::Forward,
            count: 0,
        }
    }
    /// Refer to the n'th instead of the first occurence of a `TextElement` in the specified
    /// direction.
    pub fn nth(mut self, n: usize) -> Self {
        self.count = n;
        self
    }
}

fn pos(r: Result<TextPosition, TextPosition>) -> TextPosition {
    match r {
        Ok(p) => p,
        Err(p) => p,
    }
}

fn op_res(r: Result<TextPosition, TextPosition>) -> Result<(), ()> {
    match r {
        Ok(_) => Ok(()),
        Err(_) => Err(()),
    }
}

struct Text(Rope);

struct ClusterPair {
    p_left: TextPosition,
    left: String,
    p_middle: TextPosition,
    right: String,
}

fn find_word_begin(mut it: impl Iterator<Item = ClusterPair>) -> Option<TextPosition> {
    it.find_map(
        |p| match (classify_cluster(&p.left), classify_cluster(&p.right)) {
            (ClusterType::Whitespace, ClusterType::Other | ClusterType::Keyword)
            | (ClusterType::Keyword, ClusterType::Other)
            | (ClusterType::Other, ClusterType::Keyword) => Some(p.p_middle),
            (ClusterType::Other | ClusterType::Keyword, ClusterType::Whitespace)
            | (ClusterType::Keyword, ClusterType::Keyword)
            | (ClusterType::Whitespace, ClusterType::Whitespace)
            | (ClusterType::Other, ClusterType::Other) => None,
        },
    )
}

fn find_big_word_begin(mut it: impl Iterator<Item = ClusterPair>) -> Option<TextPosition> {
    it.find_map(|p| {
        if classify_cluster(&p.left) == ClusterType::Whitespace
            && classify_cluster(&p.right) != ClusterType::Whitespace
        {
            Some(p.p_middle)
        } else {
            None
        }
    })
}

fn find_word_end(mut it: impl Iterator<Item = ClusterPair>) -> Option<TextPosition> {
    it.find_map(
        |p| match (classify_cluster(&p.left), classify_cluster(&p.right)) {
            (ClusterType::Other | ClusterType::Keyword, ClusterType::Whitespace)
            | (ClusterType::Keyword, ClusterType::Other)
            | (ClusterType::Other, ClusterType::Keyword) => Some(p.p_left),
            (ClusterType::Whitespace, ClusterType::Other | ClusterType::Keyword)
            | (ClusterType::Keyword, ClusterType::Keyword)
            | (ClusterType::Whitespace, ClusterType::Whitespace)
            | (ClusterType::Other, ClusterType::Other) => None,
        },
    )
}

fn find_big_word_end(mut it: impl Iterator<Item = ClusterPair>) -> Option<TextPosition> {
    it.find_map(|p| {
        if classify_cluster(&p.left) != ClusterType::Whitespace
            && classify_cluster(&p.right) == ClusterType::Whitespace
        {
            Some(p.p_left)
        } else {
            None
        }
    })
}

fn find_sentence_boundary(mut it: impl Iterator<Item = ClusterPair>) -> Option<TextPosition> {
    it.find_map(|p| {
        if p.left == "." && classify_cluster(&p.right) == ClusterType::Whitespace {
            Some(p.p_left)
        } else {
            None
        }
    })
}

#[derive(Copy, Clone, PartialEq, Eq)]
enum ClusterType {
    Keyword,
    Whitespace,
    Other,
}

fn classify_cluster(s: &str) -> ClusterType {
    match s {
        " " | "\n" | "\t" => ClusterType::Whitespace,
        "_" => ClusterType::Keyword,
        s if s.chars().all(char::is_alphanumeric) => ClusterType::Keyword,
        _ => ClusterType::Other,
    }
}

impl Text {
    fn empty() -> Self {
        Text(Rope::new())
    }
    fn with_content(s: &str) -> Self {
        Text(Rope::from_str(s))
    }
    fn clusters_forward(&self, mut pos: TextPosition, n: usize) -> Result<TextPosition, ()> {
        for _ in 0..n {
            pos = self.next_grapheme_cluster(pos)?;
        }
        Ok(pos)
    }

    fn grapheme_clusters_forwards<'a>(
        &'a self,
        from: TextPosition,
    ) -> impl Iterator<Item = ClusterPair> + 'a {
        let mut p_left = from;
        let mut maybe_p_middle = self.next_grapheme_cluster(p_left).ok();
        std::iter::from_fn(move || {
            let p_middle = maybe_p_middle?;
            let p_right = self.next_grapheme_cluster(p_middle).ok()?;
            let p = ClusterPair {
                p_left,
                left: self.slice(p_left..p_middle).to_string(),
                p_middle,
                right: self.slice(p_middle..p_right).to_string(),
            };
            p_left = p_middle;
            maybe_p_middle = Some(p_right);
            Some(p)
        })
    }

    fn grapheme_clusters_backwards<'a>(
        &'a self,
        from: TextPosition,
    ) -> impl Iterator<Item = ClusterPair> + 'a {
        let mut p_middle = from;
        let mut p_right = self.next_grapheme_cluster(p_middle).unwrap_or(p_middle);
        std::iter::from_fn(move || {
            let p_left = self.prev_grapheme_cluster(p_middle).ok()?;
            let p = ClusterPair {
                p_left,
                left: self.slice(p_left..p_middle).to_string(),
                p_middle,
                right: self.slice(p_middle..p_right).to_string(),
            };
            p_right = p_middle;
            p_middle = p_left;
            Some(p)
        })
    }

    fn next_element(
        &self,
        begin: TextPosition,
        elm: TextElement,
    ) -> Result<TextPosition, TextPosition> {
        let mut clusters = self.grapheme_clusters_forwards(begin);
        let p = match elm {
            TextElement::CurrentPosition => Some(begin),
            TextElement::GraphemeCluster => self.next_grapheme_cluster(begin).ok(),
            TextElement::WORDBegin => find_big_word_begin(clusters),
            TextElement::WordBegin => find_word_begin(clusters),
            TextElement::WORDEnd => {
                if let Some(_) = clusters.next() {
                    find_big_word_end(clusters).or(self.prev_grapheme_cluster(self.end()).ok())
                } else {
                    None
                }
            }
            TextElement::WordEnd => {
                if let Some(_) = clusters.next() {
                    find_word_end(clusters).or(self.prev_grapheme_cluster(self.end()).ok())
                } else {
                    None
                }
            }
            TextElement::Sentence => {
                if let Some(p) = find_sentence_boundary(clusters) {
                    find_big_word_begin(self.grapheme_clusters_forwards(p))
                } else {
                    None
                }
            }
            TextElement::LineSeparator => Some(self.line_end(begin)),
            TextElement::DocumentBoundary => Some(self.end()),
        };
        let p = p.unwrap_or(self.end());
        if p != begin {
            Ok(p)
        } else {
            Err(p)
        }
    }

    fn prev_element(
        &self,
        begin: TextPosition,
        elm: TextElement,
    ) -> Result<TextPosition, TextPosition> {
        let mut clusters = self.grapheme_clusters_backwards(begin);
        let p = match elm {
            TextElement::CurrentPosition => Some(begin),
            TextElement::GraphemeCluster => self.prev_grapheme_cluster(begin).ok(),
            TextElement::WORDBegin => {
                if let Some(_) = clusters.next() {
                    find_big_word_begin(clusters)
                } else {
                    None
                }
            }
            TextElement::WordBegin => {
                if let Some(_) = clusters.next() {
                    find_word_begin(clusters)
                } else {
                    None
                }
            }
            TextElement::WORDEnd => find_big_word_end(clusters),
            TextElement::WordEnd => find_word_end(clusters),
            TextElement::Sentence => {
                if let Some(_) = clusters.next() {
                    find_big_word_begin(clusters)
                        .and_then(|p| find_sentence_boundary(self.grapheme_clusters_backwards(p)))
                        .and_then(|p| find_big_word_begin(self.grapheme_clusters_forwards(p)))
                } else {
                    None
                }
            }
            TextElement::LineSeparator => Some(self.line_begin(begin)),
            TextElement::DocumentBoundary => Some(self.begin()),
        };
        let p = p.unwrap_or(self.begin());
        if p != begin {
            Ok(p)
        } else {
            Err(p)
        }
    }

    fn resolve_target(
        &self,
        begin: TextPosition,
        target: TextTarget,
    ) -> Result<TextPosition, TextPosition> {
        let mut pos = begin;
        for i in 0..target.count {
            match target.direction {
                Direction::Forward => match self.next_element(pos, target.element) {
                    Ok(p) => pos = p,
                    Err(p) => {
                        return if i == 0 { Err(p) } else { Ok(p) };
                    }
                },
                Direction::Backward => match self.prev_element(pos, target.element) {
                    Ok(p) => pos = p,
                    Err(p) => {
                        return if i == 0 { Err(p) } else { Ok(p) };
                    }
                },
            }
        }
        Ok(pos)
    }

    fn next_grapheme_cluster(&self, pos: TextPosition) -> Result<TextPosition, ()> {
        let (mut chunk, mut chunk_begin, _, _) = self.0.chunk_at_byte(pos.0);
        let mut cursor = GraphemeCursor::new(pos.0, self.0.len_bytes(), true);
        loop {
            match cursor.next_boundary(chunk, chunk_begin) {
                Ok(None) => return Err(()),
                Ok(Some(n)) => return Ok(TextPosition(n)),
                Err(GraphemeIncomplete::NextChunk) => {
                    let (c, b, _, _) = self.0.chunk_at_byte(chunk_begin + chunk.len());
                    chunk = c;
                    chunk_begin = b;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (c, _, _, _) = self.0.chunk_at_byte(n - 1);
                    cursor.provide_context(c, n - c.len());
                }
                _ => unreachable!(),
            }
        }
    }
    fn prev_grapheme_cluster(&self, pos: TextPosition) -> Result<TextPosition, ()> {
        let (mut chunk, mut chunk_begin, _, _) = self.0.chunk_at_byte(pos.0);
        let mut cursor = GraphemeCursor::new(pos.0, self.0.len_bytes(), true);
        loop {
            match cursor.prev_boundary(chunk, chunk_begin) {
                Ok(None) => return Err(()),
                Ok(Some(n)) => return Ok(TextPosition(n)),
                Err(GraphemeIncomplete::PrevChunk) => {
                    let (c, b, _, _) = self.0.chunk_at_byte(chunk_begin - 1);
                    chunk = c;
                    chunk_begin = b;
                }
                Err(GraphemeIncomplete::PreContext(n)) => {
                    let (c, _, _, _) = self.0.chunk_at_byte(n - 1);
                    cursor.provide_context(c, n - c.len());
                }
                _ => unreachable!(),
            }
        }
    }
    fn cluster_in_line(&self, pos: TextPosition) -> usize {
        let begin = self.line_begin(pos);
        self.count_clusters(begin..pos)
    }
    fn count_clusters(&self, mut range: std::ops::Range<TextPosition>) -> usize {
        assert!(range.start <= range.end);
        let mut num = 0;
        while range.start < range.end {
            range.start = self.next_grapheme_cluster(range.start).unwrap();
            num += 1;
        }
        assert_eq!(range.start, range.end);
        num
    }
    fn begin(&self) -> TextPosition {
        TextPosition::begin()
    }
    fn end(&self) -> TextPosition {
        TextPosition(self.0.len_bytes())
    }

    fn line_index(&self, pos: TextPosition) -> LineIndex {
        LineIndex::new(self.0.byte_to_line(pos.0))
    }
    fn begin_of_line(&self, line: LineIndex) -> TextPosition {
        self.as_slice().begin_of_line(line)
    }
    fn num_lines(&self) -> usize {
        self.0.len_lines()
    }

    fn line_begin(&self, pos: TextPosition) -> TextPosition {
        self.as_slice().line_begin(pos)
    }
    fn line_end(&self, pos: TextPosition) -> TextPosition {
        self.as_slice().line_end(pos)
    }

    fn insert(&mut self, pos: TextPosition, s: &str) -> TextPosition {
        let ci = self.0.byte_to_char(pos.0);
        self.0.insert(ci, s);
        TextPosition(pos.0 + s.len())
    }

    fn remove(&mut self, pos: std::ops::Range<TextPosition>) {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        self.0.remove(ci_begin..ci_end);
    }

    fn slice(&self, pos: std::ops::Range<TextPosition>) -> TextSlice<'_> {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        TextSlice(self.0.slice(ci_begin..ci_end))
    }

    fn lines<'a>(&'a self) -> impl Iterator<Item = TextSlice<'a>> + 'a {
        self.as_slice().lines()
    }

    fn as_slice<'a>(&'a self) -> TextSlice<'a> {
        TextSlice(self.0.slice(..))
    }
}

#[derive(Clone, Copy)]
struct TextSlice<'a>(RopeSlice<'a>);
impl<'a> TextSlice<'a> {
    fn text_width(self) -> Width {
        //TODO: it's probably possible to make this more efficient...
        text_width(&self.to_string())
    }
    fn end(self) -> TextPosition {
        TextPosition(self.0.len_bytes())
    }

    fn line_begin(self, pos: TextPosition) -> TextPosition {
        let line = self.0.byte_to_line(pos.0);
        TextPosition(self.0.line_to_byte(line))
    }
    fn line_end(self, pos: TextPosition) -> TextPosition {
        let line = self.0.byte_to_line(pos.0);
        if line == self.0.len_lines() - 1 {
            //last line
            self.end()
        } else {
            let newline_len = 1;
            TextPosition(self.0.line_to_byte(line + 1) - newline_len)
        }
    }
    fn begin_of_line(self, line: LineIndex) -> TextPosition {
        TextPosition(self.0.line_to_byte(line.raw_value()))
    }
    fn slice(self, pos: std::ops::Range<TextPosition>) -> TextSlice<'a> {
        let ci_begin = self.0.byte_to_char(pos.start.0);
        let ci_end = self.0.byte_to_char(pos.end.0);
        TextSlice(self.0.slice(ci_begin..ci_end))
    }

    fn lines(self) -> impl Iterator<Item = TextSlice<'a>> + 'a {
        let num_lines = self.0.len_lines();
        let mut i = 0;
        std::iter::from_fn(move || {
            if i < num_lines {
                let begin = self.begin_of_line(LineIndex::new(i));
                let end = self.line_end(begin);
                i += 1;
                Some(self.slice(begin..end))
            } else {
                None
            }
        })
    }
}
impl std::fmt::Display for TextSlice<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for c in self.0.chunks() {
            write!(f, "{}", c)?;
        }
        Ok(())
    }
}

#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Debug)]
struct TextPosition(usize /* byte position */);

impl TextPosition {
    fn begin() -> Self {
        TextPosition(0)
    }
}

/// A user-editable region of text.
///
/// In addition to the current text, the `TextEdit` has a concept of a cursor whose position can
/// change, but is always on a grapheme cluster in the current text.
pub struct TextEdit {
    text: Text,
    cursor_pos: TextPosition,
}

impl TextEdit {
    /// Create empty TextEdit
    pub fn new() -> Self {
        TextEdit {
            text: Text::empty(),
            cursor_pos: TextPosition::begin(),
        }
    }

    /// Get the current content in the given range.
    pub fn get(&self, bounds: impl RangeBounds<TextTarget>) -> String {
        let s = self.resolve_range(bounds);
        self.text.slice(s.0..s.1).to_string()
    }

    /// Set (and overwrite) the current content. The cursor will be placed at the very end of the
    /// text.
    pub fn set(&mut self, text: impl AsRef<str>) {
        self.text = Text::with_content(text.as_ref());
        self.cursor_pos = self.text.end();
    }

    /// Remove the given range from the content.
    /// The cursor will be set to the beginning of the deleted range.
    pub fn delete(&mut self, bounds: impl RangeBounds<TextTarget>) {
        let s = self.resolve_range(bounds);
        self.text.remove(s.0..s.1);
        self.cursor_pos = s.0;
    }

    /// Move the cursor the specified position (relative to the current position).
    pub fn move_cursor_to(&mut self, target: TextTarget) -> Result<(), ()> {
        let r = self.text.resolve_target(self.cursor_pos, target);
        self.cursor_pos = pos(r);
        op_res(r)
    }

    fn resolve_range(&self, bounds: impl RangeBounds<TextTarget>) -> (TextPosition, TextPosition) {
        let start = match bounds.start_bound() {
            Bound::Included(t) => pos(self.text.resolve_target(self.cursor_pos, *t)),
            Bound::Excluded(t) => {
                let p = pos(self.text.resolve_target(self.cursor_pos, *t));
                self.text
                    .next_grapheme_cluster(p)
                    .unwrap_or(self.text.end())
            }
            Bound::Unbounded => self.text.begin(),
        };
        let end = match bounds.end_bound() {
            Bound::Included(t) => {
                let p = pos(self.text.resolve_target(self.cursor_pos, *t));
                self.text
                    .next_grapheme_cluster(p)
                    .unwrap_or(self.text.end())
            }
            Bound::Excluded(t) => pos(self.text.resolve_target(self.cursor_pos, *t)),
            Bound::Unbounded => self.text.end(),
        };
        (start, end)
    }

    fn move_cursor_down(&mut self) -> Result<(), ()> {
        let line = self.text.line_index(self.cursor_pos);
        if line.raw_value() + 1 < self.text.num_lines() {
            let pos_in_line = self.text.cluster_in_line(self.cursor_pos);
            let line_begin = self.text.begin_of_line(line + 1);
            let line_end = self.text.line_end(line_begin);
            let num_clusters = self.text.count_clusters(line_begin..line_end);

            let pos_in_line = pos_in_line.min(num_clusters);
            self.cursor_pos = self.text.clusters_forward(line_begin, pos_in_line).unwrap();

            Ok(())
        } else {
            Err(())
        }
    }

    fn move_cursor_right(&mut self) -> Result<(), ()> {
        if self.text.line_end(self.cursor_pos) == self.cursor_pos {
            Err(())
        } else {
            self.cursor_pos = self.text.next_grapheme_cluster(self.cursor_pos)?;
            Ok(())
        }
    }

    fn move_cursor_up(&mut self) -> Result<(), ()> {
        let line = self.text.line_index(self.cursor_pos);
        if line.raw_value() > 0 {
            let pos_in_line = self.text.cluster_in_line(self.cursor_pos);
            let line_begin = self.text.begin_of_line(line - 1);
            let line_end = self.text.line_end(line_begin);
            let num_clusters = self.text.count_clusters(line_begin..line_end);

            let pos_in_line = pos_in_line.min(num_clusters);
            self.cursor_pos = self.text.clusters_forward(line_begin, pos_in_line).unwrap();

            Ok(())
        } else {
            Err(())
        }
    }

    fn move_cursor_left(&mut self) -> Result<(), ()> {
        if self.text.line_begin(self.cursor_pos) == self.cursor_pos {
            Err(())
        } else {
            self.cursor_pos = self.text.prev_grapheme_cluster(self.cursor_pos)?;
            Ok(())
        }
    }

    /// Insert text directly *before* the current cursor position
    pub fn insert(&mut self, text: &str) {
        self.text.insert(self.cursor_pos, text);
    }

    /// Returns the byte position of the cursor in the current line
    pub fn cursor_byte_pos_in_line(&self) -> usize {
        self.cursor_pos.0 - self.text.line_begin(self.cursor_pos).0
    }

    /// Prepare for drawing as a `Widget`.
    pub fn as_widget<'a>(&'a self) -> TextEditWidget<'a> {
        TextEditWidget {
            textedit: self,
            cursor_style_active_blink_on: StyleModifier::new().invert(BoolModifyMode::Toggle),
            cursor_style_active_blink_off: StyleModifier::new(),
            cursor_style_inactive: StyleModifier::new().underline(true),
        }
    }
}

/// Note that there is no concept of moving up or down for a `TextEdit`.
impl Navigatable for TextEdit {
    fn move_up(&mut self) -> OperationResult {
        self.move_cursor_up()
    }
    fn move_down(&mut self) -> OperationResult {
        self.move_cursor_down()
    }
    fn move_left(&mut self) -> OperationResult {
        self.move_cursor_left()
    }
    fn move_right(&mut self) -> OperationResult {
        self.move_cursor_right()
    }
}

impl Writable for TextEdit {
    fn write(&mut self, c: char) -> OperationResult {
        self.insert(&c.to_string());
        self.cursor_pos = self.text.next_grapheme_cluster(self.cursor_pos).unwrap();
        Ok(())
    }
}

impl Editable for TextEdit {
    fn delete_forwards(&mut self) -> OperationResult {
        //i.e., "del" key
        let start = self.cursor_pos;
        let end = self.text.next_grapheme_cluster(start)?;
        self.text.remove(start..end);
        Ok(())
    }
    fn delete_backwards(&mut self) -> OperationResult {
        //i.e., "backspace"
        let end = self.cursor_pos;
        let start = self.text.prev_grapheme_cluster(end)?;
        self.text.remove(start..end);
        self.cursor_pos = start;
        Ok(())
    }
    fn go_to_beginning_of_line(&mut self) -> OperationResult {
        self.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
    }
    fn go_to_end_of_line(&mut self) -> OperationResult {
        self.move_cursor_to(TextTarget::forward(TextElement::LineSeparator))
    }
    fn clear(&mut self) -> OperationResult {
        if self.text.0.len_bytes() == 0 {
            Err(())
        } else {
            self.text = Text::empty();
            self.cursor_pos = TextPosition::begin();
            Ok(())
        }
    }
}

/// A `Widget` representing a `TextEdit`
///
/// It allows for customization of cursor styles.
pub struct TextEditWidget<'a> {
    textedit: &'a TextEdit,
    cursor_style_active_blink_on: StyleModifier,
    cursor_style_active_blink_off: StyleModifier,
    cursor_style_inactive: StyleModifier,
}

impl<'a> TextEditWidget<'a> {
    /// Define the style that the cursor will be drawn with on the "on" tick when the widget is
    /// active.
    pub fn cursor_blink_on(mut self, style: StyleModifier) -> Self {
        self.cursor_style_active_blink_on = style;
        self
    }

    /// Define the style that the cursor will be drawn with on the "off" tick when the widget is
    /// active.
    pub fn cursor_blink_off(mut self, style: StyleModifier) -> Self {
        self.cursor_style_active_blink_off = style;
        self
    }

    /// Define the style that the cursor will be drawn with when the widget is inactive.
    pub fn cursor_inactive(mut self, style: StyleModifier) -> Self {
        self.cursor_style_inactive = style;
        self
    }
}

impl<'a> Widget for TextEditWidget<'a> {
    fn space_demand(&self) -> Demand2D {
        Demand2D {
            width: Demand::at_least(
                self.textedit
                    .text
                    .lines()
                    .map(|l| l.text_width())
                    .max()
                    .unwrap()
                    + 1,
            ),
            height: Demand::exact(self.textedit.text.num_lines()),
        }
    }
    fn draw(&self, mut window: Window, hints: RenderingHints) {
        let height = window.get_height();
        let before_cursor = self.textedit.cursor_pos;
        let line_begin = self.textedit.text.line_begin(before_cursor);
        let line_end = self.textedit.text.line_end(before_cursor);
        let after_cursor = self
            .textedit
            .text
            .next_grapheme_cluster(before_cursor)
            .ok()
            .filter(|a| a <= &line_end);

        let right_padding = 1;
        let lower_padding = 1;

        let text_width_before_cursor = self
            .textedit
            .text
            .slice(line_begin..before_cursor)
            .text_width();
        let draw_cursor_start_pos = ::std::cmp::min(
            ColIndex::new(0),
            (window.get_width() - text_width_before_cursor - right_padding).from_origin(),
        );

        let cursor_style = match (hints.active, hints.blink) {
            (true, Blink::On) => self.cursor_style_active_blink_on,
            (true, Blink::Off) => self.cursor_style_active_blink_off,
            (false, _) => self.cursor_style_inactive,
        };

        let current_line = self.textedit.text.line_index(self.textedit.cursor_pos);
        let num_following_lines = self.textedit.text.num_lines() - current_line.raw_value() - 1;
        let cursor_row = (height - 1 - lower_padding.min(num_following_lines as i32))
            .min((current_line.raw_value() as i32).into())
            .max(0.into())
            .from_origin();

        let mut cursor = Cursor::new(&mut window).position(draw_cursor_start_pos, cursor_row);
        cursor.set_line_start_column(draw_cursor_start_pos);

        use std::fmt::Write;
        if let Some(after_cursor) = after_cursor {
            let _ = write!(
                cursor,
                "{}",
                self.textedit.text.slice(line_begin..before_cursor)
            );
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                let _ = write!(
                    cursor,
                    "{}",
                    self.textedit.text.slice(before_cursor..after_cursor)
                );
            }
            let _ = write!(
                cursor,
                "{}",
                self.textedit.text.slice(after_cursor..line_end)
            );
        } else {
            let _ = write!(cursor, "{}", self.textedit.text.slice(line_begin..line_end));
            {
                let mut cursor = cursor.save().style_modifier();
                cursor.apply_style_modifier(cursor_style);
                cursor.write(" ");
            }
        }
        cursor.wrap_line();

        cursor.move_to_x(draw_cursor_start_pos);
        for line in self
            .textedit
            .text
            .slice(self.textedit.text.begin_of_line(current_line + 1)..self.textedit.text.end())
            .lines()
        {
            if cursor.get_row() >= height.from_origin() {
                break;
            }
            let _ = writeln!(cursor, "{}", line);
        }

        cursor.move_to_y(0.into());
        cursor.move_to_x(draw_cursor_start_pos);
        let num_rows_above = cursor_row.raw_value() as usize;
        assert!(num_rows_above <= current_line.raw_value());
        let first_line_begin = self
            .textedit
            .text
            .begin_of_line(current_line - num_rows_above);
        let last_line_end = line_begin;
        if current_line.raw_value() > 0 {
            for line in self
                .textedit
                .text
                .slice(first_line_begin..last_line_end)
                .lines()
            {
                let _ = writeln!(cursor, "{}", line);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::base::grapheme_cluster::GraphemeCluster;

    use super::*;
    use base::test::FakeTerminal;

    fn test_textedit<F: Fn(&mut TextEdit)>(window_dim: (u32, u32), after: &str, action: F) {
        let mut term = FakeTerminal::with_size(window_dim);
        {
            let mut window = term.create_root_window();
            window.fill(GraphemeCluster::try_from('_').unwrap());
            let mut textedit = TextEdit::new();
            action(&mut textedit);
            textedit
                .as_widget()
                .cursor_blink_on(StyleModifier::new().bold(true))
                .draw(
                    window,
                    RenderingHints::default().active(true).blink(Blink::On),
                );
        }
        term.assert_looks_like(after);
    }

    #[test]
    fn test_set_truncate() {
        test_textedit((3, 1), "d* *_", |t| {
            t.set("abc");
            t.set("d");
        });
    }

    #[test]
    fn test_get() {
        test_textedit((5, 2), "*a*b cd|efg h", |t| {
            t.set("ab cd\nefg h");
            assert_eq!(t.get(..), "ab cd\nefg h");
            assert_eq!(
                t.get(TextTarget::backward(TextElement::GraphemeCluster).nth(2)..),
                " h"
            );
            assert_eq!(t.get(TextTarget::backward(TextElement::WORDBegin)..), "h");
            assert_eq!(
                t.get(TextTarget::backward(TextElement::LineSeparator)..),
                "efg h"
            );

            t.move_cursor_to(TextTarget::backward(TextElement::DocumentBoundary))
                .unwrap();
            assert_eq!(t.get(..), "ab cd\nefg h");
            assert_eq!(
                t.get(..TextTarget::backward(TextElement::LineSeparator)),
                ""
            );
            assert_eq!(
                t.get(..=TextTarget::backward(TextElement::LineSeparator)),
                "a"
            );
            assert_eq!(
                t.get(..=TextTarget::forward(TextElement::WORDEnd).nth(2)),
                "ab cd"
            );
            assert_eq!(
                t.get(..TextTarget::forward(TextElement::LineSeparator).nth(2)),
                "ab cd"
            );
        });
    }

    #[test]
    fn test_delete() {
        test_textedit((5, 2), "* *____|_____", |t| {
            t.set("ab cd\nefg h");
            t.delete(..)
        });
        test_textedit((5, 2), "ab * *_|efg h", |t| {
            t.set("ab cd\nefg h");
            t.move_cursor_to(TextTarget::backward(TextElement::DocumentBoundary))
                .unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDBegin))
                .unwrap();

            t.delete(TextTarget::cursor()..TextTarget::forward(TextElement::LineSeparator))
        });
    }

    #[test]
    fn test_single_line_simple() {
        test_textedit((5, 1), "abc* *_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
        });
        test_textedit((5, 1), "a沐c* *", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
        });
    }

    #[test]
    fn test_single_line_move() {
        test_textedit((5, 1), "abd*c*_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.write('d').unwrap();
        });
        test_textedit((5, 1), "ab沐*c*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.write('沐').unwrap();
        });
        test_textedit((5, 1), "ad*b*c_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
            t.move_cursor_right().unwrap();
            t.write('d').unwrap();
        });
        test_textedit((5, 1), "abc* *_", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::LineSeparator))
                .unwrap();
        });
    }

    #[test]
    fn test_single_line_delete() {
        test_textedit((5, 1), "a*c*___", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_forwards().unwrap();
        });
        test_textedit((5, 1), "*沐***c__", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_forwards().unwrap();
        });
    }

    #[test]
    fn test_single_line_backspace() {
        test_textedit((5, 1), "*b*c___", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.move_cursor_left().unwrap();
            t.delete_backwards().unwrap();
        });
        test_textedit((5, 1), "a*c*___", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.move_cursor_left().unwrap();
            t.delete_backwards().unwrap();
        });
    }

    #[test]
    fn test_single_line_long() {
        //TODO: This is broken, but probably somewhere else? window? cursor?
        //test_textedit((5, 1), " def* *", |t| {
        //    t.write('a').unwrap();
        //    t.write('b').unwrap();
        //    t.write('沐').unwrap();
        //    t.write('d').unwrap();
        //    t.write('e').unwrap();
        //    t.write('f').unwrap();
        //});
        test_textedit((5, 1), "cdef* *", |t| {
            t.write('a').unwrap();
            t.write('沐').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
        });
        test_textedit((5, 1), "沐de*f*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('沐').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
            t.move_cursor_left().unwrap();
        });
        test_textedit((5, 1), "*a*b沐d", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('沐').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.write('f').unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
        });
    }

    #[test]
    fn test_multi_line_simple() {
        test_textedit((2, 5), "a_|b_|c_|d_|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
            t.write('d').unwrap();
            t.write('\n').unwrap();
            t.write('e').unwrap();
        });

        test_textedit((2, 5), "a_|b_|c_|* *_|__", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
        });

        test_textedit((2, 3), "c_|d_|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('\n').unwrap();
            t.write('d').unwrap();
            t.write('\n').unwrap();
            t.write('e').unwrap();
        });
    }

    #[test]
    fn test_multi_line_long() {
        test_textedit((2, 3), "__|__|e* *", |t| {
            t.write('a').unwrap();
            t.write('\n').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
        });

        test_textedit((2, 3), "b_|__|d* *", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
        });

        test_textedit((2, 3), "ab|o_|c*d*", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.move_cursor_left().unwrap();
        });

        test_textedit((2, 3), "ab|o_|*c*d", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
        });
    }

    #[test]
    fn test_multi_line_move() {
        test_textedit((2, 3), "ab|*o*_|cd", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
            t.move_cursor_up().unwrap();
        });

        test_textedit((2, 3), "ab|o* *|cd", |t| {
            t.write('a').unwrap();
            t.write('b').unwrap();
            t.write('\n').unwrap();
            t.write('o').unwrap();
            t.write('\n').unwrap();
            t.write('c').unwrap();
            t.write('d').unwrap();
            t.write('e').unwrap();
            t.move_cursor_up().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
            assert!(t.move_cursor_left().is_err());
            t.move_cursor_to(TextTarget::forward(TextElement::LineSeparator))
                .unwrap();
            assert!(t.move_cursor_right().is_err());
        });
    }

    #[test]
    fn test_move_big_word_begin_forward() {
        test_textedit((6, 1), "abc *d*e", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDBegin))
                .unwrap();
        });

        test_textedit((7, 1), "abc de* *", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDBegin).nth(2))
                .unwrap();
        });

        test_textedit((7, 1), "abc de* *", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDBegin).nth(3))
                .unwrap();
        });

        test_textedit((8, 1), "abc de* *_", |t| {
            t.set("abc de");
            assert!(t
                .move_cursor_to(TextTarget::forward(TextElement::WORDBegin))
                .is_err());
        });
    }

    #[test]
    fn test_move_word_begin_forward() {
        test_textedit((6, 1), "a_c *d*e", |t| {
            t.set("a_c de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordBegin))
                .unwrap();
        });

        test_textedit((6, 1), "abc *+*e", |t| {
            t.set("abc +e");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordBegin))
                .unwrap();
        });

        test_textedit((6, 1), "ab*+* +e", |t| {
            t.set("ab+ +e");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordBegin))
                .unwrap();
        });

        test_textedit((6, 1), "+*b*+ +e", |t| {
            t.set("+b+ +e");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordBegin))
                .unwrap();
        });

        test_textedit((6, 1), "+-+ *+*e", |t| {
            t.set("+-+ +e");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordBegin))
                .unwrap();
        });
    }

    #[test]
    fn test_move_big_word_begin_backward() {
        test_textedit((6, 1), "abc *d*e", |t| {
            t.set("abc de");
            t.move_cursor_to(TextTarget::backward(TextElement::WORDBegin))
                .unwrap();
        });
        test_textedit((6, 1), "*a*bc de", |t| {
            t.set("abc de");
            t.move_cursor_to(TextTarget::backward(TextElement::WORDBegin).nth(2))
                .unwrap();
        });
        test_textedit((6, 1), "*a*bc de", |t| {
            t.set("abc de");
            t.move_cursor_to(TextTarget::backward(TextElement::WORDBegin).nth(3))
                .unwrap();
        });
        test_textedit((6, 1), "*a*bc de", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            assert!(t
                .move_cursor_to(TextTarget::backward(TextElement::WORDBegin))
                .is_err());
        });
    }

    #[test]
    fn test_move_word_begin_backward() {
        test_textedit((6, 1), "abc *d*_", |t| {
            t.set("abc d_");
            t.move_cursor_to(TextTarget::backward(TextElement::WordBegin))
                .unwrap();
        });
        test_textedit((6, 1), "*-*+- de", |t| {
            t.set("-+- de");
            t.move_cursor_to(TextTarget::backward(TextElement::WordBegin).nth(2))
                .unwrap();
        });
        test_textedit((6, 1), "a*+*- de", |t| {
            t.set("a+- de");
            t.move_cursor_to(TextTarget::backward(TextElement::WordBegin).nth(2))
                .unwrap();
        });
    }

    #[test]
    fn test_move_big_word_end_forward() {
        test_textedit((6, 1), "ab*c* de", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDEnd))
                .unwrap();
        });

        test_textedit((6, 1), "abc d*e*", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_right().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDEnd))
                .unwrap();
        });

        test_textedit((7, 1), "abc d*e* ", |t| {
            t.set("abc de ");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDEnd).nth(2))
                .unwrap();
        });

        test_textedit((8, 1), "abc de* *_", |t| {
            t.set("abc de ");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WORDEnd).nth(3))
                .unwrap();
        });
    }

    #[test]
    fn test_move_word_end_forward() {
        test_textedit((6, 1), "ab*c* de", |t| {
            t.set("abc de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordEnd))
                .unwrap();
        });

        test_textedit((6, 1), "a*b*- de", |t| {
            t.set("ab- de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordEnd))
                .unwrap();
        });

        test_textedit((6, 1), "-*b*- de", |t| {
            t.set("-b- de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordEnd))
                .unwrap();
        });

        test_textedit((6, 1), "-+*-* de", |t| {
            t.set("-+- de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordEnd))
                .unwrap();
        });

        test_textedit((6, 1), "-+- d*e*", |t| {
            t.set("-+- de");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::WordEnd).nth(2))
                .unwrap();
        });
    }

    #[test]
    fn test_move_big_word_end_backward() {
        test_textedit((6, 1), "ab*c* de", |t| {
            t.set("abc de");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WORDEnd).nth(1))
                .unwrap();
        });

        test_textedit((6, 1), "*a*bc de", |t| {
            t.set("abc de");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WORDEnd).nth(2))
                .unwrap();
        });
    }

    #[test]
    fn test_move_word_end_backward() {
        test_textedit((6, 1), "ab*c* _e", |t| {
            t.set("abc _e");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WordEnd).nth(1))
                .unwrap();
        });

        test_textedit((6, 1), "*a*bc _e", |t| {
            t.set("abc _e");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WordEnd).nth(2))
                .unwrap();
        });

        test_textedit((6, 1), "a*-*c _e", |t| {
            t.set("a-c _e");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WordEnd).nth(2))
                .unwrap();
        });

        test_textedit((6, 1), "*a*-+ _e", |t| {
            t.set("a-+ _e");
            t.move_cursor_left().unwrap();
            t.move_cursor_to(TextTarget::backward(TextElement::WordEnd).nth(2))
                .unwrap();
        });
    }

    #[test]
    fn test_move_line_sep_forward() {
        test_textedit((3, 2), "ab* *|c__", |t| {
            t.set("ab\nc");
            t.move_cursor_up().unwrap();
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::LineSeparator))
                .unwrap();
            assert!(t
                .move_cursor_to(TextTarget::forward(TextElement::LineSeparator))
                .is_err());
        });
    }

    #[test]
    fn test_move_line_sep_backward() {
        test_textedit((3, 2), "ab_|*c*__", |t| {
            t.set("ab\nc");
            t.move_cursor_to(TextTarget::backward(TextElement::LineSeparator))
                .unwrap();
        });
    }

    #[test]
    fn test_move_sentence_forward() {
        test_textedit((13, 1), "abc. *d*ef. ghi", |t| {
            t.set("abc. def. ghi");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::Sentence))
                .unwrap();
        });

        test_textedit((13, 1), "abc. def. *g*hi", |t| {
            t.set("abc. def. ghi");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::Sentence).nth(2))
                .unwrap();
        });

        test_textedit((14, 1), "abc. def. ghi* *", |t| {
            t.set("abc. def. ghi");
            t.go_to_beginning_of_line().unwrap();
            t.move_cursor_to(TextTarget::forward(TextElement::Sentence).nth(3))
                .unwrap();
        });
    }

    #[test]
    fn test_move_sentence_backward() {
        test_textedit((13, 1), "abc. def. *g*hi", |t| {
            t.set("abc. def. ghi");
            t.move_cursor_to(TextTarget::backward(TextElement::Sentence))
                .unwrap();
        });
        test_textedit((13, 1), "abc. *d*ef. ghi", |t| {
            t.set("abc. def. ghi");
            t.move_cursor_to(TextTarget::backward(TextElement::Sentence).nth(2))
                .unwrap();
        });
        test_textedit((13, 1), "*a*bc. def. ghi", |t| {
            t.set("abc. def. ghi");
            t.move_cursor_to(TextTarget::backward(TextElement::Sentence).nth(3))
                .unwrap();
            assert!(t
                .move_cursor_to(TextTarget::backward(TextElement::Sentence).nth(3))
                .is_err());
        });
    }
}
