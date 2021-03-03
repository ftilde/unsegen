//! A widget implementing "readline"-like functionality.
use super::super::{Demand2D, HorizontalLayout, RenderingHints, SeparatingStyle, Widget};
use super::{LineEdit, LineLabel};
use base::Window;
use input::{Editable, Navigatable, OperationResult, Scrollable, Writable};
use std::ops::{Deref, DerefMut};

/// A widget implementing "readline"-like functionality.
///
/// Basically a more sophisticated version of `LineEdit` with history.
pub struct PromptLine {
    prompt: LineLabel,
    pub line: LineEdit,
    history: Vec<String>,
    state: State,
    layout: HorizontalLayout,
}

enum State {
    Editing,
    Scrollback { active_line: String, pos: usize }, //invariant: pos < history.len()
}

impl PromptLine {
    /// Construct a PromptLine with the given symbol that will be displayed left of the `LineEdit`
    /// for user interaction.
    pub fn with_prompt(prompt: String) -> Self {
        PromptLine {
            prompt: LineLabel::new(prompt),
            line: LineEdit::new(),
            history: Vec::new(),
            state: State::Editing,
            layout: HorizontalLayout::new(SeparatingStyle::None),
        }
    }

    /// Change the symbol left of the user editable section.
    pub fn set_prompt(&mut self, prompt: String) {
        self.prompt = LineLabel::new(prompt);
    }

    /// Get the `n`'th line from the history.
    pub fn previous_line(&self, n: usize) -> Option<&str> {
        self.history
            .get(self.history.len().checked_sub(n).unwrap_or(0))
            .map(String::as_str)
    }

    /// Get the current content of the `LineEdit`
    pub fn active_line(&self) -> &str {
        self.line.get()
    }

    /// Mark the current content as "accepted", e.g., if the user has entered and submitted a command.
    ///
    /// This adds the current line to the front of the history buffer.
    pub fn finish_line(&mut self) -> &str {
        if self.history.is_empty()
            || self.line.get() != self.history.last().expect("history is not empty").as_str()
        {
            self.history.push(self.line.get().to_owned());
        }
        let _ = self.line.clear();
        &self.history[self.history.len() - 1]
    }

    /// Set the line content according to the current scrollback position
    fn sync_line_to_history_scroll_position(&mut self) {
        if let State::Scrollback { pos, .. } = self.state {
            // history[pos] is always valid because of the invariant on history_scroll_pos
            self.line.set(&self.history[pos]);
        }
    }

    /// An edit operation changes the state from "we are looking through history" to "we are
    /// editing a complete new line".
    fn note_edit_operation(&mut self, res: OperationResult) -> OperationResult {
        if res.is_ok() {
            self.state = State::Editing;
        }
        res
    }
}

impl Widget for PromptLine {
    fn space_demand(&self) -> Demand2D {
        let widgets: Vec<&dyn Widget> = vec![&self.prompt, &self.line];
        self.layout.space_demand(widgets.as_slice())
    }
    fn draw(&self, window: Window, hints: RenderingHints) {
        let widgets: Vec<(&dyn Widget, RenderingHints)> =
            vec![(&self.prompt, hints), (&self.line, hints)];
        self.layout.draw(window, widgets.as_slice());
    }
}

impl Scrollable for PromptLine {
    fn scroll_forwards(&mut self) -> OperationResult {
        let result;
        let mut tmp = State::Editing;
        std::mem::swap(&mut tmp, &mut self.state);
        self.state = match tmp {
            State::Editing => {
                result = Err(());
                State::Editing
            }
            State::Scrollback {
                active_line,
                mut pos,
            } => {
                result = Ok(());
                if pos + 1 < self.history.len() {
                    pos += 1;
                    State::Scrollback { pos, active_line }
                } else {
                    self.line.set(&active_line);
                    State::Editing
                }
            }
        };
        self.sync_line_to_history_scroll_position();
        result
    }
    fn scroll_backwards(&mut self) -> OperationResult {
        let result;
        let mut tmp = State::Editing;
        std::mem::swap(&mut tmp, &mut self.state);
        self.state = match tmp {
            State::Editing => {
                if self.history.len() > 0 {
                    result = Ok(());
                    State::Scrollback {
                        active_line: self.line.get().to_owned(),
                        pos: self.history.len() - 1,
                    }
                } else {
                    result = Err(());
                    State::Editing
                }
            }
            State::Scrollback {
                active_line,
                mut pos,
            } => {
                if pos > 0 {
                    pos -= 1;
                    result = Ok(());
                } else {
                    result = Err(());
                }
                State::Scrollback { active_line, pos }
            }
        };
        self.sync_line_to_history_scroll_position();
        result
    }
    fn scroll_to_beginning(&mut self) -> OperationResult {
        let result;
        self.state = if self.history.len() > 0 {
            result = Ok(());
            State::Scrollback {
                active_line: self.line.get().to_owned(),
                pos: 0,
            }
        } else {
            result = Err(());
            State::Editing
        };
        self.sync_line_to_history_scroll_position();
        result
    }
    fn scroll_to_end(&mut self) -> OperationResult {
        let result;
        let mut tmp = State::Editing;
        std::mem::swap(&mut tmp, &mut self.state);
        self.state = match tmp {
            State::Editing => {
                result = Err(());
                State::Editing
            }
            State::Scrollback { active_line, .. } => {
                result = Ok(());
                self.line.set(&active_line);
                State::Editing
            }
        };
        self.sync_line_to_history_scroll_position();
        result
    }
}
impl Navigatable for PromptLine {
    fn move_up(&mut self) -> OperationResult {
        self.scroll_backwards()
    }
    fn move_down(&mut self) -> OperationResult {
        self.scroll_forwards()
    }
    fn move_left(&mut self) -> OperationResult {
        self.line.move_left()
    }
    fn move_right(&mut self) -> OperationResult {
        self.line.move_right()
    }
}

impl Writable for PromptLine {
    fn write(&mut self, c: char) -> OperationResult {
        let res = self.line.write(c);
        self.note_edit_operation(res)
    }
}

impl Editable for PromptLine {
    fn delete_forwards(&mut self) -> OperationResult {
        let res = self.line.delete_forwards();
        self.note_edit_operation(res)
    }
    fn delete_backwards(&mut self) -> OperationResult {
        let res = self.line.delete_backwards();
        self.note_edit_operation(res)
    }
    fn go_to_beginning_of_line(&mut self) -> OperationResult {
        let res = self.line.go_to_beginning_of_line();
        self.note_edit_operation(res)
    }
    fn go_to_end_of_line(&mut self) -> OperationResult {
        let res = self.line.go_to_end_of_line();
        self.note_edit_operation(res)
    }
    fn clear(&mut self) -> OperationResult {
        let res = self.line.clear();
        self.note_edit_operation(res)
    }
}

impl Deref for PromptLine {
    type Target = LineEdit;

    fn deref(&self) -> &Self::Target {
        &self.line
    }
}

impl DerefMut for PromptLine {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.line
    }
}
