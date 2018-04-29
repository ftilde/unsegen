use std::cmp::min;
use std::cell::RefCell;
use std::fmt;
use std::fs::{File, Metadata};
use std::io;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::ops::{Range};
use base::ranges::{Bound, RangeArgument};
use base::basic_types::*;

pub trait LineStorage {
    type Line;
    fn view_line<I: Into<LineIndex>>(&self, pos: I) -> Option<Self::Line>;

    fn view<'a, I: Into<LineIndex>, R: RangeArgument<I>>(
        &'a self,
        range: R,
    ) -> Box<DoubleEndedIterator<Item = (LineIndex, Self::Line)> + 'a>
    where
        Self: ::std::marker::Sized,
    {
        // Not exactly sure, why this is needed... we only store a reference?!
        let start: LineIndex = match range.start() {
            // Always inclusive
            Bound::Unbound => LineIndex::new(0),
            Bound::Inclusive(i) => i.into(),
            Bound::Exclusive(i) => i.into() + 1,
        };
        let end: LineIndex = match range.end() {
            // Always exclusive
            Bound::Unbound => {
                //This is not particularly nice, but what can you do...
                let u_start: usize = start.into();
                let mut end = start;
                for i in u_start.. {
                    end += 1;
                    if self.view_line(LineIndex::new(i)).is_none() {
                        break;
                    }
                }
                end
            }
            Bound::Inclusive(i) => i.into() + 1,
            Bound::Exclusive(i) => i.into(),
        };
        let urange = Range::<usize> {
            start: start.into(),
            end: end.into(),
        };
        Box::new(LineStorageIterator::<Self::Line, Self>::new(self, urange))
    }
}

struct LineStorageIterator<'a, I: 'a, L: 'a + LineStorage<Line = I>> {
    storage: &'a L,
    range: Range<usize>,
}

impl<'a, I: 'a, L: 'a + LineStorage<Line = I>> LineStorageIterator<'a, I, L> {
    fn new(storage: &'a L, range: Range<usize>) -> Self {
        LineStorageIterator {
            storage: storage,
            range: range,
        }
    }
}
impl<'a, I: 'a, L: 'a + LineStorage<Line = I>> Iterator for LineStorageIterator<'a, I, L> {
    type Item = (LineIndex, I);
    fn next(&mut self) -> Option<Self::Item> {
        if self.range.start < self.range.end {
            let item_index = self.range.start;
            self.range.start += 1;
            if let Some(line) = self.storage.view_line(LineIndex::new(item_index)) {
                Some((LineIndex::new(item_index), line))
            } else {
                None
            }
        } else {
            None
        }
    }
}

impl<'a, I: 'a, L: 'a + LineStorage<Line = I>> DoubleEndedIterator
    for LineStorageIterator<'a, I, L>
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.range.start < self.range.end {
            let item_index = self.range.end - 1;
            self.range.end -= 1;
            if let Some(line) = self.storage.view_line(LineIndex::new(item_index)) {
                Some((LineIndex::new(item_index), line))
            } else {
                None
            }
        } else {
            None
        }
    }
}

pub struct MemoryLineStorage<L> {
    pub lines: Vec<L>,
}

impl<L> MemoryLineStorage<L> {
    pub fn new() -> Self {
        Self::with_lines(Vec::new())
    }

    pub fn with_lines(lines: Vec<L>) -> Self {
        MemoryLineStorage { lines: lines }
    }

    pub fn num_lines_stored(&self) -> usize {
        return self.lines.len();
    }
}

impl<L: Default> MemoryLineStorage<L> {
    pub fn active_line_mut(&mut self) -> &mut L {
        if self.lines.is_empty() {
            self.lines.push(L::default());
        }
        return self.lines.last_mut().expect("last line");
    }
}

impl<L: Clone> LineStorage for MemoryLineStorage<L> {
    type Line = L;
    fn view_line<I: Into<LineIndex>>(&self, pos: I) -> Option<L> {
        let upos: usize = pos.into().into();
        self.lines.get(upos).map(|s: &L| s.clone())
    }
}

pub type StringLineStorage = MemoryLineStorage<String>;

impl fmt::Write for StringLineStorage {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut s = s.to_owned();

        while let Some(newline_offset) = s.find('\n') {
            let mut line: String = s.drain(..(newline_offset + 1)).collect();
            line.pop(); //Remove the \n
            self.active_line_mut().push_str(&line);
            self.lines.push(String::new());
        }
        self.active_line_mut().push_str(&s);
        Ok(())
    }
}

pub struct FileLineStorage {
    reader: RefCell<BufReader<File>>,
    line_seek_positions: RefCell<Vec<usize>>,
    file_path: PathBuf,
}
impl FileLineStorage {
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let file = try!{File::open(path.as_ref())};
        Ok(FileLineStorage {
            reader: RefCell::new(BufReader::new(file)),
            line_seek_positions: RefCell::new(vec![0]),
            file_path: path.as_ref().to_path_buf(),
        })
    }

    pub fn get_file_path(&self) -> &Path {
        &self.file_path.as_path()
    }

    pub fn get_file_metadata(&self) -> ::std::io::Result<Metadata> {
        self.reader.borrow().get_ref().metadata()
    }

    fn get_line(&self, index: usize) -> Option<String> {
        let mut buffer = Vec::new();

        let mut line_seek_positions = self.line_seek_positions.borrow_mut();
        let mut reader = self.reader.borrow_mut();

        loop {
            let current_max_index: usize =
                line_seek_positions[min(index, line_seek_positions.len() - 1)];
            reader
                .seek(SeekFrom::Start(current_max_index as u64))
                .expect("seek to line pos");
            let n_bytes = reader.read_until(b'\n', &mut buffer).expect("read line");
            if n_bytes == 0 {
                //We reached EOF
                return None;
            }
            if index < line_seek_positions.len() {
                //We found the desired line
                let mut string = String::from_utf8_lossy(&buffer).into_owned();
                if string.as_str().bytes().last().unwrap_or(b'_') == b'\n' {
                    string.pop();
                }
                return Some(string);
            }
            line_seek_positions.push(current_max_index + n_bytes);
        }
    }
}

impl LineStorage for FileLineStorage {
    type Line = String;
    fn view_line<I: Into<LineIndex>>(&self, pos: I) -> Option<String> {
        self.get_line(pos.into().into())
    }
}
