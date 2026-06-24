use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph},
};
use std::time::Instant;
use std::collections::VecDeque;

use crate::tui::colors::WHITE;

pub const TOAST_MAX_WIDTH: u16 = 50;
pub const TOAST_MAX_ITEMS: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Error,
    #[allow(dead_code)]
    Warning,
    #[allow(dead_code)]
    Info,
}

impl ToastLevel {
    pub fn duration_secs(&self) -> u64 {
        match self {
            ToastLevel::Error => 15,
            ToastLevel::Warning => 5,
            ToastLevel::Info => 3,
        }
    }

    pub fn border_style(&self) -> Style {
        match self {
            ToastLevel::Error => Style::default().fg(crate::tui::colors::RED),
            ToastLevel::Warning => Style::default().fg(crate::tui::colors::YELLOW),
            ToastLevel::Info => Style::default().fg(crate::tui::colors::DIM_GRAY),
        }
    }
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
        self.created_at.elapsed().as_secs() >= self.level.duration_secs()
    }
}

pub fn render_toasts(f: &mut Frame, area: Rect, toasts: &VecDeque<Toast>) {
    if toasts.is_empty() {
        return;
    }

    let toast_height: u16 = 3;
    let total_height = (toasts.len() as u16 * toast_height).min(area.height);
    let start_y = area.bottom().saturating_sub(total_height);

    for (i, toast) in toasts.iter().enumerate() {
        let y = start_y + (i as u16 * toast_height);
        let toast_area = Rect {
            x: area.right().saturating_sub(TOAST_MAX_WIDTH).saturating_sub(2),
            y,
            width: TOAST_MAX_WIDTH,
            height: toast_height.min(area.height.saturating_sub(y)),
        };

        if toast_area.width == 0 || toast_area.height == 0 {
            continue;
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(toast.level.border_style())
            .style(Style::default().bg(Color::Rgb(15, 15, 15)));
        let text = Line::from(toast.text.clone())
            .style(Style::default().fg(WHITE).add_modifier(Modifier::BOLD));
        f.render_widget(Paragraph::new(text).block(block), toast_area);
    }
}
