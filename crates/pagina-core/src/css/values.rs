/// CSS length with unit.
#[derive(Debug, Clone, Copy)]
pub enum Length {
    Mm(f64),
    Cm(f64),
    In(f64),
    Pt(f64),
    Pc(f64),
    Px(f64),
    Em(f64),
    Percent(f64),
    Zero,
}

/// Conversion factor from a length unit to millimetres (em_base_pt = 1.0).
/// Em and Percent are handled separately in `to_mm`.
fn unit_to_mm_factor(value: f64, variant_index: u8, em_base_pt: f64) -> f64 {
    const FACTORS: [(f64, f64); 6] = [
        (1.0, 0.0),          // Mm:  v * 1.0
        (10.0, 0.0),         // Cm:  v * 10.0
        (25.4, 0.0),         // In:  v * 25.4
        (25.4, 72.0),        // Pt:  v * 25.4 / 72.0
        (25.4, 6.0),         // Pc:  v * 25.4 / 6.0
        (25.4, 96.0),        // Px:  v * 25.4 / 96.0
    ];
    let _ = em_base_pt;
    let (mul, div) = FACTORS[variant_index as usize];
    if div == 0.0 { value * mul } else { value * mul / div }
}

impl Length {
    /// Resolve to millimetres. `em_base_pt` is the current font size in pt.
    pub fn to_mm(self, em_base_pt: f64) -> f64 {
        match self {
            Length::Mm(v) => unit_to_mm_factor(v, 0, em_base_pt),
            Length::Cm(v) => unit_to_mm_factor(v, 1, em_base_pt),
            Length::In(v) => unit_to_mm_factor(v, 2, em_base_pt),
            Length::Pt(v) => unit_to_mm_factor(v, 3, em_base_pt),
            Length::Pc(v) => unit_to_mm_factor(v, 4, em_base_pt),
            Length::Px(v) => unit_to_mm_factor(v, 5, em_base_pt),
            Length::Em(v) => v * em_base_pt * 25.4 / 72.0,
            Length::Percent(v) => v / 100.0 * em_base_pt * 25.4 / 72.0,
            Length::Zero => 0.0,
        }
    }

    /// Resolve to points.
    pub fn to_pt(self, em_base_pt: f64) -> f64 {
        match self {
            Length::Pt(v) => v,
            Length::Em(v) => v * em_base_pt,
            Length::Percent(v) => v / 100.0 * em_base_pt,
            other => other.to_mm(em_base_pt) * 72.0 / 25.4,
        }
    }
}

/// CMYK color (values 0.0 - 1.0).
#[derive(Debug, Clone, Copy)]
pub struct CmykColor {
    pub c: f32,
    pub m: f32,
    pub y: f32,
    pub k: f32,
}

/// CSS color (RGB with optional CMYK for print output).
#[derive(Debug, Clone, Copy)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
    /// When set, PDF output uses CMYK instead of RGB.
    pub cmyk: Option<CmykColor>,
}

impl Color {
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 1.0, cmyk: None };
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 1.0, cmyk: None };
    pub const TRANSPARENT: Color = Color { r: 0, g: 0, b: 0, a: 0.0, cmyk: None };

    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 1.0, cmyk: None }
    }

    pub fn cmyk(c: f32, m: f32, y: f32, k: f32) -> Self {
        // Also store an approximate RGB fallback
        let r = (255.0 * (1.0 - c) * (1.0 - k)) as u8;
        let g = (255.0 * (1.0 - m) * (1.0 - k)) as u8;
        let b = (255.0 * (1.0 - y) * (1.0 - k)) as u8;
        Self { r, g, b, a: 1.0, cmyk: Some(CmykColor { c, m, y, k }) }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        COLOR_TABLE.iter()
            .find(|(n, _)| *n == lower)
            .map(|(_, c)| *c)
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b)
            }
            _ => return None,
        };
        Some(Self::rgb(r, g, b))
    }
}

/// Named color lookup table.
const COLOR_TABLE: &[(&str, Color)] = &[
    ("black", Color::BLACK),
    ("white", Color::WHITE),
    ("red", Color { r: 255, g: 0, b: 0, a: 1.0, cmyk: None }),
    ("green", Color { r: 0, g: 128, b: 0, a: 1.0, cmyk: None }),
    ("blue", Color { r: 0, g: 0, b: 255, a: 1.0, cmyk: None }),
    ("navy", Color { r: 0, g: 0, b: 128, a: 1.0, cmyk: None }),
    ("transparent", Color::TRANSPARENT),
    ("gray", Color { r: 128, g: 128, b: 128, a: 1.0, cmyk: None }),
    ("grey", Color { r: 128, g: 128, b: 128, a: 1.0, cmyk: None }),
    ("darkgray", Color { r: 169, g: 169, b: 169, a: 1.0, cmyk: None }),
    ("darkgrey", Color { r: 169, g: 169, b: 169, a: 1.0, cmyk: None }),
    ("lightgray", Color { r: 211, g: 211, b: 211, a: 1.0, cmyk: None }),
    ("lightgrey", Color { r: 211, g: 211, b: 211, a: 1.0, cmyk: None }),
    ("maroon", Color { r: 128, g: 0, b: 0, a: 1.0, cmyk: None }),
    ("orange", Color { r: 255, g: 165, b: 0, a: 1.0, cmyk: None }),
    ("purple", Color { r: 128, g: 0, b: 128, a: 1.0, cmyk: None }),
    ("teal", Color { r: 0, g: 128, b: 128, a: 1.0, cmyk: None }),
    ("silver", Color { r: 192, g: 192, b: 192, a: 1.0, cmyk: None }),
];

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontWeight {
    #[default]
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum FontStyle {
    #[default]
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BreakValue {
    #[default]
    Auto,
    Page,
    Avoid,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Display {
    #[default]
    Block,
    Inline,
    None,
    ListItem,
}

/// Items that can appear in the `content` property of margin boxes.
#[derive(Debug, Clone)]
pub enum ContentItem {
    String(String),
    Counter(String),
    Counters(String, String),
    TargetCounter(String, String),
    RunningString(String),
    Attr(String),
    None,
}

/// Position of a margin box within a page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarginBoxPosition {
    TopLeftCorner,
    TopLeft,
    TopCenter,
    TopRight,
    TopRightCorner,
    RightTop,
    RightMiddle,
    RightBottom,
    BottomRightCorner,
    BottomRight,
    BottomCenter,
    BottomLeft,
    BottomLeftCorner,
    LeftBottom,
    LeftMiddle,
    LeftTop,
}

/// Lookup table for margin box position names.
const MARGIN_BOX_TABLE: &[(&str, MarginBoxPosition)] = &[
    ("top-left-corner", MarginBoxPosition::TopLeftCorner),
    ("top-left", MarginBoxPosition::TopLeft),
    ("top-center", MarginBoxPosition::TopCenter),
    ("top-right", MarginBoxPosition::TopRight),
    ("top-right-corner", MarginBoxPosition::TopRightCorner),
    ("bottom-right-corner", MarginBoxPosition::BottomRightCorner),
    ("bottom-right", MarginBoxPosition::BottomRight),
    ("bottom-center", MarginBoxPosition::BottomCenter),
    ("bottom-left", MarginBoxPosition::BottomLeft),
    ("bottom-left-corner", MarginBoxPosition::BottomLeftCorner),
    ("right-top", MarginBoxPosition::RightTop),
    ("right-middle", MarginBoxPosition::RightMiddle),
    ("right-bottom", MarginBoxPosition::RightBottom),
    ("left-bottom", MarginBoxPosition::LeftBottom),
    ("left-middle", MarginBoxPosition::LeftMiddle),
    ("left-top", MarginBoxPosition::LeftTop),
];

impl MarginBoxPosition {
    pub fn from_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        MARGIN_BOX_TABLE.iter()
            .find(|(n, _)| *n == lower)
            .map(|(_, pos)| *pos)
    }

    pub fn is_top(&self) -> bool {
        matches!(
            self,
            Self::TopLeftCorner | Self::TopLeft | Self::TopCenter | Self::TopRight | Self::TopRightCorner
        )
    }

    pub fn is_bottom(&self) -> bool {
        matches!(
            self,
            Self::BottomLeftCorner | Self::BottomLeft | Self::BottomCenter | Self::BottomRight | Self::BottomRightCorner
        )
    }
}
