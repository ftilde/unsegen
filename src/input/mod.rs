//! Raw terminal input events, common abstractions for application (component) behavior and means
//! to easily distribute events.
//!
//! # Example:
//! ```
//! use unsegen::input::*;
//! use std::io::Read;
//!
//! struct Scroller {
//!     line_number: u32,
//!     end: u32,
//! }
//!
//! impl Scrollable for Scroller {
//!     fn scroll_backwards(&mut self) -> OperationResult {
//!         if self.line_number > 0 {
//!             self.line_number -= 1;
//!             Ok(())
//!         } else {
//!             Err(())
//!         }
//!     }
//!     fn scroll_forwards(&mut self) -> OperationResult {
//!         if self.line_number < self.end - 1 {
//!             self.line_number += 1;
//!             Ok(())
//!         } else {
//!             Err(())
//!         }
//!     }
//! }
//!
//! fn main() {
//!     let mut scroller = Scroller {
//!         line_number: 0,
//!         end: 5,
//!     };
//!
//!     // Read all inputs from something that implements Read
//!     for input in Input::read_all(&[b'1', b'2', b'3', b'4'][..]) {
//!         let input = input.unwrap();
//!
//!         // Define a chain of handlers for different kinds of events!
//!         // If a handler (Behavior) cannot process an input, it is passed down the chain.
//!         let leftover = input
//!             .chain((Key::Char('1'), || println!("Got a 1!")))
//!             .chain(
//!                 ScrollBehavior::new(&mut scroller)
//!                     .backwards_on(Key::Char('2'))
//!                     .forwards_on(Key::Char('3')),
//!             )
//!             .chain(|i: Input| {
//!                 if let Event::Key(Key::Char(c)) = i.event {
//!                     println!("Got some char: {}", c);
//!                     None // matches! event will be consumed
//!                 } else {
//!                     Some(i)
//!                 }
//!             })
//!             .finish();
//!         if let Some(e) = leftover {
//!             println!("Could not handle input {:?}", e);
//!         }
//!     }
//!
//!     // We could not scroll back first, but one line forwards later!
//!     assert_eq!(scroller.line_number, 1);
//! }
//! ```

use std::collections::HashSet;
pub use termion::event::{Event, Key, MouseButton, MouseEvent};
use termion::input::{EventsAndRaw, TermReadEventsAndRaw};

use std::io;

/// A structure corresponding to a single input event, e.g., a single keystroke or mouse event.
///
/// In addition to the semantic Event enum itself, the raw bytes that created this event are
/// available, as well. This is useful if the user wants to pass the input on to some other
/// terminal-like abstraction under certain circumstances (e.g., when writing a terminal
/// multiplexer).
#[derive(Eq, PartialEq, Clone, Debug)]
pub struct Input {
    pub event: Event,
    pub raw: Vec<u8>,
}

impl Input {
    /// Create an iterator that reads from the provided argument and converts the read bytes into
    /// a stream of `Input`s.
    ///
    /// Please note that the iterator blocks when no bytes are available from the `Read` source.
    pub fn read_all<R: io::Read>(read: R) -> InputIter<R> {
        InputIter {
            inner: read.events_and_raw(),
        }
    }

    /// Begin matching and processing of the event. See `InputChain`.
    pub fn chain<B: Behavior>(self, behavior: B) -> InputChain {
        let chain_begin = InputChain { input: Some(self) };
        chain_begin.chain(behavior)
    }

    /// Convert `Input` to `InputChain` without processing an event. See `InputChain`.
    pub fn into_chain(self) -> InputChain {
        InputChain { input: Some(self) }
    }

    /// Check whether this event is equal to the provided event-like argument.
    pub fn matches<T: ToEvent>(&self, e: T) -> bool {
        self.event == e.to_event()
    }
}

/// An iterator of `Input` events.
pub struct InputIter<R: io::Read> {
    inner: EventsAndRaw<R>,
}

impl<R: io::Read> Iterator for InputIter<R> {
    type Item = Result<Input, io::Error>;

    fn next(&mut self) -> Option<Result<Input, io::Error>> {
        self.inner.next().map(|tuple| {
            tuple.map(|(event, raw)| Input {
                event: event,
                raw: raw,
            })
        })
    }
}

/// An intermediate element in a chain of `Behavior`s that are matched against the event and
/// executed if applicable.
///
/// # Examples:
/// ```
/// use unsegen::input::*;
///
/// let mut triggered_first = false;
/// let mut triggered_second = false;
/// let mut triggered_third = false;
///
/// let input = Input {
///     event: Event::Key(Key::Char('g')),
///     raw: Vec::new(), //Incorrect, but does not matter for this example.
/// };
///
/// let res = input
///     .chain((Key::Char('f'), || triggered_first = true)) // does not match, passes event on
///     .chain(|i: Input| if let Event::Key(Key::Char(_)) = i.event {
///         triggered_second = true;
///         None // matches! event will be consumed
///     } else {
///         Some(i)
///     })
///     .chain((Key::Char('g'), || triggered_first = true)) // matches, but is not reached!
///     .finish();
///
/// assert!(!triggered_first);
/// assert!(triggered_second);
/// assert!(!triggered_third);
/// assert!(res.is_none());
/// ```
pub struct InputChain {
    input: Option<Input>,
}

impl InputChain {
    /// Add another behavior to the line of input processors that will try to consume the event one
    /// after another.
    pub fn chain<B: Behavior>(self, behavior: B) -> InputChain {
        if let Some(event) = self.input {
            InputChain {
                input: behavior.input(event),
            }
        } else {
            InputChain { input: None }
        }
    }

    /// Unpack the final chain value. If the `Input` was consumed by some `Behavior`, the result
    /// will be None, otherwise the original `Input` will be returned.
    pub fn finish(self) -> Option<Input> {
        self.input
    }
}

/// Used conveniently supply `Event`-like arguments to a number of functions in the input module.
/// For example, you can supply `Key::Up` instead of `Event::Key(Key::Up)`.
///
/// Basically an `Into<Event>`, but we cannot use that as Event is a reexport of termion.
pub trait ToEvent {
    fn to_event(self) -> Event;
}

impl ToEvent for Key {
    fn to_event(self) -> Event {
        Event::Key(self)
    }
}

impl ToEvent for MouseEvent {
    fn to_event(self) -> Event {
        Event::Mouse(self)
    }
}

impl ToEvent for Event {
    fn to_event(self) -> Event {
        self
    }
}

/// Very thin wrapper around HashSet<Event>, mostly to conveniently insert `ToEvent`s.
struct EventSet {
    events: HashSet<Event>,
}
impl EventSet {
    fn new() -> Self {
        EventSet {
            events: HashSet::new(),
        }
    }
    fn insert<E: ToEvent>(&mut self, event: E) {
        self.events.insert(event.to_event());
    }
    fn contains(&self, event: &Event) -> bool {
        self.events.contains(event)
    }
}

/// Something that reacts to input and possibly consumes it.
///
/// An inplementor is free to check the `Input` for arbitrary criteria and return the input if not
/// consumed. Note that the implementor should not _change_ the input event in that case.
///
/// If the implementor somehow reacts to the input, it is generally a good idea to "consume" the
/// value by returning None. This makes sure that subsequent `Behavior`s will not act.
///
/// Another thing of note is that a Behavior is generally constructed on the fly and consumed in
/// the `input` function!
/// For specialised behavior that does not fit into the often used abstractions defined in this
/// module (`Scrollable`, `Writable`, `Navigatable`, ...) the easiest way to construct a behavior
/// is either using a `FnOnce(Input) -> Option<Input>` where the implementor has to decide whether
/// the input matches the desired criteria or using a pair `(ToEvent, FnOnce())` where the function
/// is only iff the `Input` to be processed matches the provided `Event`-like thing.
pub trait Behavior {
    fn input(self, input: Input) -> Option<Input>;
}

impl<F: FnOnce(Input) -> Option<Input>> Behavior for F {
    fn input(self, input: Input) -> Option<Input> {
        self(input)
    }
}

impl<E: ToEvent, F: FnOnce()> Behavior for (E, F) {
    fn input(self, input: Input) -> Option<Input> {
        let (event, function) = self;
        if input.matches(event) {
            function();
            None
        } else {
            Some(input)
        }
    }
}

impl<E: ToEvent + Clone, F: FnOnce()> Behavior for (&[E], F) {
    fn input(self, input: Input) -> Option<Input> {
        let (it, function) = self;
        for event in it {
            if input.matches(event.clone()) {
                function();
                return None;
            }
        }
        Some(input)
    }
}

/// A common return type for Operations such as functions of `Scrollable`, `Writable`,
/// `Navigatable`, etc.
///
/// Ok(()) means: The input was processed successfully and should be consumed.
/// Err(()) means: The input could not be processed and should be passed on to and processed by
/// some other `Behavior`.
pub type OperationResult = Result<(), ()>;
fn pass_on_if_err(res: OperationResult, input: Input) -> Option<Input> {
    if res.is_err() {
        Some(input)
    } else {
        None
    }
}

// ScrollableBehavior -----------------------------------------------

/// Collection of triggers for functions of something `Scrollable` implementing `Behavior`.
pub struct ScrollBehavior<'a, S: Scrollable + 'a> {
    scrollable: &'a mut S,
    to_beginning_on: EventSet,
    to_end_on: EventSet,
    backwards_on: EventSet,
    forwards_on: EventSet,
}

impl<'a, S: Scrollable> ScrollBehavior<'a, S> {
    /// Create the behavior to act on the provided ´Scrollable`. Add triggers using other functions!
    pub fn new(scrollable: &'a mut S) -> Self {
        ScrollBehavior {
            scrollable: scrollable,
            backwards_on: EventSet::new(),
            forwards_on: EventSet::new(),
            to_beginning_on: EventSet::new(),
            to_end_on: EventSet::new(),
        }
    }
    /// Make the behavior trigger the `scroll_to_beginning` function on the provided event.
    pub fn to_beginning_on<E: ToEvent>(mut self, event: E) -> Self {
        self.to_beginning_on.insert(event);
        self
    }
    /// Make the behavior trigger the `scroll_to_end` function on the provided event.
    pub fn to_end_on<E: ToEvent>(mut self, event: E) -> Self {
        self.to_end_on.insert(event);
        self
    }
    /// Make the behavior trigger the `scroll_backwards` function on the provided event.
    pub fn backwards_on<E: ToEvent>(mut self, event: E) -> Self {
        self.backwards_on.insert(event);
        self
    }
    /// Make the behavior trigger the `scroll_forwards` function on the provided event.
    pub fn forwards_on<E: ToEvent>(mut self, event: E) -> Self {
        self.forwards_on.insert(event);
        self
    }
}

impl<'a, S: Scrollable> Behavior for ScrollBehavior<'a, S> {
    fn input(self, input: Input) -> Option<Input> {
        if self.forwards_on.contains(&input.event) {
            pass_on_if_err(self.scrollable.scroll_forwards(), input)
        } else if self.backwards_on.contains(&input.event) {
            pass_on_if_err(self.scrollable.scroll_backwards(), input)
        } else if self.to_beginning_on.contains(&input.event) {
            pass_on_if_err(self.scrollable.scroll_to_beginning(), input)
        } else if self.to_end_on.contains(&input.event) {
            pass_on_if_err(self.scrollable.scroll_to_end(), input)
        } else {
            Some(input)
        }
    }
}

/// Something that can be scrolled. Use in conjunction with `ScrollBehavior` to manipulate when
/// input arrives.
///
/// Note that `scroll_to_beginning` and `scroll_to_end` should be implemented manually if a fast
/// pass is available and performance is important. By default these functions call
/// `scroll_backwards` and `scroll_forwards` respectively until they fail.
pub trait Scrollable {
    fn scroll_backwards(&mut self) -> OperationResult;
    fn scroll_forwards(&mut self) -> OperationResult;
    fn scroll_to_beginning(&mut self) -> OperationResult {
        if self.scroll_backwards().is_err() {
            return Err(());
        } else {
            while self.scroll_backwards().is_ok() {}
            Ok(())
        }
    }
    fn scroll_to_end(&mut self) -> OperationResult {
        if self.scroll_forwards().is_err() {
            return Err(());
        } else {
            while self.scroll_forwards().is_ok() {}
            Ok(())
        }
    }
}

// WriteBehavior ------------------------------------------

/// Collection of triggers for functions of something `Writable` implementing `Behavior`.
pub struct WriteBehavior<'a, W: Writable + 'a> {
    writable: &'a mut W,
}
impl<'a, W: Writable + 'a> WriteBehavior<'a, W> {
    pub fn new(writable: &'a mut W) -> Self {
        WriteBehavior { writable: writable }
    }
}

impl<'a, W: Writable + 'a> Behavior for WriteBehavior<'a, W> {
    fn input(self, input: Input) -> Option<Input> {
        if let Event::Key(Key::Char(c)) = input.event {
            pass_on_if_err(self.writable.write(c), input)
        } else {
            Some(input)
        }
    }
}

/// Something that can be written to in the sense of a text box, editor or text input.
///
/// All inputs that correspond to keystrokes with a corresponding `char` representation will be
/// converted and passed to the `Writable`.
pub trait Writable {
    fn write(&mut self, c: char) -> OperationResult;
}

// NavigateBehavior ------------------------------------------------

/// Collection of triggers for functions of something `Navigatable` implementing `Behavior`.
pub struct NavigateBehavior<'a, N: Navigatable + 'a> {
    navigatable: &'a mut N,
    up_on: EventSet,
    down_on: EventSet,
    left_on: EventSet,
    right_on: EventSet,
}

impl<'a, N: Navigatable + 'a> NavigateBehavior<'a, N> {
    /// Create the behavior to act on the provided `Navigatable`. Add triggers using other functions!
    pub fn new(navigatable: &'a mut N) -> Self {
        NavigateBehavior {
            navigatable: navigatable,
            up_on: EventSet::new(),
            down_on: EventSet::new(),
            left_on: EventSet::new(),
            right_on: EventSet::new(),
        }
    }

    /// Make the behavior trigger the `move_up` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Up`.
    pub fn up_on<E: ToEvent>(mut self, event: E) -> Self {
        self.up_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_down` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Down`.
    pub fn down_on<E: ToEvent>(mut self, event: E) -> Self {
        self.down_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_left` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Left`.
    pub fn left_on<E: ToEvent>(mut self, event: E) -> Self {
        self.left_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_right` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Right`.
    pub fn right_on<E: ToEvent>(mut self, event: E) -> Self {
        self.right_on.insert(event);
        self
    }
}

impl<'a, N: Navigatable + 'a> Behavior for NavigateBehavior<'a, N> {
    fn input(self, input: Input) -> Option<Input> {
        if self.up_on.contains(&input.event) {
            pass_on_if_err(self.navigatable.move_up(), input)
        } else if self.down_on.contains(&input.event) {
            pass_on_if_err(self.navigatable.move_down(), input)
        } else if self.left_on.contains(&input.event) {
            pass_on_if_err(self.navigatable.move_left(), input)
        } else if self.right_on.contains(&input.event) {
            pass_on_if_err(self.navigatable.move_right(), input)
        } else {
            Some(input)
        }
    }
}

/// Something that can be navigated like a cursor in a text editor or character in a simple 2D
/// game.
pub trait Navigatable {
    fn move_up(&mut self) -> OperationResult;
    fn move_down(&mut self) -> OperationResult;
    fn move_left(&mut self) -> OperationResult;
    fn move_right(&mut self) -> OperationResult;
}

// EditBehavior ---------------------------------------------------------

/// Collection of triggers for functions of something `Editable` implementing `Behavior`.
pub struct EditBehavior<'a, E: Editable + 'a> {
    editable: &'a mut E,
    up_on: EventSet,
    down_on: EventSet,
    left_on: EventSet,
    right_on: EventSet,
    delete_forwards_on: EventSet,
    delete_backwards_on: EventSet,
    clear_on: EventSet,
    go_to_beginning_of_line_on: EventSet,
    go_to_end_of_line_on: EventSet,
}

impl<'a, E: Editable> EditBehavior<'a, E> {
    /// Create the behavior to act on the provided `Editable`. Add triggers using other functions!
    pub fn new(editable: &'a mut E) -> Self {
        EditBehavior {
            editable: editable,
            up_on: EventSet::new(),
            down_on: EventSet::new(),
            left_on: EventSet::new(),
            right_on: EventSet::new(),
            delete_forwards_on: EventSet::new(),
            delete_backwards_on: EventSet::new(),
            clear_on: EventSet::new(),
            go_to_beginning_of_line_on: EventSet::new(),
            go_to_end_of_line_on: EventSet::new(),
        }
    }

    /// Make the behavior trigger the `move_up` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Up`.
    pub fn up_on<T: ToEvent>(mut self, event: T) -> Self {
        self.up_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_down` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Down`.
    pub fn down_on<T: ToEvent>(mut self, event: T) -> Self {
        self.down_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_left` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Left`.
    pub fn left_on<T: ToEvent>(mut self, event: T) -> Self {
        self.left_on.insert(event);
        self
    }
    /// Make the behavior trigger the `move_right` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Right`.
    pub fn right_on<T: ToEvent>(mut self, event: T) -> Self {
        self.right_on.insert(event);
        self
    }
    /// Make the behavior trigger the `delete_forwards` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Delete`.
    pub fn delete_forwards_on<T: ToEvent>(mut self, event: T) -> Self {
        self.delete_forwards_on.insert(event);
        self
    }
    /// Make the behavior trigger the `delete_backwards` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Backspace`.
    pub fn delete_backwards_on<T: ToEvent>(mut self, event: T) -> Self {
        self.delete_backwards_on.insert(event);
        self
    }
    /// Make the behavior trigger the `clear` function on the provided event.
    pub fn clear_on<T: ToEvent>(mut self, event: T) -> Self {
        self.clear_on.insert(event);
        self
    }
    /// Make the behavior trigger the `go_to_beginning_of_line` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::Home`.
    pub fn go_to_beginning_of_line_on<T: ToEvent>(mut self, event: T) -> Self {
        self.go_to_beginning_of_line_on.insert(event);
        self
    }
    /// Make the behavior trigger the `go_to_end_of_line_on` function on the provided event.
    ///
    /// A typical candidate for `event` would be `Key::End`.
    pub fn go_to_end_of_line_on<T: ToEvent>(mut self, event: T) -> Self {
        self.go_to_end_of_line_on.insert(event);
        self
    }
}

impl<'a, E: Editable> Behavior for EditBehavior<'a, E> {
    fn input(self, input: Input) -> Option<Input> {
        if self.up_on.contains(&input.event) {
            pass_on_if_err(self.editable.move_up(), input)
        } else if self.down_on.contains(&input.event) {
            pass_on_if_err(self.editable.move_down(), input)
        } else if self.left_on.contains(&input.event) {
            pass_on_if_err(self.editable.move_left(), input)
        } else if self.right_on.contains(&input.event) {
            pass_on_if_err(self.editable.move_right(), input)
        } else if self.delete_forwards_on.contains(&input.event) {
            pass_on_if_err(self.editable.delete_forwards(), input)
        } else if self.delete_backwards_on.contains(&input.event) {
            pass_on_if_err(self.editable.delete_backwards(), input)
        } else if self.clear_on.contains(&input.event) {
            pass_on_if_err(self.editable.clear(), input)
        } else if self.go_to_beginning_of_line_on.contains(&input.event) {
            pass_on_if_err(self.editable.go_to_beginning_of_line(), input)
        } else if self.go_to_end_of_line_on.contains(&input.event) {
            pass_on_if_err(self.editable.go_to_end_of_line(), input)
        } else if let Event::Key(Key::Char(c)) = input.event {
            pass_on_if_err(self.editable.write(c), input)
        } else {
            Some(input)
        }
    }
}

/// Something that acts like a text editor.
pub trait Editable: Navigatable + Writable {
    /// In the sense of pressing the "Delete" key.
    fn delete_forwards(&mut self) -> OperationResult;
    /// In the sense of pressing the "Backspace" key.
    fn delete_backwards(&mut self) -> OperationResult;
    /// In the sense of pressing the "Home" key.
    fn go_to_beginning_of_line(&mut self) -> OperationResult;
    /// In the sense of pressing the "End" key.
    fn go_to_end_of_line(&mut self) -> OperationResult;
    fn clear(&mut self) -> OperationResult;
}
