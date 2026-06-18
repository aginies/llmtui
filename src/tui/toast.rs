use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use std::time::Instant;

use crate::tui::colors::WHITE;

pub const TOAST_MAX_WIDTH: u16 = 50;
pub const TOAST_DURATION_SECS: u64 = 5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ToastLevel {
    Error,
    #[allow(dead_code)]
    Warning,
    #[allow(dead_code)]
    Info,
}

#[derive(Debug, Clone)]
pub struct Toast {
    pub text: String,
    pub level: ToastLevel,
    pub created_at: Instant,
}

impl Toast {
    pub fn new(text: impl Into<String>, level: ToastLevel) -> Self {
        Self {
            text: text.into(),
            level,
            created_at: Instant::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed().as_secs() >= TOAST_DURATION_SECS
    }

    pub fn border_style(&self) -> Style {
        match self.level {
            ToastLevel::Error => Style::default().fg(crate::tui::colors::RED),
            ToastLevel::Warning => Style::default().fg(crate::tui::colors::YELLOW),
            ToastLevel::Info => Style::default().fg(crate::tui::colors::DIM_GRAY),
        }
    }
}

pub fn render_toast(f: &mut Frame, area: Rect, toast: &Toast) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(toast.border_style())
        .style(Style::default().bg(Color::Rgb(15, 15, 15)));
    let text = Line::from(toast.text.clone()).style(Style::default().fg(WHITE).add_modifier(Modifier::BOLD));
    f.render_widget(Paragraph::new(text).block(block), area);
}
