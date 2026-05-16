use pulldown_cmark::{Options, Event, Tag, TagEnd};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState},
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use crate::tui::app::App;

/// Markdown renderer for TUI display.
pub struct MdRenderer {
    lines: Vec<Line<'static>>,
    /// Pending text segments: (text, style) pairs for the current line.
    pending: Vec<(String, Style)>,
    current_style: Style,
    in_code_block: bool,
    code_block_style: Style,
    indent: u16,
    list_marker: Option<String>,
    in_list: bool,
    blockquote_depth: u16,
    in_table: bool,
    table_is_header: bool,
    pending_table_header: Vec<(String, Style)>,
    pending_table_line: Vec<(String, Style)>,
}

impl MdRenderer {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            pending: Vec::new(),
            current_style: Style::default(),
            in_code_block: false,
            code_block_style: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::DIM),
            indent: 0,
            list_marker: None,
            in_list: false,
            blockquote_depth: 0,
            in_table: false,
            table_is_header: false,
            pending_table_header: Vec::new(),
            pending_table_line: Vec::new(),
        }
    }

    pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
        let mut renderer = Self::new();
        let options = Options::all();
        let parser = pulldown_cmark::Parser::new_ext(text, options);

        for event in parser {
            renderer.handle_event(event);
        }

        renderer.finish()
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        if self.in_code_block {
            self.flush_code_block();
        }
        if self.in_list {
            self.in_list = false;
            self.list_marker = None;
        }
        if self.in_table {
            self.flush_table_row();
        }

        // Trim trailing empty lines
        while self.lines.last().is_some_and(|l| l.spans.is_empty()) {
            self.lines.pop();
        }

        self.lines
    }

    fn handle_event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.handle_start(tag),
            Event::End(tag) => self.handle_end(tag),
            Event::Text(text) => self.handle_text(&text),
            Event::Code(code) => self.handle_code(&code),
            Event::SoftBreak => self.handle_soft_break(),
            Event::HardBreak => self.handle_hard_break(),
            Event::TaskListMarker(checked) => self.handle_task_list_marker(checked),
            Event::Rule => self.handle_rule(),
            Event::FootnoteReference(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::Html(_)
            | Event::InlineHtml(_) => {
                // Skip advanced markdown features
            }
        }
    }

    fn handle_start(&mut self, tag: Tag) {
        match tag {
            Tag::CodeBlock(_) => {
                if !self.in_code_block {
                    self.flush_line();
                    self.in_code_block = true;
                }
            }
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                self.current_style = match level {
                    pulldown_cmark::HeadingLevel::H1 => Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H2 => Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H3 => Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default().add_modifier(Modifier::BOLD),
                };
            }
            Tag::List(start) => {
                if let Some(n) = start {
                    self.list_marker = Some(format!("{n}."));
                } else {
                    self.list_marker = Some("•".to_string());
                }
                self.in_list = true;
            }
            Tag::Item => {
                // Item starts within a list
            }
            Tag::BlockQuote(_) => {
                self.blockquote_depth += 1;
            }
            Tag::Table(_) => {
                self.in_table = true;
                self.table_is_header = true;
                self.pending_table_header.clear();
            }
            Tag::TableCell => {}
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::CodeBlock => {
                if self.in_code_block {
                    self.flush_code_block();
                    self.in_code_block = false;
                    self.current_style = Style::default();
                }
            }
            TagEnd::Paragraph => {
                self.flush_line();
                self.lines.push(Line::from(""));
                self.current_style = Style::default();
            }
            TagEnd::List(_) => {
                self.in_list = false;
                self.list_marker = None;
            }
            TagEnd::Item => {}
            TagEnd::BlockQuote(_) => {
                if self.blockquote_depth > 0 {
                    self.blockquote_depth -= 1;
                }
            }
            TagEnd::Table => {
                if self.in_table {
                    self.flush_table_row();
                    self.in_table = false;
                }
            }
            TagEnd::TableCell => {}
            TagEnd::Heading(_) => {
                self.flush_line();
                self.current_style = Style::default();
            }
            _ => {}
        }
    }

    fn handle_text(&mut self, text: &str) {
        if self.in_table {
            self.pending_table_line
                .push((text.to_string(), self.current_style));
        } else {
            self.pending.push((text.to_string(), self.current_style));
        }
    }

    fn handle_code(&mut self, code: &str) {
        if self.in_table {
            self.pending_table_line
                .push((code.to_string(), Style::default().fg(Color::Green)));
        } else if self.in_code_block {
            self.pending.push((code.to_string(), self.code_block_style));
        } else {
            self.pending.push((
                format!("`{code}`"),
                self.current_style.fg(Color::Green),
            ));
        }
    }

    fn handle_soft_break(&mut self) {
        if self.in_code_block || self.in_table {
            self.pending.push(("\n".to_string(), self.current_style));
        } else {
            self.pending.push((" ".to_string(), self.current_style));
        }
    }

    fn handle_hard_break(&mut self) {
        if self.in_code_block || self.in_table {
            self.pending.push(("\n".to_string(), self.current_style));
        } else {
            self.flush_line();
        }
    }

    fn handle_task_list_marker(&mut self, checked: bool) {
        let marker = if checked { "☑" } else { "☐" };
        self.pending
            .push((format!("{marker} "), self.current_style));
    }

    fn handle_rule(&mut self) {
        if self.in_code_block {
            self.flush_code_block();
        }
        if self.in_table {
            self.flush_table_row();
        }
        let line = Line::from(vec![Span::styled(
            "────────────────────────────────────────",
            Style::default().fg(Color::DarkGray),
        )]);
        self.lines.push(line);
    }

    fn flush_line(&mut self) {
        if self.pending.is_empty() {
            return;
        }

        // If we have a pending table header, accumulate it
        if self.in_table && self.table_is_header {
            let text: String = self.pending.iter().map(|(s, _)| s.as_str()).collect();
            self.pending_table_header.push((text, self.current_style));
            self.pending.clear();
            return;
        }

        if self.in_table && !self.table_is_header {
            let text: String = self.pending.iter().map(|(s, _)| s.as_str()).collect();
            self.pending_table_line.push((text, self.current_style));
            self.pending.clear();
            return;
        }

        // Build indent prefix
        let indent_str = " ".repeat(self.indent as usize);

        // Build list marker
        let marker_text = if let Some(ref marker) = self.list_marker {
            format!("{marker} ")
        } else {
            String::new()
        };

        // Build blockquote prefix
        let bq_prefix: String = "│".repeat(self.blockquote_depth as usize);

        // Combine prefix
        let prefix = format!("{indent_str}{marker_text}{bq_prefix}");

        // Combine all pending text segments
        let text: String = self.pending.iter().map(|(s, _)| s.as_str()).collect();
        self.pending.clear();

        // Build spans: prefix with default style, then content with current style
        let mut spans = Vec::new();
        if !prefix.is_empty() {
            spans.push(Span::raw(prefix));
        }
        if !text.is_empty() {
            spans.push(Span::styled(text, self.current_style));
        }

        self.lines.push(Line::from(spans));
    }

    fn flush_code_block(&mut self) {
        self.flush_line();
        let last = self.lines.last();
        if last.is_some_and(|l| !l.spans.is_empty()) {
            let line = Line::from(vec![Span::raw("─".repeat(80))]);
            self.lines.push(line);
        }
    }

    fn flush_table_row(&mut self) {
        if self.table_is_header {
            if !self.pending_table_header.is_empty() {
                let spans: Vec<Span> = self
                    .pending_table_header
                    .iter()
                    .map(|(text, style)| Span::styled(text.clone(), style.add_modifier(Modifier::BOLD)))
                    .collect();
                self.lines.push(Line::from(spans));
                self.lines.push(Line::from(vec![Span::styled(
                    "───┼───────",
                    Style::default().fg(Color::DarkGray),
                )]));
            }
            self.table_is_header = false;
        } else if !self.pending_table_line.is_empty() {
            let spans: Vec<Span> = self
                .pending_table_line
                .iter()
                .map(|(text, style)| Span::styled(text.clone(), *style))
                .collect();
            self.lines.push(Line::from(spans));
        }
        self.pending_table_line.clear();
    }
}

pub fn render(f: &mut Frame<'_>, area: Rect, app: &mut App) {
    let readme_id_and_text = match &app.models_mode {
        crate::tui::app::ModelsMode::Search { results, .. } => {
            app.search_results_idx
                .and_then(|idx| results.get(idx).and_then(|r| r.readme.as_ref().map(|text| (r.model_id.clone(), text))))
        }
        crate::tui::app::ModelsMode::Files { selected_result, model_id, .. } => {
            selected_result.as_ref().and_then(|r| r.readme.as_ref().map(|text| (model_id.clone(), text)))
        }
        _ => None,
    };

    let lines = if let Some((id, text)) = readme_id_and_text {
        // Check cache
        if let Some((cached_id, cached_lines)) = &app.readme_cache {
            if cached_id == &id {
                cached_lines.clone()
            } else {
                let new_lines = MdRenderer::render_markdown(text);
                app.readme_cache = Some((id, new_lines.clone()));
                new_lines
            }
        } else {
            let new_lines = MdRenderer::render_markdown(text);
            app.readme_cache = Some((id, new_lines.clone()));
            new_lines
        }
    } else {
        vec![Line::raw("No README available.")]
    };

    let available_height = area.height.saturating_sub(2);
    let max_offset = lines.len().saturating_sub(available_height as usize) as u16;

    if app.readme_scroll_offset > max_offset {
        app.readme_scroll_offset = max_offset;
    }

    let start_idx = app.readme_scroll_offset as usize;
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();

    // Calculate max horizontal offset and truncate lines
    let max_line_width = visible_lines.iter().map(|line| line.width()).max().unwrap_or(0) as u16;
    let max_offset_x = max_line_width.saturating_sub(area.width);

    if app.readme_scroll_offset_x > max_offset_x {
        app.readme_scroll_offset_x = max_offset_x;
    }

    let scroll_x = app.readme_scroll_offset_x as usize;
    let truncated_lines: Vec<Line> = if scroll_x == 0 {
        visible_lines
    } else {
        visible_lines
            .into_iter()
            .map(|mut line| {
                let mut chars_seen = 0;
                line.spans.retain_mut(|span| {
                    let span_chars = span.content.chars().count();
                    let span_start = chars_seen;
                    let span_end = chars_seen + span_chars;
                    chars_seen = span_end;
                    if span_start >= scroll_x {
                        return false;
                    }
                    if span_end > scroll_x {
                        let skip = scroll_x - span_start;
                        span.content = span.content[skip..].to_string().into();
                        if span.content.is_empty() {
                            return false;
                        }
                    }
                    true
                });
                line
            })
            .collect()
    };

    let block = Block::default()
        .title(" README ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let wrap = ratatui::widgets::Wrap {
        trim: true,
    };
    let paragraph = Paragraph::new(truncated_lines).block(block).wrap(wrap);
    f.render_widget(paragraph, area);

    // Vertical scrollbar
    if lines.len() > available_height as usize {
        let scrollbar_area = Rect {
            x: area.right().saturating_sub(1),
            y: area.top(),
            width: 1,
            height: area.height,
        };

        let mut scrollbar_state = ScrollbarState::new(lines.len())
            .position(app.readme_scroll_offset as usize);

        f.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            scrollbar_area,
            &mut scrollbar_state,
        );
    }

    // Horizontal scrollbar
    if max_offset_x > 0 {
        let scrollbar_area = Rect {
            x: area.left(),
            y: area.bottom().saturating_sub(1),
            width: area.width,
            height: 1,
        };
        let bar = Line::from(Span::styled(
            "█".repeat(area.width as usize),
            Style::default().bg(Color::DarkGray),
        ));
        f.render_widget(bar, scrollbar_area);
    }
}
