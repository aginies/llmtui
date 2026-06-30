use std::sync::LazyLock;

use ratatui::style::{Color, Modifier, Style};

// ── Named colors ─────────────────────────────────────────────────────────────

/// Primary accent — labels, titles, hints, active states. Rich amber instead of flat yellow.
pub const ACCENT: Color = Color::Rgb(255, 203, 107);

/// Success / focus / selection. Softer, more pleasing green.
pub const GREEN: Color = Color::Rgb(166, 227, 161);

/// Bright green for focused panel borders — vivid teal-green for high visibility.
pub const LIGHT_GREEN: Color = Color::Rgb(80, 220, 140);

/// Error / danger / dirty state. Warm red, less harsh than pure red.
pub const RED: Color = Color::Rgb(243, 139, 168);

/// Default body text / value color. Off-white for reduced eye strain.
pub const WHITE: Color = Color::Rgb(205, 214, 244);

/// Foreground for text on colored backgrounds.
pub const BLACK: Color = Color::Black;

/// Unfocused borders, disabled elements. Muted blue-gray.
pub const GRAY: Color = Color::Rgb(100, 116, 139);

/// Secondary text — timestamps, footers, descriptions. Soft blue-gray.
pub const DIM_GRAY: Color = Color::Rgb(137, 180, 250);

/// Mid-tone gray for disabled elements, unfocused borders.
pub const MID_GRAY: Color = Color::Rgb(99, 110, 125);

/// Brighter gray for descriptions and secondary text. Readable on dark backgrounds.
pub const DESCRIPTION_GRAY: Color = Color::Rgb(176, 190, 197);

/// Brighter gray for unfocused panel borders. Cool blue-gray.
pub const LIGHT_GRAY: Color = Color::Rgb(115, 130, 155);

/// Links, URLs, data values, metrics, log info. Soft cyan — easy on eyes.
pub const CYAN: Color = Color::Rgb(129, 229, 249);

/// Special indicators — sort labels, MTP, MoE architecture. Soft magenta.
pub const MAGENTA: Color = Color::Rgb(203, 166, 247);

/// Rare / niche use (backend cached indicator). Soft blue.
pub const BLUE: Color = Color::Rgb(137, 180, 250);

/// Background color for status bar and toasts. Deep navy.
pub const BG_DARK: Color = Color::Rgb(17, 24, 39);

/// VRAM usage bar — low utilization (0-60%). Vivid green.
pub const VRAM_GREEN: Color = Color::Rgb(74, 222, 128);

/// VRAM usage bar — medium utilization (60-80%). Warm amber.
pub const VRAM_YELLOW: Color = Color::Rgb(251, 191, 36);

/// VRAM usage bar — high utilization (80-100%). Sharp red.
pub const VRAM_RED: Color = Color::Rgb(248, 113, 113);

// ── Common Style patterns ────────────────────────────────────────────────────

/// Panel title / section header: accent text, bold.
pub static TITLE: LazyLock<Style> =
    LazyLock::new(|| Style::default().fg(ACCENT).add_modifier(Modifier::BOLD));

/// Secondary / dimmed text (timestamps, footers, descriptions).
pub static DIM_TEXT: LazyLock<Style> = LazyLock::new(|| Style::default().fg(DIM_GRAY));

/// Focused panel border color — vivid teal-green.
pub static BORDER_FOCUSED: LazyLock<Style> = LazyLock::new(|| Style::default().fg(LIGHT_GREEN));
