use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};

// ── Named colors ─────────────────────────────────────────────────────────────

/// Primary accent color — labels, titles, hints, active states
pub const YELLOW: Color = Color::Yellow;

/// Success / focus / selection color
pub const GREEN: Color = Color::Green;

/// Bright green for focused panel borders — more visible than ANSI Green
pub const LIGHT_GREEN: Color = Color::Rgb(29, 168, 29);

/// Error / danger / dirty state color
pub const RED: Color = Color::Red;

/// Default body text / value color
pub const WHITE: Color = Color::White;

/// Foreground for text on colored backgrounds
pub const BLACK: Color = Color::Black;

/// Unfocused borders, disabled elements
pub const GRAY: Color = Color::Gray;

/// Secondary text — timestamps, footers, descriptions, subtitles
/// Brighter than ANSI DarkGray for better contrast on dark backgrounds
pub const DIM_GRAY: Color = Color::Gray;

/// Mid-tone gray for disabled elements, unfocused borders
/// Brighter than DIM_GRAY for visual hierarchy
pub const MID_GRAY: Color = Color::DarkGray;

/// Brighter gray for descriptions and secondary text
/// Readable on dark backgrounds
pub const DESCRIPTION_GRAY: Color = Color::Rgb(180, 180, 180);

/// Brighter gray for unfocused panel borders
pub const LIGHT_GRAY: Color = Color::Rgb(120, 120, 140);

/// Links, URLs, data values, metrics, log info
pub const CYAN: Color = Color::Cyan;

/// Special indicators — sort labels, MTP, MoE architecture
pub const MAGENTA: Color = Color::Magenta;

/// Rare / niche use (backend cached indicator)
pub const BLUE: Color = Color::Blue;

/// VRAM usage bar — low utilization (0-60%)
pub const VRAM_GREEN: Color = Color::Rgb(0, 200, 0);

/// VRAM usage bar — medium utilization (60-80%)
pub const VRAM_YELLOW: Color = Color::Rgb(255, 200, 0);

/// VRAM usage bar — high utilization (80-100%)
pub const VRAM_RED: Color = Color::Rgb(255, 60, 60);

// ── Common Style patterns ────────────────────────────────────────────────────

/// Selected row / item: white text on yellow background, bold
#[allow(dead_code)]
pub static SELECTED_ROW: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(WHITE)
        .bg(YELLOW)
        .add_modifier(Modifier::BOLD)
});

/// Edit cursor: black text on yellow background
#[allow(dead_code)]
pub static EDIT_CURSOR: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(BLACK)
        .bg(YELLOW)
});

/// Panel title / section header: yellow text, bold
#[allow(dead_code)]
pub static TITLE: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(YELLOW)
        .add_modifier(Modifier::BOLD)
});

/// Panel title / section header (dimmed, for disabled state)
#[allow(dead_code)]
pub static TITLE_DIM: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(DIM_GRAY)
        .add_modifier(Modifier::BOLD)
});

/// Label / field name color
#[allow(dead_code)]
pub static LABEL: LazyLock<Style> = LazyLock::new(|| Style::default().fg(YELLOW));

/// Default body text / values
#[allow(dead_code)]
pub static BODY_TEXT: LazyLock<Style> = LazyLock::new(|| Style::default().fg(WHITE));

/// Secondary / dimmed text (timestamps, footers, descriptions)
#[allow(dead_code)]
pub static DIM_TEXT: LazyLock<Style> = LazyLock::new(|| Style::default().fg(DIM_GRAY));

/// Link / URL color
#[allow(dead_code)]
pub static LINK_TEXT: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD)
});

/// Focused panel border color
#[allow(dead_code)]
pub static BORDER_FOCUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(LIGHT_GREEN));

/// Unfocused panel border color
#[allow(dead_code)]
pub static BORDER_UNFOCUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(MID_GRAY));

/// Status: success / complete / loaded
#[allow(dead_code)]
pub static STATUS_SUCCESS: LazyLock<Style> = LazyLock::new(|| Style::default().fg(GREEN));

/// Status: loading / active / downloading
#[allow(dead_code)]
pub static STATUS_LOADING: LazyLock<Style> = LazyLock::new(|| Style::default().fg(YELLOW));

/// Status: error / failed / cancelled
#[allow(dead_code)]
pub static STATUS_ERROR: LazyLock<Style> = LazyLock::new(|| Style::default().fg(RED));

/// Status: paused
#[allow(dead_code)]
pub static STATUS_PAUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(DIM_GRAY));

/// Dirty / uncommitted setting value
#[allow(dead_code)]
pub static DIRTY: LazyLock<Style> = LazyLock::new(|| Style::default().fg(RED));

/// Disabled setting name / value
#[allow(dead_code)]
pub static DISABLED: LazyLock<Style> = LazyLock::new(|| {
    Style::default()
        .fg(GRAY)
        .add_modifier(Modifier::DIM)
});

/// Sort direction label color
#[allow(dead_code)]
pub static SORT_LABEL: LazyLock<Style> = LazyLock::new(|| Style::default().fg(CYAN));
