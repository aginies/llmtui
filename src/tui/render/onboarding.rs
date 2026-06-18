use crate::tui::colors::*;
use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
     widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap},
};

const TOTAL_STEPS: usize = 8;

fn highlight_keys(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut current = String::new();
    let mut in_key = false;

    for ch in text.chars() {
        if ch == '`' {
            if !in_key && !current.is_empty() {
                spans.push(Span::raw(current.clone()));
                current.clear();
            }
            in_key = !in_key;
        } else if in_key {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default()
                    .fg(YELLOW)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            current.push(ch);
        }
    }

    if !current.is_empty() {
        spans.push(Span::raw(current));
    }

    spans
}

pub fn render_onboarding(f: &mut Frame, area: Rect, _app: &crate::tui::app::App, step: usize) {
    let w = (area.width as f64 * 0.75).clamp(60.0, 80.0) as u16;
    let h = (area.height as f64 * 0.75).clamp(22.0, 30.0) as u16;
    let popup_area = Rect {
        x: (area.width - w) / 2,
        y: (area.height - h) / 2,
        width: w,
        height: h,
    };

    let title_key = match step {
        0 => "onboarding.welcome.title",
        1 => "onboarding.step1.title",
        2 => "onboarding.step2.title",
        3 => "onboarding.step3.title",
        4 => "onboarding.step4.title",
        5 => "onboarding.step5.title",
        6 => "onboarding.step6.title",
        7 => "onboarding.step7.title",
        _ => "onboarding.welcome.title",
    };

    let desc_key = match step {
        0 => "onboarding.welcome.description",
        1 => "onboarding.step1.description",
        2 => "onboarding.step2.description",
        3 => "onboarding.step3.description",
        4 => "onboarding.step4.description",
        5 => "onboarding.step5.description",
        6 => "onboarding.step6.description",
        7 => "onboarding.step7.description",
        _ => "onboarding.welcome.description",
    };

    let keys_key = match step {
        0 => "onboarding.welcome.keys",
        1 => "onboarding.step1.keys",
        2 => "onboarding.step2.keys",
        3 => "onboarding.step3.keys",
        4 => "onboarding.step4.keys",
        5 => "onboarding.step5.keys",
        6 => "onboarding.step6.keys",
        7 => "onboarding.step7.keys",
        _ => "onboarding.welcome.keys",
    };

    let title = crate::t!(title_key);
    let description = crate::t!(desc_key);
    let keys_text = crate::t!(keys_key);

    let step_indicator = format!(
        "{} {}/{}",
        crate::t!("onboarding.step_indicator"),
        step + 1,
        TOTAL_STEPS
    );

     // Progress bar
    let mut lines: Vec<Line> = Vec::new();
    let bar_area = Rect {
        x: popup_area.x + 1,
        y: popup_area.y + 1,
        width: popup_area.width.saturating_sub(2),
        height: 1,
    };
    let ratio = (step + 1) as f64 / TOTAL_STEPS as f64;
    let progress_bar = Gauge::default()
        .ratio(ratio.min(1.0))
        .label(format!("Step {}/{}", step + 1, TOTAL_STEPS))
        .gauge_style(Style::default().fg(CYAN));
    f.render_widget(progress_bar, bar_area);
    lines.push(Line::from(""));
    lines.push(Line::from(""));

    // Title
    lines.push(Line::from(Span::styled(
        title,
        Style::default()
            .fg(YELLOW)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(description));
    lines.push(Line::from(""));

    // Key shortcuts section
    if !keys_text.is_empty() {
        lines.push(Line::from(Span::styled(
            "── KEY SHORTCUTS ──",
            Style::default()
                .fg(CYAN)
                .add_modifier(Modifier::BOLD),
        )));
        let key_spans = highlight_keys(keys_text);
        if !key_spans.is_empty() {
            lines.push(Line::from(key_spans));
        }
    }

    let block = Block::default()
        .title(format!(" {} — {} ", title, step_indicator))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(YELLOW))
        .border_type(BorderType::Rounded);

    f.render_widget(ratatui::widgets::Clear, popup_area);
    f.render_widget(
        Paragraph::new(lines)
            .block(block)
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: false }),
        popup_area,
    );

    // Footer
    let next_label = if step + 1 >= TOTAL_STEPS {
        crate::t!("onboarding.complete")
    } else {
        crate::t!("onboarding.next")
    };
    let footer_text = format!(
        " [{}] {}  [Esc/q] {}  [←/p] ",
        next_label,
        crate::t!("onboarding.skip"),
        crate::t!("onboarding.previous")
    );
    let footer = Paragraph::new(Line::from(Span::styled(
        footer_text,
        Style::default().fg(DIM_GRAY),
    )));
    f.render_widget(footer, popup_area);
}
