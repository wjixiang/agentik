use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    prelude::{StatefulWidget, Widget},
    widgets::{Block, Padding},
};

use crate::state::{AgentTabState, InputMode};
use crate::widgets::{
    chat_widget::{ChatWidget, ChatWidgetState},
    input_area::{InputWidget, InputWidgetState},
    status_bar::StatusBar,
};

/// Composite widget that renders the entire Agent tab: status bar, chat area with
/// scrollbar, and input area.
pub struct AgentTabWidget<'a> {
    pub state: &'a mut AgentTabState,
}

impl Widget for AgentTabWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::prelude::Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // StatusBar
                Constraint::Min(5),    // Chat
                Constraint::Length(3), // Input
            ])
            .split(area);

        let ts = &mut *self.state;

        // ── StatusBar ──
        let status_bar = StatusBar {
            status: &ts.status,
            input_tokens: ts.input_tokens,
            output_tokens: ts.output_tokens,
        };
        status_bar.render(layout[0], buf);

        // ── Chat area ──
        let chat_area = layout[1];
        let chat_block = Block::default().padding(Padding::new(2, 2, 2, 2));
        let chat_inner_area = chat_block.inner(chat_area);

        let viewport_height = chat_inner_area.height;
        ts.clamp_scroll(viewport_height);

        let mut chat_state = ChatWidgetState::new(ts.scroll_offset);
        let chat_widget = ChatWidget {
            messages: &ts.messages,
        };

        chat_widget.render(chat_inner_area, buf, &mut chat_state);
        ts.content_line_count = chat_state.total_lines;

        // ── Input area ──
        let running = ts.status != crate::state::AgentStatus::Idle;
        let title: &str = match (running, ts.input_mode) {
            (true, _) => " ■ input (running) ",
            (false, InputMode::Browse) => "▏browse (Enter=edit) ",
            (false, InputMode::Input) => " > input ",
        };

        let input_widget = InputWidget {
            disabled: running,
            title,
            placeholder: "Type a message...",
        };
        let mut input_state = InputWidgetState {
            input: &mut ts.input,
        };
        input_widget.render(layout[2], buf, &mut input_state);
    }
}
