//! TUI colour themes.
//!
//! The renderer keeps the military-green palette as its semantic base. After a
//! frame is drawn, `Theme::paint` remaps those base colours to the selected
//! theme. New widgets therefore inherit every theme without a second colour
//! table, and the default theme is a byte-for-byte identity mapping.

use ratatui::buffer::Buffer;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub id: &'static str,
    pub name_zh: &'static str,
    pub name_en: &'static str,
    pub app_bg: Color,
    pub panel_bg: Color,
    pub menu_bg: Color,
    pub menu_selected_bg: Color,
    pub primary: Color,
    pub primary_bright: Color,
    pub dim: Color,
    pub faint: Color,
    pub rule: Color,
    pub link_hover: Color,
    pub white: Color,
    pub body: Color,
    pub muted: Color,
    pub code_text: Color,
    pub input_text: Color,
    pub on_bright: Color,
    pub warning: Color,
    pub danger: Color,
    pub code_bg: Color,
    pub code_added_bg: Color,
    pub code_removed_bg: Color,
}

macro_rules! theme {
    (
        $id:literal, $zh:literal, $en:literal,
        bg = ($app:expr, $panel:expr, $menu:expr, $selected:expr),
        accent = ($primary:expr, $bright:expr, $dim:expr, $faint:expr, $rule:expr, $hover:expr),
        text = ($white:expr, $body:expr, $muted:expr, $code:expr, $input:expr, $on_bright:expr),
        semantic = ($warning:expr, $danger:expr),
        code = ($code_bg:expr, $added:expr, $removed:expr)
    ) => {
        Theme {
            id: $id,
            name_zh: $zh,
            name_en: $en,
            app_bg: $app,
            panel_bg: $panel,
            menu_bg: $menu,
            menu_selected_bg: $selected,
            primary: $primary,
            primary_bright: $bright,
            dim: $dim,
            faint: $faint,
            rule: $rule,
            link_hover: $hover,
            white: $white,
            body: $body,
            muted: $muted,
            code_text: $code,
            input_text: $input,
            on_bright: $on_bright,
            warning: $warning,
            danger: $danger,
            code_bg: $code_bg,
            code_added_bg: $added,
            code_removed_bg: $removed,
        }
    };
}

pub const THEMES: &[Theme] = &[
    theme!(
        "military",
        "军旅绿",
        "Military Green",
        bg = (
            Color::Rgb(0x06, 0x10, 0x0a),
            Color::Rgb(0x08, 0x14, 0x0d),
            Color::Rgb(0x0a, 0x1f, 0x12),
            Color::Rgb(0x14, 0x3d, 0x24)
        ),
        accent = (
            Color::Rgb(0x70, 0x8a, 0x41),
            Color::Rgb(0x8a, 0xa6, 0x52),
            Color::Rgb(0x8b, 0x96, 0x6f),
            Color::Rgb(0x68, 0x73, 0x58),
            Color::Rgb(0x4a, 0x59, 0x30),
            Color::Rgb(0xb7, 0xff, 0xc8)
        ),
        text = (
            Color::White,
            Color::Indexed(250),
            Color::Indexed(245),
            Color::Indexed(252),
            Color::Indexed(255),
            Color::Black
        ),
        semantic = (Color::Yellow, Color::LightRed),
        code = (Color::Indexed(236), Color::Indexed(22), Color::Indexed(52))
    ),
    theme!(
        "matrix",
        "矩阵绿",
        "Matrix Green",
        bg = (
            Color::Rgb(0x00, 0x00, 0x00),
            Color::Rgb(0x00, 0x1a, 0x08),
            Color::Rgb(0x00, 0x20, 0x0a),
            Color::Rgb(0x00, 0x4d, 0x1a)
        ),
        accent = (
            Color::Rgb(0x00, 0xff, 0x41),
            Color::Rgb(0xb6, 0xff, 0xb0),
            Color::Rgb(0x00, 0xb3, 0x2d),
            Color::Rgb(0x00, 0x66, 0x22),
            Color::Rgb(0x00, 0x3b, 0x12),
            Color::Rgb(0xe0, 0xff, 0xe0)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xc8, 0xff, 0xd0),
            Color::Rgb(0x6f, 0x9f, 0x78),
            Color::Rgb(0x9d, 0xff, 0xb0),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x00, 0x00, 0x00)
        ),
        semantic = (Color::Rgb(0xff, 0xd7, 0x00), Color::Rgb(0xff, 0x5f, 0x56)),
        code = (
            Color::Rgb(0x00, 0x1a, 0x08),
            Color::Rgb(0x00, 0x3b, 0x12),
            Color::Rgb(0x3b, 0x0a, 0x0a)
        )
    ),
    theme!(
        "mono",
        "黑白灰",
        "Monochrome",
        bg = (
            Color::Rgb(0x0b, 0x0b, 0x0b),
            Color::Rgb(0x14, 0x14, 0x14),
            Color::Rgb(0x1a, 0x1a, 0x1a),
            Color::Rgb(0x33, 0x33, 0x33)
        ),
        accent = (
            Color::Rgb(0xf2, 0xf2, 0xf2),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xb3, 0xb3, 0xb3),
            Color::Rgb(0x73, 0x73, 0x73),
            Color::Rgb(0x3d, 0x3d, 0x3d),
            Color::Rgb(0xff, 0xff, 0xff)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xd9, 0xd9, 0xd9),
            Color::Rgb(0x8a, 0x8a, 0x8a),
            Color::Rgb(0xf2, 0xf2, 0xf2),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x0b, 0x0b, 0x0b)
        ),
        semantic = (Color::Rgb(0xe6, 0xc1, 0x5a), Color::Rgb(0xff, 0x6b, 0x6b)),
        code = (
            Color::Rgb(0x1a, 0x1a, 0x1a),
            Color::Rgb(0x24, 0x33, 0x24),
            Color::Rgb(0x33, 0x24, 0x24)
        )
    ),
    theme!(
        "ocean",
        "深蓝",
        "Deep Blue",
        bg = (
            Color::Rgb(0x06, 0x11, 0x1f),
            Color::Rgb(0x0a, 0x1a, 0x2e),
            Color::Rgb(0x0d, 0x22, 0x38),
            Color::Rgb(0x17, 0x3a, 0x5c)
        ),
        accent = (
            Color::Rgb(0x4d, 0xa3, 0xff),
            Color::Rgb(0x9c, 0xcc, 0xff),
            Color::Rgb(0x2f, 0x7f, 0xbf),
            Color::Rgb(0x1e, 0x4f, 0x73),
            Color::Rgb(0x12, 0x30, 0x4a),
            Color::Rgb(0xcf, 0xe8, 0xff)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xd6, 0xe6, 0xf5),
            Color::Rgb(0x7f, 0x9f, 0xb8),
            Color::Rgb(0xb8, 0xd9, 0xff),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x03, 0x10, 0x1c)
        ),
        semantic = (Color::Rgb(0xff, 0xcc, 0x66), Color::Rgb(0xff, 0x6b, 0x6b)),
        code = (
            Color::Rgb(0x0a, 0x1a, 0x2e),
            Color::Rgb(0x12, 0x3a, 0x2a),
            Color::Rgb(0x3a, 0x16, 0x20)
        )
    ),
    theme!(
        "sakura",
        "樱花粉",
        "Sakura",
        bg = (
            Color::Rgb(0x1a, 0x0f, 0x14),
            Color::Rgb(0x24, 0x15, 0x1c),
            Color::Rgb(0x2a, 0x18, 0x20),
            Color::Rgb(0x4a, 0x24, 0x34)
        ),
        accent = (
            Color::Rgb(0xff, 0x9e, 0xc4),
            Color::Rgb(0xff, 0xd6, 0xe6),
            Color::Rgb(0xd1, 0x7a, 0x9c),
            Color::Rgb(0x8a, 0x4d, 0x65),
            Color::Rgb(0x4a, 0x24, 0x34),
            Color::Rgb(0xff, 0xe3, 0xef)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xf5, 0xe6, 0xec),
            Color::Rgb(0xb8, 0x92, 0xa1),
            Color::Rgb(0xff, 0xd6, 0xe6),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x1a, 0x0f, 0x14)
        ),
        semantic = (Color::Rgb(0xff, 0xd1, 0x66), Color::Rgb(0xff, 0x6b, 0x8a)),
        code = (
            Color::Rgb(0x24, 0x15, 0x1c),
            Color::Rgb(0x1f, 0x3a, 0x2a),
            Color::Rgb(0x3a, 0x16, 0x20)
        )
    ),
    theme!(
        "copper",
        "橙铜",
        "Copper Orange",
        bg = (
            Color::Rgb(0x1a, 0x12, 0x0b),
            Color::Rgb(0x24, 0x1a, 0x10),
            Color::Rgb(0x2a, 0x1e, 0x12),
            Color::Rgb(0x4a, 0x2a, 0x10)
        ),
        accent = (
            Color::Rgb(0xf3, 0x70, 0x21),
            Color::Rgb(0xff, 0xb0, 0x66),
            Color::Rgb(0xc2, 0x6a, 0x2a),
            Color::Rgb(0x7a, 0x42, 0x18),
            Color::Rgb(0x4a, 0x2a, 0x10),
            Color::Rgb(0xff, 0xe0, 0xc0)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xf5, 0xe8, 0xda),
            Color::Rgb(0xb8, 0x9a, 0x80),
            Color::Rgb(0xff, 0xd0, 0xa0),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x1a, 0x12, 0x0b)
        ),
        semantic = (Color::Rgb(0xff, 0xd1, 0x66), Color::Rgb(0xff, 0x6b, 0x6b)),
        code = (
            Color::Rgb(0x24, 0x1a, 0x10),
            Color::Rgb(0x1f, 0x3a, 0x2a),
            Color::Rgb(0x3a, 0x16, 0x20)
        )
    ),
    theme!(
        "cyber",
        "赛博风",
        "Cyberpunk",
        bg = (
            Color::Rgb(0x0a, 0x00, 0x14),
            Color::Rgb(0x12, 0x00, 0x22),
            Color::Rgb(0x16, 0x00, 0x2a),
            Color::Rgb(0x2a, 0x0a, 0x4a)
        ),
        accent = (
            Color::Rgb(0x00, 0xf0, 0xff),
            Color::Rgb(0xff, 0x2b, 0xd6),
            Color::Rgb(0x8a, 0x5c, 0xff),
            Color::Rgb(0x4a, 0x2a, 0x80),
            Color::Rgb(0x2a, 0x0a, 0x4a),
            Color::Rgb(0xff, 0x9e, 0xf0)
        ),
        text = (
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0xe6, 0xdc, 0xff),
            Color::Rgb(0x8a, 0x7a, 0xaa),
            Color::Rgb(0x9e, 0xf8, 0xff),
            Color::Rgb(0xff, 0xff, 0xff),
            Color::Rgb(0x05, 0x00, 0x0a)
        ),
        semantic = (Color::Rgb(0xff, 0xe6, 0x00), Color::Rgb(0xff, 0x38, 0x60)),
        code = (
            Color::Rgb(0x12, 0x00, 0x22),
            Color::Rgb(0x0a, 0x2a, 0x2a),
            Color::Rgb(0x2a, 0x0a, 0x20)
        )
    ),
    theme!(
        "wasteland",
        "末日废土",
        "Wasteland",
        bg = (
            Color::Rgb(0x14, 0x11, 0x0c),
            Color::Rgb(0x1e, 0x19, 0x10),
            Color::Rgb(0x24, 0x1e, 0x12),
            Color::Rgb(0x3a, 0x30, 0x18)
        ),
        accent = (
            Color::Rgb(0xc9, 0xa2, 0x27),
            Color::Rgb(0xe8, 0xc7, 0x66),
            Color::Rgb(0x9a, 0x7a, 0x33),
            Color::Rgb(0x5f, 0x4d, 0x1f),
            Color::Rgb(0x3a, 0x30, 0x18),
            Color::Rgb(0xf0, 0xdf, 0x9a)
        ),
        text = (
            Color::Rgb(0xff, 0xf5, 0xdc),
            Color::Rgb(0xea, 0xdf, 0xc8),
            Color::Rgb(0x9a, 0x8a, 0x6a),
            Color::Rgb(0xe8, 0xd7, 0x9a),
            Color::Rgb(0xff, 0xf5, 0xdc),
            Color::Rgb(0x14, 0x11, 0x0c)
        ),
        semantic = (Color::Rgb(0xe0, 0xa1, 0x00), Color::Rgb(0xd9, 0x5d, 0x39)),
        code = (
            Color::Rgb(0x1e, 0x19, 0x10),
            Color::Rgb(0x1f, 0x3a, 0x2a),
            Color::Rgb(0x3a, 0x1a, 0x14)
        )
    ),
];

impl Default for Theme {
    fn default() -> Self {
        THEMES[0]
    }
}

impl Theme {
    pub fn all() -> &'static [Theme] {
        THEMES
    }

    /// Resolve a public id or a human-facing alias. Unknown themes fail closed
    /// so a typo never silently changes the palette.
    pub fn from_id(value: &str) -> Option<Self> {
        let normalized = value.trim().to_ascii_lowercase().replace('_', "-");
        let index = match normalized.as_str() {
            "military" | "military-green" | "army" | "军旅绿" => 0,
            "matrix" | "matrix-green" | "矩阵绿" => 1,
            "mono" | "monochrome" | "grey" | "gray" | "黑白灰" => 2,
            "ocean" | "deep-blue" | "deepblue" | "深蓝" => 3,
            "sakura" | "pink" | "樱花粉" => 4,
            "copper" | "copper-orange" | "orange" | "橙铜" | "爱马仕橙" => 5,
            "cyber" | "cyberpunk" | "赛博" | "赛博风" => 6,
            "wasteland" | "post-apocalyptic" | "末日废土" => 7,
            _ => return None,
        };
        THEMES.get(index).copied()
    }

    pub fn paint(&self, buffer: &mut Buffer) {
        for cell in &mut buffer.content {
            cell.fg = self.recolor(cell.fg);
            cell.bg = self.recolor(cell.bg);
        }
    }

    fn recolor(&self, color: Color) -> Color {
        match color {
            Color::Rgb(0x06, 0x10, 0x0a) => self.app_bg,
            Color::Rgb(0x08, 0x14, 0x0d) => self.panel_bg,
            Color::Rgb(0x0a, 0x1f, 0x12) => self.menu_bg,
            Color::Rgb(0x14, 0x3d, 0x24) => self.menu_selected_bg,
            Color::Rgb(0x70, 0x8a, 0x41) => self.primary,
            Color::Rgb(0x8a, 0xa6, 0x52) => self.primary_bright,
            Color::Rgb(0x8b, 0x96, 0x6f) => self.dim,
            Color::Rgb(0x68, 0x73, 0x58) => self.faint,
            Color::Rgb(0x4a, 0x59, 0x30) => self.rule,
            Color::Rgb(0xb7, 0xff, 0xc8) => self.link_hover,
            Color::Indexed(236) => self.code_bg,
            Color::Indexed(22) => self.code_added_bg,
            Color::Indexed(52) => self.code_removed_bg,
            Color::Indexed(245) => self.muted,
            Color::Indexed(250) => self.body,
            Color::Indexed(252) => self.code_text,
            Color::Indexed(255) => self.input_text,
            Color::White => self.white,
            Color::Yellow => self.warning,
            Color::LightRed => self.danger,
            Color::Black => self.on_bright,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_public_themes_have_unique_ids_and_aliases() {
        assert_eq!(Theme::all().len(), 8);
        let mut ids = std::collections::BTreeSet::new();
        for theme in Theme::all() {
            assert!(ids.insert(theme.id));
            assert!(!theme.name_zh.is_empty());
            assert!(!theme.name_en.is_empty());
            assert_eq!(Theme::from_id(theme.id), Some(*theme));
        }
        assert_eq!(Theme::from_id("军旅绿"), Some(Theme::default()));
        assert_eq!(
            Theme::from_id("爱马仕橙").map(|theme| theme.id),
            Some("copper")
        );
        assert!(Theme::from_id("not-a-theme").is_none());
    }

    #[test]
    fn military_paint_is_an_identity_mapping() {
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 2, 1));
        buffer.content[0].fg = Color::Rgb(0x70, 0x8a, 0x41);
        buffer.content[0].bg = Color::Rgb(0x06, 0x10, 0x0a);
        buffer.content[1].fg = Color::LightRed;
        buffer.content[1].bg = Color::Indexed(236);
        let before = buffer.content.clone();
        Theme::default().paint(&mut buffer);
        assert_eq!(buffer.content, before);
    }

    #[test]
    fn non_default_theme_remaps_base_colours() {
        let theme = Theme::from_id("matrix").unwrap();
        let mut buffer = Buffer::empty(ratatui::layout::Rect::new(0, 0, 1, 1));
        buffer.content[0].fg = Color::Rgb(0x70, 0x8a, 0x41);
        buffer.content[0].bg = Color::Rgb(0x06, 0x10, 0x0a);
        theme.paint(&mut buffer);
        assert_eq!(buffer.content[0].fg, theme.primary);
        assert_eq!(buffer.content[0].bg, theme.app_bg);
    }
}
