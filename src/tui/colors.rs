use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};

// ── Named colors ─────────────────────────────────────────────────────────────

/// Primary accent color — labels, titles, hints, active states
pub const YELLOW: Color = Color::Yellow;

/// Success / focus / selection color
pub const GREEN: Color = Color::Green;

/// Error / danger / dirty state color
pub const RED: Color = Color::Red;

/// Default body text / value color
pub const WHITE: Color = Color::White;

/// Foreground for text on colored backgrounds
pub const BLACK: Color = Color::Black;

/// Unfocused borders, disabled elements
pub const GRAY: Color = Color::Gray;

/// Secondary text — timestamps, footers, descriptions, subtitles
pub const DARK_GRAY: Color = Color::DarkGray;

/// Links, URLs, data values, metrics, log info
pub const CYAN: Color = Color::Cyan;

/// Special indicators — sort labels, MTP, MoE architecture
pub const MAGENTA: Color = Color::Magenta;

/// Rare / niche use (backend cached indicator)
pub const BLUE: Color = Color::Blue;

// ── Common Style patterns ────────────────────────────────────────────────────

/// Selected row / item: black text on green background, bold
pub static SELECTED_ROW: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(BLACK)
        .bg(GREEN)
        .add_modifier(Modifier::BOLD)
});

/// Edit cursor: black text on yellow background
pub static EDIT_CURSOR: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(BLACK)
        .bg(YELLOW)
});

/// Panel title / section header: yellow text, bold
pub static TITLE: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(YELLOW)
        .add_modifier(Modifier::BOLD)
});

/// Panel title / section header (dimmed, for disabled state)
pub static TITLE_DIM: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(YELLOW)
        .add_modifier(Modifier::DIM)
});

/// Label / field name color
pub static LABEL: LazyLock<Style> = LazyLock::new(|| Style::default().fg(YELLOW));

/// Default body text / values
pub static BODY_TEXT: LazyLock<Style> = LazyLock::new(|| Style::default().fg(WHITE));

/// Secondary / dimmed text (timestamps, footers, descriptions)
pub static DIM_TEXT: LazyLock<Style> = LazyLock::new(|| Style::default().fg(DARK_GRAY));

/// Link / URL color
pub static LINK_TEXT: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
});

/// Focused panel border color
pub static BORDER_FOCUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(GREEN));

/// Unfocused panel border color
pub static BORDER_UNFOCUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(GRAY));

/// Status: success / complete / loaded
pub static STATUS_SUCCESS: LazyLock<Style> = LazyLock::new(|| Style::default().fg(GREEN));

/// Status: loading / active / downloading
pub static STATUS_LOADING: LazyLock<Style> = LazyLock::new(|| Style::default().fg(YELLOW));

/// Status: error / failed / cancelled
pub static STATUS_ERROR: LazyLock<Style> = LazyLock::new(|| Style::default().fg(RED));

/// Status: paused
pub static STATUS_PAUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(WHITE));

/// Dirty / uncommitted setting value
pub static DIRTY: LazyLock<Style> = LazyLock::new(|| Style::default().fg(RED));

/// Disabled setting name / value
pub static DISABLED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(GRAY));

/// Sort direction label color
pub static SORT_LABEL: LazyLock<Style> = LazyLock::new(|| Style::default().fg(MAGENTA));
