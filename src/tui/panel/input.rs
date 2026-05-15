use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use crate::tui::app::App;

pub fn render(f: &mut Frame, area: Rect, app: &mut App) {
    let title = " Chat Input ";
    let border_color = if app.active_panel == crate::tui::app::ActivePanel::ChatInput {
        Color::Yellow
    } else {
        Color::DarkGray
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    // Inner area for text (subtract borders)
    let inner_area = block.inner(area);
    let width = inner_area.width as usize;

    // Truncate text if it's too long to fit in the input box.
    // We want the end of the text (where the cursor is) to be visible.
    let display_text = if app.chat_input.chars().count() >= width {
        let chars: Vec<char> = app.chat_input.chars().collect();
        let skip = chars.len().saturating_sub(width).saturating_add(1);
        chars.iter().skip(skip).collect::<String>()
    } else {
        app.chat_input.clone()
    };

    let paragraph = Paragraph::new(display_text.clone()).block(block);
    f.render_widget(paragraph, area);

    // Set terminal cursor if focused
    if app.active_panel == crate::tui::app::ActivePanel::ChatInput {
        f.set_cursor_position(Position {
            x: inner_area.x + display_text.chars().count() as u16,
            y: inner_area.y,
        });
    }
}
