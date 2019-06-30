//! `unsegen` is a library facilitating the creation of text user interface (TUI) applications akin to ncurses.
//!
//! Detailed examples can be found at the root of each of the four main modules.
#[macro_use]
extern crate ndarray;
extern crate nix;
extern crate raw_tty;
extern crate smallvec;
extern crate termion;
extern crate unicode_segmentation;
extern crate unicode_width;

pub mod base;
pub mod container;
pub mod input;
pub mod widget;
