use crossterm::event::{KeyCode, KeyEvent};

use ratatui::{
    layout::Rect,
    prelude::{StatefulWidget, Widget},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
};

/// State for a single-line text input.
#[derive(Debug, Default)]
pub struct InputState {
    content: String,
    cursor: usize,
}

impl InputState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn value(&self) -> &str {
        &self.content
    }

    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor = 0;
    }

    /// Insert a character at the cursor position.
    pub fn insert(&mut self, ch: char) {
        self.content.insert(self.cursor, ch);
        self.cursor += ch.len_utf8();
    }

    /// Delete the character before the cursor.
    pub fn backspace(&mut self) {
        if self.cursor > 0 {
            let prev = self.content[..self.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
            self.cursor -= prev;
            self.content.remove(self.cursor);
        }
    }

    /// Move cursor left.
    pub fn cursor_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= self.content[..self.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
        }
    }

    /// Move cursor right.
    pub fn cursor_right(&mut self) {
        if self.cursor < self.content.len() {
            self.cursor += self.content[self.cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(1);
        }
    }

    /// Move cursor to start.
    pub fn cursor_home(&mut self) {
        self.cursor = 0;
    }

    /// Move cursor to end.
    pub fn cursor_end(&mut self) {
        self.cursor = self.content.len();
    }

    /// Handle a key event. Returns true if the key was consumed.
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(c) => {
                self.insert(c);
                true
            }
            KeyCode::Backspace => {
                self.backspace();
                true
            }
            KeyCode::Left => {
                self.cursor_left();
                true
            }
            KeyCode::Right => {
                self.cursor_right();
                true
            }
            KeyCode::Home => {
                self.cursor_home();
                true
            }
            KeyCode::End => {
                self.cursor_end();
                true
            }
            KeyCode::Delete => {
                if self.cursor < self.content.len() {
                    self.content.remove(self.cursor);
                }
                true
            }
            _ => false,
        }
    }
}

/// State for [`InputWidget`].
pub struct InputWidgetState<'a> {
    pub input: &'a mut InputState,
}

/// Composite widget: bordered input area + text content with visual cursor.
pub struct InputWidget<'a> {
    pub disabled: bool,
    pub title: &'a str,
    pub placeholder: &'a str,
}

impl<'a> StatefulWidget for InputWidget<'a> {
    type State = InputWidgetState<'a>;

    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer, state: &mut Self::State) {
        // Render border
        let style = if self.disabled {
            Style::default().fg(Color::DarkGray)
        } else {
            Style::default()
        };

        let block = Block::bordered()
            .title(format!(" {} ", self.title))
            .border_style(style);
        let inner = block.inner(area);
        block.render(area, buf);

        if self.disabled {
            Paragraph::new(Line::from(Span::styled(
                if self.placeholder.is_empty() {
                    "■ running...".to_string()
                } else {
                    self.placeholder.to_string()
                },
                Style::default().fg(Color::DarkGray),
            )))
            .render(inner, buf);
        } else {
            render_input_text(state.input, self.placeholder, inner, buf);
        }
    }
}

/// Render the text content inside the input area with a visual block cursor.
pub fn render_input_text(
    state: &InputState,
    placeholder: &str,
    area: Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    if state.is_empty() {
        Paragraph::new(Span::styled(
            placeholder,
            Style::default().fg(Color::DarkGray),
        ))
        .render(area, buf);
    } else {
        let text = state.value();
        let cursor_pos = state.cursor;

        let before = &text[..cursor_pos];
        let after = &text[cursor_pos..];

        let line = Line::from(vec![
            Span::raw(before.to_string()),
            Span::styled(
                if after.is_empty() {
                    " ".to_string()
                } else {
                    after.chars().next().unwrap().to_string()
                },
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::REVERSED),
            ),
            Span::raw(
                after[after.chars().next().map_or(0, |c| c.len_utf8())..].to_string(),
            ),
        ]);

        Paragraph::new(line).render(area, buf);
    }
}
