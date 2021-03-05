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
    edit_prompt: String,
    scroll_prompt: String,
    search_prompt: String,
    pub line: LineEdit,
    history: Vec<String>,
    state: State,
    layout: HorizontalLayout,
}

enum State {
    Editing,
    Scrollback {
        active_line: String,
        pos: usize,
    }, //invariant: pos < history.len()
    Searching {
        search_pattern: String,
        pos: Option<usize>, // Found? -> some
    }, //invariant: pos < history.len()
}

fn search_prev(
    current: Option<usize>,
    history: &Vec<String>,
    search_pattern: &str,
) -> Option<usize> {
    if search_pattern.is_empty() {
        return None;
    }
    let current = current.unwrap_or(history.len());
    assert!(current <= history.len());
    history[..current]
        .iter()
        .enumerate()
        .rev()
        .find(|(_, line)| line.contains(search_pattern))
        .map(|(i, _)| i)
}
fn search_next(
    current: Option<usize>,
    history: &Vec<String>,
    search_pattern: &str,
) -> Option<usize> {
    if search_pattern.is_empty() {
        return None;
    }
    let start = current.map(|c| c + 1).unwrap_or(0);
    history[start..]
        .iter()
        .position(|line| line.contains(search_pattern))
        .map(|v| v + start)
}

impl PromptLine {
    /// Construct a PromptLine with the given symbol that will be displayed left of the `LineEdit`
    /// for user interaction.
    pub fn with_prompt(prompt: String) -> Self {
        PromptLine {
            edit_prompt: prompt.clone(),
            scroll_prompt: prompt.clone(),
            search_prompt: prompt.clone(),
            prompt: LineLabel::new(prompt),
            line: LineEdit::new(),
            history: Vec::new(),
            state: State::Editing,
            layout: HorizontalLayout::new(SeparatingStyle::None),
        }
    }

    /// Change the symbol left of the user editable section (for editing, search and scrolling).
    pub fn set_prompt(&mut self, prompt: String) {
        self.edit_prompt = prompt.clone();
        self.search_prompt = prompt.clone();
        self.scroll_prompt = prompt;
        self.update_display();
    }

    /// Change the symbol left of the user editable section (only for edit operations).
    pub fn set_edit_prompt(&mut self, prompt: String) {
        self.edit_prompt = prompt;
        self.update_display();
    }

    /// Change the symbol left of the user editable section (only while searching).
    pub fn set_search_prompt(&mut self, prompt: String) {
        self.search_prompt = prompt;
        self.update_display();
    }

    /// Change the symbol left of the user editable section (only while scrolling).
    pub fn set_scroll_prompt(&mut self, prompt: String) {
        self.scroll_prompt = prompt;
        self.update_display();
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
        self.state = State::Editing;
        self.update_display();
        let _ = self.line.clear();
        &self.history[self.history.len() - 1]
    }

    pub fn enter_search(&mut self) {
        let mut tmp = State::Editing;
        std::mem::swap(&mut tmp, &mut self.state);
        let (pos, search_pattern) = match tmp {
            State::Editing => (None, "".to_owned()),
            State::Searching {
                pos,
                search_pattern,
            } => (pos, search_pattern),
            State::Scrollback { pos, .. } => (Some(pos), "".to_owned()),
        };
        let pos = search_prev(pos, &self.history, &search_pattern);
        self.state = State::Searching {
            pos,
            search_pattern,
        };
        self.update_display();
    }

    /// Set the line content according to the current scrollback position
    fn update_display(&mut self) {
        match &mut self.state {
            State::Editing => {
                self.prompt.set(self.edit_prompt.clone());
            }
            State::Scrollback { pos, .. } => {
                self.prompt.set(self.scroll_prompt.clone());
                self.line.set(&self.history[*pos]);
            }
            State::Searching {
                pos,
                search_pattern,
                ..
            } => {
                self.prompt
                    .set(format!("{}\"{}\": ", self.search_prompt, search_pattern));
                if let Some(p) = pos {
                    self.line.set(&self.history[*p]);
                } else {
                    self.line.set("");
                }
            }
        }
    }

    /// An edit operation changes the state from "we are looking through history" to "we are
    /// editing a complete new line".
    fn note_edit_operation(&mut self, res: OperationResult) -> OperationResult {
        if res.is_ok() {
            self.state = State::Editing;
            self.update_display();
        }
        res
    }

    fn searching(&self) -> bool {
        if let State::Searching { .. } = self.state {
            true
        } else {
            false
        }
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
            State::Searching {
                search_pattern,
                pos,
            } => {
                let pos = search_next(pos, &self.history, &search_pattern);
                result = pos.map(|_| ()).ok_or(());

                State::Searching {
                    search_pattern,
                    pos,
                }
            }
        };
        self.update_display();
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
            State::Searching {
                search_pattern,
                pos,
            } => {
                let pos = search_prev(pos, &self.history, &search_pattern);
                result = pos.map(|_| ()).ok_or(());

                State::Searching {
                    search_pattern,
                    pos,
                }
            }
        };
        self.update_display();
        result
    }
    fn scroll_to_beginning(&mut self) -> OperationResult {
        let result;
        let mut tmp = State::Editing;
        std::mem::swap(&mut tmp, &mut self.state);
        self.state = match tmp {
            State::Editing | State::Scrollback { .. } => {
                if self.history.len() > 0 {
                    result = Ok(());
                    State::Scrollback {
                        active_line: self.line.get().to_owned(),
                        pos: 0,
                    }
                } else {
                    result = Err(());
                    State::Editing
                }
            }
            State::Searching { search_pattern, .. } => {
                let pos = search_next(None, &self.history, &search_pattern);
                result = pos.map(|_| ()).ok_or(());

                State::Searching {
                    search_pattern,
                    pos,
                }
            }
        };
        self.update_display();
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
            State::Searching { search_pattern, .. } => {
                let pos = search_prev(None, &self.history, &search_pattern);
                result = pos.map(|_| ()).ok_or(());

                State::Searching {
                    search_pattern,
                    pos,
                }
            }
        };
        self.update_display();
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
        if self.searching() {
            self.state = State::Editing;
            self.update_display();
            Ok(())
        } else {
            self.line.move_left()
        }
    }
    fn move_right(&mut self) -> OperationResult {
        if self.searching() {
            self.state = State::Editing;
            self.update_display();
            Ok(())
        } else {
            self.line.move_right()
        }
    }
}

impl Writable for PromptLine {
    fn write(&mut self, c: char) -> OperationResult {
        let res = match &mut self.state {
            State::Editing | State::Scrollback { .. } => {
                let res = self.line.write(c);
                self.note_edit_operation(res)
            }
            State::Searching {
                search_pattern,
                pos,
                ..
            } => match c {
                '\n' => Err(()),
                o => {
                    search_pattern.push(o);
                    *pos = search_prev(pos.map(|p| p + 1), &self.history, &search_pattern);
                    pos.map(|_| ()).ok_or(())
                }
            },
        };
        self.update_display();
        res
    }
}

impl Editable for PromptLine {
    fn delete_forwards(&mut self) -> OperationResult {
        let res = self.line.delete_forwards();
        self.note_edit_operation(res)
    }
    fn delete_backwards(&mut self) -> OperationResult {
        let res = match &mut self.state {
            State::Editing | State::Scrollback { .. } => {
                let res = self.line.delete_backwards();
                self.note_edit_operation(res)
            }
            State::Searching {
                search_pattern,
                pos,
            } => {
                if search_pattern.pop().is_some() {
                    *pos = search_prev(pos.map(|p| p + 1), &self.history, &search_pattern);
                } else {
                    self.state = State::Editing;
                }
                Ok(())
            }
        };
        self.update_display();
        res
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
        let res = match &mut self.state {
            State::Editing | State::Scrollback { .. } => {
                let res = self.line.clear();
                self.note_edit_operation(res)
            }
            State::Searching { .. } => {
                self.state = State::Editing;
                Ok(())
            }
        };
        self.update_display();
        res
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_search_prev() {
        let history = vec![
            "".to_string(),
            "abc".to_string(),
            "foo".to_string(),
            "".to_string(),
            "foo".to_string(),
        ];

        assert_eq!(search_prev(Some(0), &history, ""), None);
        assert_eq!(search_prev(Some(1), &history, "a"), None);
        assert_eq!(search_prev(Some(5), &history, "foo"), Some(4));
        assert_eq!(search_prev(Some(4), &history, "foo"), Some(2));
        assert_eq!(search_prev(Some(2), &history, "foo"), None);
        assert_eq!(search_prev(Some(5), &history, "foo2"), None);
        assert_eq!(search_prev(None, &history, ""), None);
    }

    #[test]
    fn test_search_next() {
        let history = vec![
            "".to_string(),
            "abc".to_string(),
            "foo".to_string(),
            "".to_string(),
            "foo".to_string(),
        ];

        assert_eq!(search_next(Some(4), &history, ""), None);
        assert_eq!(search_next(Some(0), &history, "a"), Some(1));
        assert_eq!(search_next(Some(0), &history, "foo"), Some(2));
        assert_eq!(search_next(Some(2), &history, "foo"), Some(4));
        assert_eq!(search_next(Some(4), &history, "foo"), None);
        assert_eq!(search_next(Some(0), &history, "foo2"), None);
        assert_eq!(search_next(None, &history, ""), None);
    }
}
