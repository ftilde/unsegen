//! Basic terminal rendering including Terminal setup, "slicing" using Windows, and formatted
//! writing to Windows using Cursors.
//!
//! # Example:
//! ```no_run
//! use unsegen::base::*;
//! use std::io::stdout;
//! use std::fmt::Write;
//!
//! let stdout = stdout();
//! let mut term = Terminal::new(stdout.lock()).unwrap();
//!
//! {
//!     let win = term.create_root_window();
//!     let (left, mut right) = win.split(ColIndex::new(5)).unwrap();
//!
//!     let (mut top, mut bottom) = left.split(RowIndex::new(2)).unwrap();
//!
//!     top.fill(GraphemeCluster::try_from('X').unwrap());
//!
//!     bottom.modify_default_style(StyleModifier::new().fg_color(Color::Green));
//!     bottom.fill(GraphemeCluster::try_from('O').unwrap());
//!
//!     let mut cursor = Cursor::new(&mut right)
//!         .position(ColIndex::new(1), RowIndex::new(2))
//!         .wrapping_mode(WrappingMode::Wrap)
//!         .style_modifier(StyleModifier::new().bold(true).bg_color(Color::Red));
//!
//!     writeln!(cursor, "Hi there!").unwrap();
//! }
//! term.present();
//! ```

pub mod basic_types;
pub mod cursor;
pub mod grapheme_cluster;
pub mod style;
pub mod terminal;
pub mod window;

pub use self::basic_types::*;
pub use self::cursor::*;
pub use self::grapheme_cluster::*;
pub use self::style::*;
pub use self::terminal::*;
pub use self::window::*;
