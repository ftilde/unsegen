//! Use `unsegen`'s input module to raise signals on the usual key combinations (e.g., SIGINT on CTRL-C)
//!
//! # Example:
//! ```should_panic
//! extern crate unsegen;
//! extern crate unsegen_signals;
//!
//! use unsegen::input::*;
//! use unsegen_signals::*;
//!
//! for input in Input::read_all(&[b'a', b'b', b'c', 0o32/*Ctrl-Z*/, 0x3 /*Ctrl-C*/][..]) {
//!     let input = input.unwrap();
//!
//!     input
//!         // Will send SIGINT and panic the test!
//!         .chain(SignalBehavior::new().on_default::<SIGINT>())
//!         // But Ctrl-Z will pass through here, because have no handler for SIGTSTP
//!         .chain(|i: Input| {
//!             match i.event {
//!                 Event::Key(Key::Char(c)) => println!("Char: {}", c),
//!                 Event::Key(Key::Ctrl(c)) => println!("Ctrl: {}", c),
//!                 _ => return Some(i),
//!             }
//!             None
//!         });
//! }
//! ```
extern crate libc;
extern crate unsegen;

use unsegen::input::{Behavior, Event, Input, Key, ToEvent};

use libc::{getpid, kill};

use std::collections::HashMap;

type CSig = libc::c_int;

/// Every signal is described by a type that ties together its `libc` representation and a default
/// `Event` (i.e., most likely, a key combination) that fires it.
pub trait Signal {
    fn to_sig() -> CSig;
    fn default_event() -> Event;
}

/// Type corresponding to SIGINT, by default triggered by [Ctrl-Z](https://en.wikipedia.org/wiki/Ctrl-C).
pub struct SIGINT;
/// Type corresponding to SIGTSTP, by default triggered by [Ctrl-Z](https://en.wikipedia.org/wiki/Ctrl-Z).
pub struct SIGTSTP;
/// Type corresponding to SIGQUIT, by default triggered by [Ctrl-\](https://en.wikipedia.org/wiki/Ctrl-%5C).
pub struct SIGQUIT;

impl Signal for SIGINT {
    fn to_sig() -> CSig {
        libc::SIGINT
    }
    fn default_event() -> Event {
        Event::Key(Key::Ctrl('c'))
    }
}

impl Signal for SIGTSTP {
    fn to_sig() -> CSig {
        libc::SIGTSTP
    }
    fn default_event() -> Event {
        Event::Key(Key::Ctrl('z'))
    }
}

impl Signal for SIGQUIT {
    fn to_sig() -> CSig {
        libc::SIGQUIT
    }
    fn default_event() -> Event {
        Event::Key(Key::Ctrl('\\'))
    }
}

/// Raises signals which will be passed to the underlying terminal.
pub struct SignalBehavior {
    mapping: HashMap<Event, CSig>,
}

impl SignalBehavior {
    /// Create the Behavior without any triggers.
    ///
    /// Add triggers using `on` or `on_default`.
    pub fn new() -> Self {
        SignalBehavior {
            mapping: HashMap::new(),
        }
    }

    /// Raise a signal on a specific event.
    pub fn on<S: Signal, E: ToEvent>(mut self, e: E) -> Self {
        self.mapping.insert(e.to_event(), S::to_sig());
        self
    }

    /// Raise a signal on the default event
    pub fn on_default<S: Signal>(self) -> Self {
        self.on::<S, Event>(S::default_event())
    }
}

impl<'a> Behavior for SignalBehavior {
    fn input(self, i: Input) -> Option<Input> {
        if let Some(sig) = self.mapping.get(&i.event) {
            unsafe {
                kill(getpid(), *sig);
            }
            None
        } else {
            Some(i)
        }
    }
}
