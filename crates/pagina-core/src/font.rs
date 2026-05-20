use std::collections::HashMap;

use printpdf::{BuiltinFont, FontId, PdfDocument, PdfFontHandle};

use crate::css::values::{FontStyle, FontWeight};

// ═══════════════════════════════════════════════════════════════
//  Font metrics
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct FontMetrics {
    char_widths: HashMap<char, u16>,
    units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    default_width: u16,
}

impl FontMetrics {
    pub fn text_width_mm(&self, text: &str, font_size_pt: f64) -> f64 {
        let scale = font_size_pt / self.units_per_em as f64 * 25.4 / 72.0;
        text.chars()
            .map(|c| self.char_widths.get(&c).copied().unwrap_or(self.default_width) as f64)
            .sum::<f64>()
            * scale
    }

    pub fn space_width_mm(&self, font_size_pt: f64) -> f64 {
        let w = self.char_widths.get(&' ').copied().unwrap_or(self.default_width);
        w as f64 * font_size_pt / self.units_per_em as f64 * 25.4 / 72.0
    }

    pub fn line_height_mm(&self, font_size_pt: f64, line_height_ratio: f64) -> f64 {
        font_size_pt * line_height_ratio * 25.4 / 72.0
    }
}

/// Parse font bytes with ttf-parser and extract glyph metrics.
fn parse_metrics(font_bytes: &[u8], font_index: u32) -> Option<FontMetrics> {
    let face = ttf_parser::Face::parse(font_bytes, font_index).ok()?;
    let units_per_em = face.units_per_em();

    let mut char_widths = HashMap::new();

    // Pre-cache ASCII + Latin-1 Supplement
    for codepoint in (0x20u32..=0x7E).chain(0xA0u32..=0xFF) {
        let Some(ch) = char::from_u32(codepoint) else { continue };
        let Some(gid) = face.glyph_index(ch) else { continue };
        let Some(advance) = face.glyph_hor_advance(gid) else { continue };
        char_widths.insert(ch, advance);
    }

    let default_width = char_widths.get(&' ').copied().unwrap_or(250);

    Some(FontMetrics {
        char_widths,
        units_per_em,
        ascender: face.ascender(),
        descender: face.descender(),
        default_width,
    })
}

/// Parse font and pre-cache widths for all characters in `text`.
fn cache_chars_for_text(metrics: &mut FontMetrics, font_bytes: &[u8], font_index: u32, text: &str) {
    let Ok(face) = ttf_parser::Face::parse(font_bytes, font_index) else {
        return;
    };
    for ch in text.chars() {
        if metrics.char_widths.contains_key(&ch) {
            continue;
        }
        let Some(gid) = face.glyph_index(ch) else { continue };
        let Some(advance) = face.glyph_hor_advance(gid) else { continue };
        metrics.char_widths.insert(ch, advance);
    }
}

// ═══════════════════════════════════════════════════════════════
//  Font resolution
// ═══════════════════════════════════════════════════════════════

/// Builtin font lookup table: (is_courier, weight, style) -> BuiltinFont.
const BUILTIN_FONT_TABLE: &[(bool, bool, bool, BuiltinFont)] = &[
    // (is_courier, is_bold, is_italic, font)
    (true,  true,  true,  BuiltinFont::CourierBoldOblique),
    (true,  true,  false, BuiltinFont::CourierBold),
    (true,  false, true,  BuiltinFont::CourierOblique),
    (true,  false, false, BuiltinFont::Courier),
    (false, true,  true,  BuiltinFont::HelveticaBoldOblique),
    (false, true,  false, BuiltinFont::HelveticaBold),
    (false, false, true,  BuiltinFont::HelveticaOblique),
    (false, false, false, BuiltinFont::Helvetica),
];

pub fn resolve_builtin(weight: FontWeight, style: FontStyle, family: &str) -> BuiltinFont {
    let f = family.to_ascii_lowercase();
    let is_courier = f.contains("courier") || f.contains("mono") || f.contains("monospace");
    let is_bold = weight == FontWeight::Bold;
    let is_italic = style == FontStyle::Italic;

    BUILTIN_FONT_TABLE.iter()
        .find(|(c, b, i, _)| *c == is_courier && *b == is_bold && *i == is_italic)
        .map(|(_, _, _, font)| *font)
        .unwrap_or(BuiltinFont::Helvetica)
}

// ═══════════════════════════════════════════════════════════════
//  Font manager
// ═══════════════════════════════════════════════════════════════

pub struct FontManager {
    builtin_metrics: HashMap<BuiltinFont, FontMetrics>,
    builtin_bytes: HashMap<BuiltinFont, Vec<u8>>,
    external_fonts: Vec<ExternalFont>,
}

pub struct ExternalFont {
    pub family_name: String,
    pub metrics: FontMetrics,
    pub parsed_font: printpdf::ParsedFont,
    pub font_bytes: Vec<u8>,
    pub font_id: Option<FontId>,
}

#[derive(Debug, Clone)]
pub enum ResolvedFont {
    Builtin(BuiltinFont),
    External(usize),
}

/// Parameters for measuring text.
pub struct MeasureParams<'a> {
    pub text: &'a str,
    pub family: &'a str,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub font_size_pt: f64,
}

impl FontManager {
    pub fn new() -> Self {
        let mut builtin_metrics = HashMap::new();
        let mut builtin_bytes = HashMap::new();

        for bf in BuiltinFont::all_ids() {
            let subset = bf.get_subset_font();
            if let Some(metrics) = parse_metrics(&subset.bytes, 0) {
                builtin_bytes.insert(bf, subset.bytes.clone());
                builtin_metrics.insert(bf, metrics);
            }
        }

        Self {
            builtin_metrics,
            builtin_bytes,
            external_fonts: Vec::new(),
        }
    }

    /// Load an external font from bytes.
    pub fn load_font(&mut self, font_bytes: Vec<u8>, family_name: &str) -> bool {
        let metrics = match parse_metrics(&font_bytes, 0) {
            Some(m) => m,
            None => return false,
        };

        let mut warnings = Vec::new();
        let parsed = match printpdf::ParsedFont::from_bytes(&font_bytes, 0, &mut warnings) {
            Some(p) => p,
            None => return false,
        };

        self.external_fonts.push(ExternalFont {
            family_name: family_name.to_string(),
            metrics,
            parsed_font: parsed,
            font_bytes,
            font_id: None,
        });
        true
    }

    /// Resolve a font family + weight + style to a specific font.
    pub fn resolve(&self, family: &str, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        for (i, ext) in self.external_fonts.iter().enumerate() {
            if ext.family_name.eq_ignore_ascii_case(family) {
                return ResolvedFont::External(i);
            }
        }
        ResolvedFont::Builtin(resolve_builtin(weight, style, family))
    }

    /// Get metrics for a resolved font.
    pub fn metrics(&self, font: &ResolvedFont) -> &FontMetrics {
        match font {
            ResolvedFont::Builtin(bf) => self.builtin_metrics.get(bf)
                .expect("builtin font should always have metrics"),
            ResolvedFont::External(i) => &self.external_fonts[*i].metrics,
        }
    }

    /// Measure text width in mm.
    pub fn measure_text(
        &self,
        text: &str,
        family: &str,
        weight: FontWeight,
        style: FontStyle,
        font_size_pt: f64,
    ) -> f64 {
        let resolved = self.resolve(family, weight, style);
        self.metrics(&resolved).text_width_mm(text, font_size_pt)
    }

    /// Get the PDF font handle for rendering.
    pub fn pdf_handle(&self, font: &ResolvedFont) -> PdfFontHandle {
        match font {
            ResolvedFont::Builtin(bf) => PdfFontHandle::Builtin(*bf),
            ResolvedFont::External(i) => {
                if let Some(id) = &self.external_fonts[*i].font_id {
                    PdfFontHandle::External(id.clone())
                } else {
                    PdfFontHandle::Builtin(BuiltinFont::Helvetica)
                }
            }
        }
    }

    /// Register external fonts with a PdfDocument. Call before rendering.
    pub fn register_with_document(&mut self, doc: &mut PdfDocument) {
        for ext in &mut self.external_fonts {
            let font_id = doc.add_font(&ext.parsed_font);
            ext.font_id = Some(font_id);
        }
    }

    /// Pre-cache glyph widths for all characters in the document text.
    pub fn cache_document_chars(&mut self, text: &str) {
        // Cache for all builtin fonts
        for (bf, metrics) in &mut self.builtin_metrics {
            if let Some(bytes) = self.builtin_bytes.get(bf) {
                cache_chars_for_text(metrics, bytes, 0, text);
            }
        }
        // Cache for external fonts
        for ext in &mut self.external_fonts {
            cache_chars_for_text(&mut ext.metrics, &ext.font_bytes, 0, text);
        }
    }
}
