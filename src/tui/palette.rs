use ratatui::style::Color;

/// Terminal color mode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ColorMode {
    #[default]
    TrueColor,
    Indexed256,
}

/// Palette containing all UI colors for a given mode.
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub accent: Color,
    pub green: Color,
    pub light_green: Color,
    pub red: Color,
    pub white: Color,
    pub gray: Color,
    pub dim_gray: Color,
    pub mid_gray: Color,
    pub description_gray: Color,
    pub light_gray: Color,
    pub cyan: Color,
    pub magenta: Color,
    pub blue: Color,
    pub bg_dark: Color,
    pub vram_green: Color,
    pub vram_yellow: Color,
    pub vram_red: Color,
}

impl Palette {
    /// True-color palette (exact RGB values).
    pub const TRUECOLOR: Self = Self {
        accent: Color::Rgb(255, 203, 107),
        green: Color::Rgb(166, 227, 161),
        light_green: Color::Rgb(80, 220, 140),
        red: Color::Rgb(243, 139, 168),
        white: Color::Rgb(205, 214, 244),
        gray: Color::Rgb(100, 116, 139),
        dim_gray: Color::Rgb(137, 180, 250),
        mid_gray: Color::Rgb(99, 110, 125),
        description_gray: Color::Rgb(176, 190, 197),
        light_gray: Color::Rgb(115, 130, 155),
        cyan: Color::Rgb(129, 229, 249),
        magenta: Color::Rgb(203, 166, 247),
        blue: Color::Rgb(137, 180, 250),
        bg_dark: Color::Rgb(17, 24, 39),
        vram_green: Color::Rgb(74, 222, 128),
        vram_yellow: Color::Rgb(251, 191, 36),
        vram_red: Color::Rgb(248, 113, 113),
    };

    /// 256-color indexed palette (standard cube + grayscale).
    pub const INDEXED: Self = Self {
        accent: Color::Indexed(215),
        green: Color::Indexed(120),
        light_green: Color::Indexed(84),
        red: Color::Indexed(210),
        white: Color::Indexed(189),
        gray: Color::Indexed(66),
        dim_gray: Color::Indexed(147),
        mid_gray: Color::Indexed(60),
        description_gray: Color::Indexed(103),
        light_gray: Color::Indexed(67),
        cyan: Color::Indexed(123),
        magenta: Color::Indexed(183),
        blue: Color::Indexed(147),
        bg_dark: Color::Rgb(17, 24, 39),
        vram_green: Color::Indexed(49),
        vram_yellow: Color::Indexed(214),
        vram_red: Color::Indexed(209),
    };
}

/// Global palette, set once at startup.
static PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();

/// Initialize the palette with the detected/appointed color mode.
pub fn init(mode: ColorMode) {
    PALETTE.get_or_init(|| match mode {
        ColorMode::TrueColor => Palette::TRUECOLOR,
        ColorMode::Indexed256 => Palette::INDEXED,
    });
}

/// Detect color mode from environment variables.
pub fn detect_color_mode() -> ColorMode {
    let colorterm = std::env::var("COLORTERM").unwrap_or_default();
    let term = std::env::var("TERM").unwrap_or_default();
    let term_program = std::env::var("TERM_PROGRAM").unwrap_or_default();

    // truecolor / 24bit
    if colorterm.contains("truecolor") || colorterm.contains("24bit") {
        return ColorMode::TrueColor;
    }

    // tmux, screen, xterm, etc. with 256color support
    if term.contains("256color") || term.contains("256colour") {
        return ColorMode::TrueColor;
    }

    // Apple Terminal.app always supports truecolor
    if term_program.contains("Apple_Terminal") {
        return ColorMode::TrueColor;
    }

    ColorMode::TrueColor
}

/// Get the current palette. Initializes if not already done.
pub fn get() -> Palette {
    *PALETTE.get_or_init(|| match detect_color_mode() {
        ColorMode::TrueColor => Palette::TRUECOLOR,
        ColorMode::Indexed256 => Palette::INDEXED,
    })
}

/// Accessor functions — drop-in replacements for constants.

pub fn accent() -> Color { get().accent }
pub fn green() -> Color { get().green }
pub fn light_green() -> Color { get().light_green }
pub fn red() -> Color { get().red }
pub fn white() -> Color { get().white }
pub fn gray() -> Color { get().gray }
pub fn dim_gray() -> Color { get().dim_gray }
pub fn mid_gray() -> Color { get().mid_gray }
pub fn description_gray() -> Color { get().description_gray }
pub fn light_gray() -> Color { get().light_gray }
pub fn cyan() -> Color { get().cyan }
pub fn magenta() -> Color { get().magenta }
pub fn blue() -> Color { get().blue }
pub fn bg_dark() -> Color { get().bg_dark }
pub fn vram_green() -> Color { get().vram_green }
pub fn vram_yellow() -> Color { get().vram_yellow }
pub fn vram_red() -> Color { get().vram_red }
pub fn black() -> Color { Color::Black }
