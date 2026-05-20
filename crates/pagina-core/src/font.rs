use std::collections::HashMap;

use printpdf::{BuiltinFont, FontId, PdfDocument, PdfFontHandle};

use crate::css::values::{FontStyle, FontWeight};

// ═══════════════════════════════════════════════════════════════
//  Font provider trait (DI boundary)
// ═══════════════════════════════════════════════════════════════

/// Trait for font measurement. Implement this to swap font backends in tests.
pub trait FontProvider {
    fn resolve(&self, family: &str, weight: FontWeight, style: FontStyle) -> ResolvedFont;
    fn resolve_default(&self, weight: FontWeight, style: FontStyle) -> ResolvedFont;
    fn metrics(&self, font: &ResolvedFont) -> &FontMetrics;
    fn measure_text(&self, text: &str, family: &str, weight: FontWeight, style: FontStyle, size_pt: f64) -> f64;
    fn pdf_handle(&self, font: &ResolvedFont) -> PdfFontHandle;
}

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
        // Exact match by family name
        for (i, ext) in self.external_fonts.iter().enumerate() {
            if ext.family_name.eq_ignore_ascii_case(family) {
                return ResolvedFont::External(i);
            }
        }
        // Partial match (family name contains the requested name, or vice versa)
        for (i, ext) in self.external_fonts.iter().enumerate() {
            let ext_lower = ext.family_name.to_ascii_lowercase();
            let fam_lower = family.to_ascii_lowercase();
            if ext_lower.contains(&fam_lower) || fam_lower.contains(&ext_lower) {
                return ResolvedFont::External(i);
            }
        }
        ResolvedFont::Builtin(resolve_builtin(weight, style, family))
    }

    /// Resolve to the best available font (first external if any, else builtin).
    pub fn resolve_default(&self, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        if !self.external_fonts.is_empty() {
            return ResolvedFont::External(0);
        }
        ResolvedFont::Builtin(resolve_builtin(weight, style, "Helvetica"))
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

impl FontProvider for FontManager {
    fn resolve(&self, family: &str, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        self.resolve(family, weight, style)
    }
    fn resolve_default(&self, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        self.resolve_default(weight, style)
    }
    fn metrics(&self, font: &ResolvedFont) -> &FontMetrics {
        self.metrics(font)
    }
    fn measure_text(&self, text: &str, family: &str, weight: FontWeight, style: FontStyle, size_pt: f64) -> f64 {
        self.measure_text(text, family, weight, style, size_pt)
    }
    fn pdf_handle(&self, font: &ResolvedFont) -> PdfFontHandle {
        self.pdf_handle(font)
    }
}

/// Mock font provider for testing. Fixed-width characters.
pub struct MockFontProvider {
    pub char_width_units: u16,
    pub units_per_em: u16,
    metrics: FontMetrics,
}

impl MockFontProvider {
    /// Create a mock where every character has the same width.
    pub fn new(char_width_units: u16, units_per_em: u16) -> Self {
        let mut widths = HashMap::new();
        // Pre-fill ASCII range
        for c in ' '..='~' {
            widths.insert(c, char_width_units);
        }
        Self {
            char_width_units,
            units_per_em,
            metrics: FontMetrics {
                char_widths: widths,
                units_per_em,
                ascender: (units_per_em as i16 * 8) / 10,
                descender: -((units_per_em as i16 * 2) / 10),
                default_width: char_width_units,
            },
        }
    }
}

impl FontProvider for MockFontProvider {
    fn resolve(&self, _family: &str, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        ResolvedFont::Builtin(resolve_builtin(weight, style, "Helvetica"))
    }
    fn resolve_default(&self, weight: FontWeight, style: FontStyle) -> ResolvedFont {
        self.resolve("Helvetica", weight, style)
    }
    fn metrics(&self, _font: &ResolvedFont) -> &FontMetrics {
        &self.metrics
    }
    fn measure_text(&self, text: &str, _family: &str, _weight: FontWeight, _style: FontStyle, size_pt: f64) -> f64 {
        self.metrics.text_width_mm(text, size_pt)
    }
    fn pdf_handle(&self, _font: &ResolvedFont) -> PdfFontHandle {
        PdfFontHandle::Builtin(BuiltinFont::Helvetica)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::values::{FontStyle, FontWeight};

    // ── FontMetrics basic construction ──────────────────────────

    fn sample_metrics() -> FontMetrics {
        let mut widths = HashMap::new();
        widths.insert('A', 600);
        widths.insert('B', 600);
        widths.insert(' ', 300);
        FontMetrics {
            char_widths: widths,
            units_per_em: 1000,
            ascender: 800,
            descender: -200,
            default_width: 500,
        }
    }

    // ── text_width_mm ───────────────────────────────────────────

    #[test]
    fn text_width_single_char() {
        let m = sample_metrics();
        // 'A' width = 600, scale = 12.0 / 1000 * 25.4 / 72.0
        let w = m.text_width_mm("A", 12.0);
        let expected = 600.0 * 12.0 / 1000.0 * 25.4 / 72.0;
        assert!((w - expected).abs() < 1e-9);
    }

    #[test]
    fn text_width_multiple_chars() {
        let m = sample_metrics();
        let w = m.text_width_mm("AB", 12.0);
        let expected = (600.0 + 600.0) * 12.0 / 1000.0 * 25.4 / 72.0;
        assert!((w - expected).abs() < 1e-9);
    }

    #[test]
    fn text_width_unknown_char_uses_default() {
        let m = sample_metrics();
        // 'Z' is not in char_widths, should use default_width=500
        let w = m.text_width_mm("Z", 12.0);
        let expected = 500.0 * 12.0 / 1000.0 * 25.4 / 72.0;
        assert!((w - expected).abs() < 1e-9);
    }

    #[test]
    fn text_width_empty_string_is_zero() {
        let m = sample_metrics();
        assert_eq!(m.text_width_mm("", 12.0), 0.0);
    }

    #[test]
    fn text_width_scales_with_font_size() {
        let m = sample_metrics();
        let w12 = m.text_width_mm("A", 12.0);
        let w24 = m.text_width_mm("A", 24.0);
        assert!((w24 - w12 * 2.0).abs() < 1e-9);
    }

    // ── space_width_mm ──────────────────────────────────────────

    #[test]
    fn space_width_uses_space_char() {
        let m = sample_metrics();
        let w = m.space_width_mm(12.0);
        let expected = 300.0 * 12.0 / 1000.0 * 25.4 / 72.0;
        assert!((w - expected).abs() < 1e-9);
    }

    #[test]
    fn space_width_no_space_char_uses_default() {
        let m = FontMetrics {
            char_widths: HashMap::new(),
            units_per_em: 1000,
            ascender: 800,
            descender: -200,
            default_width: 400,
        };
        let w = m.space_width_mm(10.0);
        let expected = 400.0 * 10.0 / 1000.0 * 25.4 / 72.0;
        assert!((w - expected).abs() < 1e-9);
    }

    // ── line_height_mm ──────────────────────────────────────────

    #[test]
    fn line_height_calculation() {
        let m = sample_metrics();
        let lh = m.line_height_mm(12.0, 1.5);
        let expected = 12.0 * 1.5 * 25.4 / 72.0;
        assert!((lh - expected).abs() < 1e-9);
    }

    #[test]
    fn line_height_ratio_one() {
        let m = sample_metrics();
        let lh = m.line_height_mm(10.0, 1.0);
        let expected = 10.0 * 25.4 / 72.0;
        assert!((lh - expected).abs() < 1e-9);
    }

    // ── resolve_builtin ─────────────────────────────────────────

    #[test]
    fn resolve_builtin_helvetica_normal() {
        let bf = resolve_builtin(FontWeight::Normal, FontStyle::Normal, "Helvetica");
        assert_eq!(bf, BuiltinFont::Helvetica);
    }

    #[test]
    fn resolve_builtin_helvetica_bold() {
        let bf = resolve_builtin(FontWeight::Bold, FontStyle::Normal, "Helvetica");
        assert_eq!(bf, BuiltinFont::HelveticaBold);
    }

    #[test]
    fn resolve_builtin_helvetica_italic() {
        let bf = resolve_builtin(FontWeight::Normal, FontStyle::Italic, "Helvetica");
        assert_eq!(bf, BuiltinFont::HelveticaOblique);
    }

    #[test]
    fn resolve_builtin_helvetica_bold_italic() {
        let bf = resolve_builtin(FontWeight::Bold, FontStyle::Italic, "Helvetica");
        assert_eq!(bf, BuiltinFont::HelveticaBoldOblique);
    }

    #[test]
    fn resolve_builtin_courier_normal() {
        let bf = resolve_builtin(FontWeight::Normal, FontStyle::Normal, "Courier");
        assert_eq!(bf, BuiltinFont::Courier);
    }

    #[test]
    fn resolve_builtin_courier_bold() {
        let bf = resolve_builtin(FontWeight::Bold, FontStyle::Normal, "Courier");
        assert_eq!(bf, BuiltinFont::CourierBold);
    }

    #[test]
    fn resolve_builtin_monospace_maps_to_courier() {
        let bf = resolve_builtin(FontWeight::Normal, FontStyle::Normal, "monospace");
        assert_eq!(bf, BuiltinFont::Courier);
    }

    #[test]
    fn resolve_builtin_unknown_family_defaults_to_helvetica() {
        let bf = resolve_builtin(FontWeight::Normal, FontStyle::Normal, "UnknownFont");
        assert_eq!(bf, BuiltinFont::Helvetica);
    }

    // ── FontManager ─────────────────────────────────────────────

    #[test]
    fn font_manager_new_has_builtin_metrics() {
        let fm = FontManager::new();
        let resolved = fm.resolve("Helvetica", FontWeight::Normal, FontStyle::Normal);
        // Should not panic
        let _metrics = fm.metrics(&resolved);
    }

    #[test]
    fn font_manager_resolve_unknown_falls_back_to_builtin() {
        let fm = FontManager::new();
        let resolved = fm.resolve("NonExistentFont", FontWeight::Normal, FontStyle::Normal);
        assert!(matches!(resolved, ResolvedFont::Builtin(_)));
    }

    #[test]
    fn font_manager_resolve_default_no_externals() {
        let fm = FontManager::new();
        let resolved = fm.resolve_default(FontWeight::Normal, FontStyle::Normal);
        assert!(matches!(resolved, ResolvedFont::Builtin(BuiltinFont::Helvetica)));
    }

    #[test]
    fn font_manager_measure_text_positive() {
        let fm = FontManager::new();
        let w = fm.measure_text("Hello", "Helvetica", FontWeight::Normal, FontStyle::Normal, 12.0);
        assert!(w > 0.0);
    }

    #[test]
    fn font_manager_measure_empty_text_is_zero() {
        let fm = FontManager::new();
        let w = fm.measure_text("", "Helvetica", FontWeight::Normal, FontStyle::Normal, 12.0);
        assert_eq!(w, 0.0);
    }

    #[test]
    fn font_manager_pdf_handle_builtin() {
        let fm = FontManager::new();
        let resolved = fm.resolve("Helvetica", FontWeight::Normal, FontStyle::Normal);
        let handle = fm.pdf_handle(&resolved);
        assert!(matches!(handle, PdfFontHandle::Builtin(BuiltinFont::Helvetica)));
    }
}
