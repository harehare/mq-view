//! Centralized color palette so the renderer, syntax highlighter, and pager
//! draw from one theme instead of scattered hardcoded colors. Uses
//! `Color::TrueColor` (real RGB) rather than the terminal's 16-color ANSI
//! palette so dark/light actually differ regardless of terminal settings.

use colored::Color;

/// `Auto` guesses dark/light from `COLORFGBG` (no reliable OSC 11 query
/// without risking a hang in non-interactive contexts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum, Default)]
pub enum ThemeMode {
    #[default]
    Auto,
    Dark,
    Light,
}

/// Indexed the same way as `SyntaxHighlighter`'s tree-sitter highlight
/// capture list (attribute, constant, function.builtin, ...).
pub type SyntaxPalette = [(u8, u8, u8); 26];

const DARK_SYNTAX: SyntaxPalette = [
    (0, 170, 170),   // attribute
    (170, 0, 170),   // constant
    (170, 170, 0),   // function.builtin
    (60, 60, 220),   // function
    (255, 95, 255),  // keyword
    (220, 220, 220), // operator
    (0, 170, 170),   // property
    (128, 128, 128), // punctuation
    (128, 128, 128), // punctuation.bracket
    (128, 128, 128), // punctuation.delimiter
    (0, 170, 0),     // string
    (95, 215, 95),   // string.special
    (60, 60, 220),   // tag
    (170, 170, 0),   // type
    (215, 215, 95),  // type.builtin
    (220, 220, 220), // variable
    (170, 0, 170),   // variable.builtin
    (0, 170, 170),   // variable.parameter
    (128, 128, 128), // comment
    (170, 0, 170),   // number
    (170, 0, 170),   // boolean
    (0, 170, 170),   // escape
    (170, 170, 0),   // label
    (0, 170, 170),   // namespace
    (170, 170, 0),   // constructor
    (220, 220, 220), // embedded
];

const LIGHT_SYNTAX: SyntaxPalette = [
    (0, 128, 128),   // attribute
    (135, 0, 135),   // constant
    (150, 120, 0),   // function.builtin
    (0, 0, 180),     // function
    (170, 0, 170),   // keyword
    (60, 60, 60),    // operator
    (0, 128, 128),   // property
    (110, 110, 110), // punctuation
    (110, 110, 110), // punctuation.bracket
    (110, 110, 110), // punctuation.delimiter
    (0, 120, 0),     // string
    (0, 150, 60),    // string.special
    (0, 0, 180),     // tag
    (150, 100, 0),   // type
    (170, 110, 0),   // type.builtin
    (40, 40, 40),    // variable
    (135, 0, 135),   // variable.builtin
    (0, 128, 128),   // variable.parameter
    (120, 120, 120), // comment
    (135, 0, 135),   // number
    (135, 0, 135),   // boolean
    (0, 128, 128),   // escape
    (150, 120, 0),   // label
    (0, 128, 128),   // namespace
    (150, 120, 0),   // constructor
    (40, 40, 40),    // embedded
];

/// Semantic colors used across rendering, syntax highlighting, and the
/// pager UI.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Full-width heading background bar (depth 1..=5); kept bright in both
    /// themes since it's an inverted banner, not text on the terminal bg.
    pub heading_bg: [Color; 5],
    pub heading_bg_fg: Color,
    /// Heading foreground when full-width highlighting is off; this one
    /// does need to vary by theme since it sits on the terminal's own bg.
    pub heading_plain: [Color; 5],
    /// Depth >= 6 in both modes.
    pub heading_fallback: Color,
    pub muted: Color,
    pub link: Color,
    pub image: Color,
    pub checkbox_bullet: Color,
    pub code_border: Color,
    pub table_border: Color,
    pub inline_code: Color,
    /// Note, Tip, Important, Warning, Caution — order matches `CALLOUTS` in
    /// `renderer.rs`.
    pub callout: [Color; 5],
    pub search_match: Color,
    pub search_current: Color,
    pub ui_accent: Color,
    pub ui_muted: Color,
    pub syntax: SyntaxPalette,
}

impl Theme {
    pub fn dark() -> Self {
        Self {
            heading_bg: [
                Color::TrueColor {
                    r: 92,
                    g: 92,
                    b: 255,
                },
                Color::TrueColor {
                    r: 0,
                    g: 205,
                    b: 205,
                },
                Color::TrueColor {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                Color::TrueColor { r: 0, g: 205, b: 0 },
                Color::TrueColor {
                    r: 205,
                    g: 0,
                    b: 205,
                },
            ],
            heading_bg_fg: Color::TrueColor { r: 0, g: 0, b: 0 },
            heading_plain: [
                Color::TrueColor {
                    r: 92,
                    g: 92,
                    b: 255,
                },
                Color::TrueColor {
                    r: 0,
                    g: 205,
                    b: 205,
                },
                Color::TrueColor {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                Color::TrueColor { r: 0, g: 205, b: 0 },
                Color::TrueColor {
                    r: 205,
                    g: 0,
                    b: 205,
                },
            ],
            heading_fallback: Color::TrueColor {
                r: 229,
                g: 229,
                b: 229,
            },
            muted: Color::TrueColor {
                r: 128,
                g: 128,
                b: 128,
            },
            link: Color::TrueColor {
                r: 92,
                g: 92,
                b: 255,
            },
            image: Color::TrueColor {
                r: 95,
                g: 215,
                b: 95,
            },
            checkbox_bullet: Color::TrueColor {
                r: 205,
                g: 0,
                b: 205,
            },
            code_border: Color::TrueColor {
                r: 128,
                g: 128,
                b: 128,
            },
            table_border: Color::TrueColor {
                r: 0,
                g: 255,
                b: 255,
            },
            inline_code: Color::TrueColor {
                r: 255,
                g: 255,
                b: 0,
            },
            callout: [
                Color::TrueColor { r: 0, g: 0, b: 238 },
                Color::TrueColor { r: 0, g: 205, b: 0 },
                Color::TrueColor {
                    r: 205,
                    g: 0,
                    b: 205,
                },
                Color::TrueColor {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                Color::TrueColor { r: 205, g: 0, b: 0 },
            ],
            search_match: Color::TrueColor {
                r: 205,
                g: 205,
                b: 0,
            },
            search_current: Color::TrueColor {
                r: 255,
                g: 140,
                b: 0,
            },
            ui_accent: Color::TrueColor { r: 0, g: 0, b: 238 },
            ui_muted: Color::TrueColor {
                r: 128,
                g: 128,
                b: 128,
            },
            syntax: DARK_SYNTAX,
        }
    }

    pub fn light() -> Self {
        Self {
            heading_bg: [
                Color::TrueColor {
                    r: 92,
                    g: 92,
                    b: 255,
                },
                Color::TrueColor {
                    r: 0,
                    g: 205,
                    b: 205,
                },
                Color::TrueColor {
                    r: 205,
                    g: 205,
                    b: 0,
                },
                Color::TrueColor { r: 0, g: 205, b: 0 },
                Color::TrueColor {
                    r: 205,
                    g: 0,
                    b: 205,
                },
            ],
            heading_bg_fg: Color::TrueColor { r: 0, g: 0, b: 0 },
            heading_plain: [
                Color::TrueColor {
                    r: 0,
                    g: 90,
                    b: 200,
                },
                Color::TrueColor {
                    r: 0,
                    g: 140,
                    b: 140,
                },
                Color::TrueColor {
                    r: 150,
                    g: 120,
                    b: 0,
                },
                Color::TrueColor { r: 0, g: 130, b: 0 },
                Color::TrueColor {
                    r: 150,
                    g: 0,
                    b: 150,
                },
            ],
            heading_fallback: Color::TrueColor {
                r: 40,
                g: 40,
                b: 40,
            },
            muted: Color::TrueColor {
                r: 100,
                g: 100,
                b: 100,
            },
            link: Color::TrueColor {
                r: 0,
                g: 90,
                b: 200,
            },
            image: Color::TrueColor {
                r: 0,
                g: 130,
                b: 60,
            },
            checkbox_bullet: Color::TrueColor {
                r: 150,
                g: 0,
                b: 150,
            },
            code_border: Color::TrueColor {
                r: 130,
                g: 130,
                b: 130,
            },
            table_border: Color::TrueColor {
                r: 0,
                g: 120,
                b: 140,
            },
            inline_code: Color::TrueColor {
                r: 150,
                g: 110,
                b: 0,
            },
            callout: [
                Color::TrueColor {
                    r: 0,
                    g: 90,
                    b: 200,
                },
                Color::TrueColor { r: 0, g: 130, b: 0 },
                Color::TrueColor {
                    r: 150,
                    g: 0,
                    b: 150,
                },
                Color::TrueColor {
                    r: 150,
                    g: 110,
                    b: 0,
                },
                Color::TrueColor { r: 180, g: 0, b: 0 },
            ],
            search_match: Color::TrueColor {
                r: 150,
                g: 110,
                b: 0,
            },
            search_current: Color::TrueColor {
                r: 210,
                g: 90,
                b: 0,
            },
            ui_accent: Color::TrueColor {
                r: 0,
                g: 90,
                b: 200,
            },
            ui_muted: Color::TrueColor {
                r: 110,
                g: 110,
                b: 110,
            },
            syntax: LIGHT_SYNTAX,
        }
    }

    /// Terminals that don't set `COLORFGBG` (iTerm2, Kitty, Alacritty, ...)
    /// fall back to `Dark` under `Auto`.
    pub fn resolve(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Dark => Self::dark(),
            ThemeMode::Light => Self::light(),
            ThemeMode::Auto => {
                if is_light_background() {
                    Self::light()
                } else {
                    Self::dark()
                }
            }
        }
    }
}

fn is_light_background() -> bool {
    std::env::var("COLORFGBG")
        .ok()
        .and_then(|v| v.rsplit(';').next().map(str::to_string))
        .and_then(|bg| bg.parse::<u8>().ok())
        .is_some_and(|bg| matches!(bg, 7 | 15))
}
