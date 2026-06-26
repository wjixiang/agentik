use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};
use tui_markdown::{Options, StyleSheet};

/// Dark-theme stylesheet for markdown rendering in the chat widget.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PhloemStyleSheet;

impl StyleSheet for PhloemStyleSheet {
    fn heading(&self, level: u8) -> Style {
        match level {
            1 => Style::default()
                .fg(Color::Rgb(220, 220, 255))
                .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            2 => Style::default()
                .fg(Color::Rgb(180, 180, 255))
                .add_modifier(Modifier::BOLD),
            3 => Style::default()
                .fg(Color::Rgb(160, 160, 240))
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Rgb(140, 140, 220))
                .add_modifier(Modifier::ITALIC),
        }
    }

    fn code(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(200, 200, 200))
            .bg(Color::Rgb(40, 40, 40))
    }

    fn link(&self) -> Style {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::UNDERLINED)
    }

    fn blockquote(&self) -> Style {
        Style::default()
            .fg(Color::Rgb(180, 180, 100))
            .add_modifier(Modifier::ITALIC)
    }

    fn heading_meta(&self) -> Style {
        Style::default().fg(Color::DarkGray)
    }

    fn metadata_block(&self) -> Style {
        Style::default().fg(Color::Rgb(180, 180, 160))
    }
}

pub(crate) static MD_OPTIONS: LazyLock<Options<PhloemStyleSheet>> =
    LazyLock::new(|| Options::new(PhloemStyleSheet));
