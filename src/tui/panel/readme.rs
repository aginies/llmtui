use crate::tui::app::App;
use crate::tui::colors::*;
use pulldown_cmark::{Event, Options, Tag, TagEnd};
use ratatui::{
    prelude::*,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph},
};

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

impl Default for MdRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl MdRenderer {
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            pending: Vec::new(),
            current_style: Style::default(),
            in_code_block: false,
            code_block_style: Style::default()
                .fg(GREEN)
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
            self.flush_line();
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
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H2 => Style::default()
                        .fg(YELLOW)
                        .add_modifier(Modifier::BOLD),
                    pulldown_cmark::HeadingLevel::H3 => Style::default()
                        .fg(MAGENTA)
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
                self.flush_line();
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
            Tag::TableHead => {}
            Tag::TableRow => {}
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
                self.flush_line();
                self.in_list = false;
                self.list_marker = None;
            }
            TagEnd::Item => {}
            TagEnd::BlockQuote(_) => {
                if self.blockquote_depth > 0 {
                    self.blockquote_depth -= 1;
                }
            }
            TagEnd::TableHead => {
                self.flush_table_row();
                self.table_is_header = false;
            }
            TagEnd::TableRow => {
                self.flush_table_row();
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
            if self.table_is_header {
                self.pending_table_header
                    .push((text.to_string(), self.current_style));
            } else {
                self.pending_table_line
                    .push((text.to_string(), self.current_style));
            }
        } else {
            self.pending.push((text.to_string(), self.current_style));
        }
    }

    fn handle_code(&mut self, code: &str) {
        if self.in_table {
            if self.table_is_header {
                self.pending_table_header
                    .push((code.to_string(), Style::default().fg(GREEN)));
            } else {
                self.pending_table_line
                    .push((code.to_string(), Style::default().fg(GREEN)));
            }
        } else if self.in_code_block {
            self.pending.push((code.to_string(), self.code_block_style));
        } else {
            self.pending
                .push((format!("`{code}`"), self.current_style.fg(GREEN)));
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
            Style::default().fg(DIM_GRAY),
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
                    .map(|(text, style)| {
                        Span::styled(text.clone(), style.add_modifier(Modifier::BOLD))
                    })
                    .collect();
                self.lines.push(Line::from(spans));
                self.lines.push(Line::from(vec![Span::styled(
                    "───┼───────",
                    Style::default().fg(DIM_GRAY),
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
    let readme_state = match &app.models_mode {
        crate::tui::app::ModelsMode::Search { results, .. } => {
            app.search.search_results_idx.and_then(|idx| {
                results
                    .get(idx)
                    .map(|r| (r.model_id.clone(), r.readme.as_ref()))
            })
        }
        crate::tui::app::ModelsMode::Files {
            selected_result,
            model_id,
            ..
        } => selected_result
            .as_ref()
            .map(|r| (model_id.clone(), r.readme.as_ref())),
        _ => None,
    };

    let lines = match readme_state {
        Some((id, Some(text))) if !text.is_empty() => {
            // Content exists - use cache or render
            if let Some((cached_id, cached_lines)) = &app.search.readme_cache {
                if cached_id == &id {
                    cached_lines.clone()
                } else {
                    app.picker.readme_scroll_offset = 0;
                    let new_lines = MdRenderer::render_markdown(text);
                    app.search.readme_cache = Some((id, new_lines.clone()));
                    new_lines
                }
            } else {
                let new_lines = MdRenderer::render_markdown(text);
                app.search.readme_cache = Some((id, new_lines.clone()));
                new_lines
            }
        }
        Some((_, Some(_))) => {
            app.picker.readme_scroll_offset = 0;
            // Text is Some but empty
            vec![Line::from(Span::styled(
                "no README available",
                Style::default().fg(RED),
            ))]
        }
        Some((_, None)) => {
            app.picker.readme_scroll_offset = 0;
            // Not yet fetched
            vec![Line::from(Span::styled(
                "Press -> to Fetch the README.md",
                Style::default().fg(GREEN),
            ))]
        }
        None => {
            app.picker.readme_scroll_offset = 0;
            vec![Line::raw("Select a model to view README.")]
        }
    };

    let available_height = area.height.saturating_sub(2);
    let max_offset = lines.len().saturating_sub(available_height as usize) as u16;

    if app.picker.readme_scroll_offset > max_offset.into() {
        app.picker.readme_scroll_offset = max_offset.into();
    }

    let start_idx = app.picker.readme_scroll_offset;
    let visible_lines: Vec<Line> = lines
        .iter()
        .skip(start_idx)
        .take(available_height as usize)
        .cloned()
        .collect();

    let is_focused = app.ui.active_panel == crate::tui::app::ActivePanel::SearchReadme;
    let border_color = if is_focused {
        GREEN
    } else {
        MID_GRAY
    };
    let block = Block::default()
        .title(crate::t!("panel.title.readme"))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .border_type(BorderType::Rounded);
    let wrap = ratatui::widgets::Wrap { trim: true };
    let paragraph = Paragraph::new(visible_lines).block(block).wrap(wrap);
    f.render_widget(paragraph, area);

    // Vertical scrollbar
    if lines.len() > available_height as usize {
        crate::tui::render_vertical_scrollbar(
            f,
            area,
            lines.len(),
            app.picker.readme_scroll_offset,
            0,
            0,
        );
    }
}
