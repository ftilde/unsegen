//! The entry point module for presenting data to the terminal.
//!
//! To get started, create a terminal, create a root window from it, use it to render stuff to the
//! terminal and finally present the terminal content to the physical terminal.
//!
//! # Examples:
//!
//! ```no_run //tests do not provide a fully functional terminal
//! use unsegen::base::Terminal;
//! use std::io::stdout;
//! let stdout = stdout();
//! let mut term = Terminal::new(stdout.lock()).unwrap();
//!
//! let mut done = false;
//! while !done {
//!     // Process data, update data structures
//!     done = true; // or whatever condition you like
//!
//!     {
//!         let win = term.create_root_window();
//!         // use win to draw something
//!     }
//!     term.present();
//!
//! }
//! ```
use base::{Height, Style, Width, Window, WindowBuffer};
use ndarray::Axis;
use raw_tty::TtyWithGuard;
use std::io;
use std::io::{StdoutLock, Write};
use std::os::unix::io::AsRawFd;
use termion;

use nix::sys::signal::{killpg, pthread_sigmask, SigSet, SigmaskHow, SIGCONT, SIGTSTP};
use nix::unistd::getpgrp;

/// A type providing an interface to the underlying physical terminal.
/// This also provides the entry point for any rendering to the terminal buffer.
pub struct Terminal<'a, T = StdoutLock<'a>>
where
    T: AsRawFd + Write,
{
    values: WindowBuffer,
    terminal: TtyWithGuard<T>,
    size_has_changed_since_last_present: bool,
    bell_to_emit: bool,
    _phantom: ::std::marker::PhantomData<&'a ()>,
}

impl<'a, T: Write + AsRawFd> Terminal<'a, T> {
    /// Create a new terminal. The terminal takes control of the provided io sink (usually stdout)
    /// and performs all output on it.
    ///
    /// If the terminal cannot be created (e.g., because the provided io sink does not allow for
    /// setting up raw mode), the error is returned.
    pub fn new(sink: T) -> io::Result<Self> {
        let mut terminal = TtyWithGuard::new(sink)?;
        terminal.set_raw_mode()?;
        let mut term = Terminal {
            values: WindowBuffer::new(Width::new(0).unwrap(), Height::new(0).unwrap()),
            terminal,
            size_has_changed_since_last_present: true,
            bell_to_emit: false,
            _phantom: Default::default(),
        };
        term.enter_tui()?;
        Ok(term)
    }

    /// This method is intended to be called when the process received a SIGTSTP.
    ///
    /// The terminal state is restored, and the process is actually stopped within this function.
    /// When the process then receives a SIGCONT it sets up the terminal state as expected again
    /// and returns from the function.
    ///
    /// The usual way to deal with SIGTSTP (and signals in general) is to block them and `waidpid`
    /// for them in a separate thread which sends the events into some fifo. The fifo can be polled
    /// in an event loop. Then, if in the main event loop a SIGTSTP turns up, *this* function
    /// should be called.
    pub fn handle_sigtstp(&mut self) -> io::Result<()> {
        self.leave_tui()?;

        let mut stop_and_cont = SigSet::empty();
        stop_and_cont.add(SIGCONT);
        stop_and_cont.add(SIGTSTP);

        // 1. Unblock SIGTSTP and SIGCONT, so that we actually stop when we receive another SIGTSTP
        pthread_sigmask(SigmaskHow::SIG_UNBLOCK, Some(&stop_and_cont), None)?;

        // 2. Reissue SIGTSTP (this time to whole the process group!)...
        killpg(getpgrp(), SIGTSTP)?;
        // ... and stop!
        // Now we are waiting for a SIGCONT.

        // 3. Once we receive a SIGCONT we block SIGTSTP and SIGCONT again and resume.
        pthread_sigmask(SigmaskHow::SIG_BLOCK, Some(&stop_and_cont), None)?;

        self.enter_tui()
    }

    /// Set up the terminal for "full screen" work (i.e., hide cursor, switch to alternate screen).
    fn enter_tui(&mut self) -> io::Result<()> {
        write!(
            self.terminal,
            "{}{}",
            termion::screen::ToAlternateScreen,
            termion::cursor::Hide
        )?;
        self.terminal.set_raw_mode()?;
        self.terminal.flush()?;
        Ok(())
    }

    /// Restore terminal from "full screen" (i.e., show cursor again, switch to main screen).
    fn leave_tui(&mut self) -> io::Result<()> {
        write!(
            self.terminal,
            "{}{}",
            termion::screen::ToMainScreen,
            termion::cursor::Show
        )?;
        self.terminal.modify_mode(|m| m)?; //Restore saved mode
        self.terminal.flush()?;
        Ok(())
    }

    /// Temporarily switch back to main terminal screen, restore terminal state, then execute `f`
    /// and subsequently switch back to tui mode again.
    ///
    /// In other words: Execute a function `f` in "normal" terminal mode. This can be useful if the
    /// application executes a subprocess that is expected to take control of the tty temporarily.
    pub fn on_main_screen<R, F: FnOnce() -> R>(&mut self, f: F) -> io::Result<R> {
        self.leave_tui()?;
        let res = f();
        self.enter_tui()?;
        Ok(res)
    }

    /// Create a root window that covers the whole terminal grid.
    ///
    /// Use the buffer to manipulate the current window buffer and use present subsequently to
    /// write out the buffer to the actual terminal.
    pub fn create_root_window(&mut self) -> Window {
        let (x, y) = termion::terminal_size().expect("get terminal size");
        let x = Width::new(x as i32).unwrap();
        let y = Height::new(y as i32).unwrap();
        if x != self.values.as_window().get_width() || y != self.values.as_window().get_height() {
            self.size_has_changed_since_last_present = true;
            self.values = WindowBuffer::new(x, y);
        } else {
            self.values.as_window().clear();
        }

        self.values.as_window()
    }

    /// Emit a bell character ('\a') on the next call to `present`.
    ///
    /// This will usually set an urgent hint on the terminal emulator, so it is useful to draw
    /// attention to the application.
    pub fn emit_bell(&mut self) {
        self.bell_to_emit = true;
    }

    /// Present the current buffer content to the actual terminal.
    pub fn present(&mut self) {
        let mut current_style = Style::default();

        if self.size_has_changed_since_last_present {
            write!(self.terminal, "{}", termion::clear::All).expect("clear");
            self.size_has_changed_since_last_present = false;
        }
        if self.bell_to_emit {
            write!(self.terminal, "\x07").expect("emit bell");
            self.bell_to_emit = false;
        }
        for (y, line) in self.values.storage().axis_iter(Axis(0)).enumerate() {
            write!(
                self.terminal,
                "{}",
                termion::cursor::Goto(1, (y + 1) as u16)
            )
            .expect("move cursor");
            let mut buffer = String::with_capacity(line.len());
            for c in line.iter() {
                if c.style != current_style {
                    current_style.set_terminal_attributes(&mut self.terminal);
                    write!(self.terminal, "{}", buffer).expect("write buffer");
                    buffer.clear();
                    current_style = c.style;
                }
                let grapheme_cluster = match c.grapheme_cluster.as_str() {
                    c @ "\t" | c @ "\n" | c @ "\r" | c @ "\0" => {
                        panic!("Invalid grapheme cluster written to terminal: {:?}", c)
                    }
                    x => x,
                };
                buffer.push_str(grapheme_cluster);
            }
            current_style.set_terminal_attributes(&mut self.terminal);
            write!(self.terminal, "{}", buffer).expect("write leftover buffer contents");
        }
        let _ = self.terminal.flush();
    }
}

impl<'a, T: Write + AsRawFd> Drop for Terminal<'a, T> {
    fn drop(&mut self) {
        let _ = self.leave_tui();
    }
}

/// Contains a FakeTerminal useful for tests
pub mod test {
    use super::super::{
        GraphemeCluster, Height, Style, StyleModifier, StyledGraphemeCluster, Width, Window,
        WindowBuffer,
    };

    /// A fake terminal that can be used in tests to create windows and compare the resulting
    /// contents to the expected contents of windows.
    #[derive(PartialEq)]
    pub struct FakeTerminal {
        values: WindowBuffer,
    }
    impl FakeTerminal {
        /// Create a window with the specified (width, height).
        pub fn with_size((w, h): (u32, u32)) -> Self {
            FakeTerminal {
                values: WindowBuffer::new(
                    Width::new(w as i32).unwrap(),
                    Height::new(h as i32).unwrap(),
                ),
            }
        }

        /// Create a fake terminal from a format string that looks roughly like this:
        ///
        /// "1 1 2 2 3 3 4 4"
        ///
        /// Spaces and newlines are ignored and the string is coerced into the specified size.
        ///
        /// The following characters have a special meaning:
        /// * - toggle bold style
        pub fn from_str(
            (w, h): (u32, u32),
            description: &str,
        ) -> Result<Self, ::ndarray::ShapeError> {
            let mut tiles = Vec::<StyledGraphemeCluster>::new();
            let mut style = Style::plain();
            for c in GraphemeCluster::all_from_str(description) {
                if c.as_str() == "*" {
                    style = StyleModifier::new()
                        .bold(crate::base::BoolModifyMode::Toggle)
                        .apply(style);
                    continue;
                }
                if c.as_str() == " " || c.as_str() == "\n" {
                    continue;
                }
                tiles.push(StyledGraphemeCluster::new(c, style));
            }
            Ok(FakeTerminal {
                values: WindowBuffer::from_storage(::ndarray::Array2::from_shape_vec(
                    (h as usize, w as usize),
                    tiles,
                )?),
            })
        }

        /// Test if the terminal contents look like the given format string.
        /// The rows are separated by a "|".
        ///
        /// # Examples:
        ///
        /// ```
        /// use unsegen::base::terminal::test::FakeTerminal;
        /// use unsegen::base::GraphemeCluster;
        ///
        /// let mut term = FakeTerminal::with_size((2,3));
        /// {
        ///     let mut win = term.create_root_window();
        ///     win.fill(GraphemeCluster::try_from('_').unwrap());
        /// }
        ///
        /// term.assert_looks_like("__|__|__");
        /// ```
        pub fn assert_looks_like(&self, string_description: &str) {
            assert_eq!(format!("{:?}", self), string_description);
        }

        /// Create a root window that covers the whole terminal grid.
        pub fn create_root_window(&mut self) -> Window {
            self.values.as_window()
        }
    }

    impl ::std::fmt::Debug for FakeTerminal {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
            let raw_values = self.values.storage();
            for r in 0..raw_values.dim().0 {
                for c in 0..raw_values.dim().1 {
                    let c = raw_values.get((r, c)).expect("debug: in bounds");
                    if c.style.format().bold {
                        write!(f, "*{}*", c.grapheme_cluster.as_str())?;
                    } else {
                        write!(f, "{}", c.grapheme_cluster.as_str())?;
                    }
                }
                if r != raw_values.dim().0 - 1 {
                    write!(f, "|")?;
                }
            }
            Ok(())
        }
    }
}
