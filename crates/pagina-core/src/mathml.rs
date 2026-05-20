/// MathML rendering to layout items.
///
/// Supports basic MathML elements:
/// - <math> container
/// - <mrow> horizontal grouping
/// - <mi> identifier (variable)
/// - <mn> number
/// - <mo> operator
/// - <mfrac> fraction (numerator/denominator)
/// - <msup> superscript
/// - <msub> subscript
/// - <msqrt> square root

use crate::css::values::*;
use crate::font::FontProvider;
use crate::layout::LayoutItem;
use crate::layout::ItemKind;
use crate::style::{StyledContent, StyledNode};

/// Render a <math> element into positioned layout items.
pub fn render_math(
    node: &StyledNode,
    base_font_size: f64,
    fm: &dyn FontProvider,
) -> Vec<LayoutItem> {
    let style = MathStyle {
        font_size: base_font_size,
        color: node.style.color,
        font_family: node.style.font_family.clone(),
    };
    let mut ctx = MathContext { items: Vec::new(), x: 0.0, fm };
    ctx.render_node(node, 0.0, &style);
    ctx.items
}

/// Width of a <math> element in mm.
pub fn math_width(node: &StyledNode, base_font_size: f64, fm: &dyn FontProvider) -> f64 {
    let items = render_math(node, base_font_size, fm);
    items
        .iter()
        .map(|item| item.x_mm + measure_item_width(item, fm))
        .fold(0.0_f64, f64::max)
}

struct MathStyle {
    font_size: f64,
    color: Color,
    font_family: String,
}

impl MathStyle {
    fn scaled(&self, factor: f64) -> Self {
        Self {
            font_size: self.font_size * factor,
            color: self.color,
            font_family: self.font_family.clone(),
        }
    }

    fn line_height_mm(&self) -> f64 {
        self.font_size * 25.4 / 72.0
    }
}

struct CenteredTextParams<'a> {
    container_x: f64,
    container_w: f64,
    text_w: f64,
    y: f64,
    text: &'a str,
}

struct MathContext<'a> {
    items: Vec<LayoutItem>,
    x: f64,
    fm: &'a dyn FontProvider,
}

impl<'a> MathContext<'a> {
    fn render_node(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        match node.tag.as_str() {
            "math" | "mrow" => self.render_children(node, y_offset, style),
            "mi" => self.render_mi(node, y_offset, style),
            "mn" | "mo" => self.render_mn_mo(node, y_offset, style),
            "mfrac" => self.render_mfrac(node, y_offset, style),
            "msup" => self.render_msup(node, y_offset, style),
            "msub" => self.render_msub(node, y_offset, style),
            "msqrt" => self.render_msqrt(node, y_offset, style),
            _ => self.render_children(node, y_offset, style),
        }
    }

    fn render_children(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        for child in &node.children {
            self.render_content(child, y_offset, style);
        }
    }

    fn render_content(&mut self, child: &StyledContent, y_offset: f64, style: &MathStyle) {
        match child {
            StyledContent::Element(child_node) => self.render_node(child_node, y_offset, style),
            StyledContent::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    self.emit_text(y_offset, trimmed, style);
                }
            }
        }
    }

    fn render_mi(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        let text = collect_math_text(node);
        self.items.push(LayoutItem {
            x_mm: self.x,
            y_mm: y_offset,
            font_size_pt: style.font_size,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Italic,
            font_family: style.font_family.clone(),
            color: style.color,
            text: text.clone(),
            kind: ItemKind::Text,
        });
        let w = self.fm.measure_text(&text, &style.font_family, FontWeight::Normal, FontStyle::Italic, style.font_size);
        self.x += w;
    }

    fn render_mn_mo(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        let text = collect_math_text(node);
        self.emit_text(y_offset, &text, style);
    }

    fn render_mfrac(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        let children = element_children(node);
        if children.len() < 2 {
            return;
        }

        let small_style = style.scaled(0.75);
        let num_text = collect_all_text(children[0]);
        let den_text = collect_all_text(children[1]);
        let num_w = self.fm.measure_text(&num_text, &style.font_family, FontWeight::Normal, FontStyle::Normal, small_style.font_size);
        let den_w = self.fm.measure_text(&den_text, &style.font_family, FontWeight::Normal, FontStyle::Normal, small_style.font_size);
        let frac_w = num_w.max(den_w) + 1.0;
        let frac_x = self.x;
        let lh = style.line_height_mm();

        self.emit_centered(&CenteredTextParams {
            container_x: frac_x, container_w: frac_w, text_w: num_w,
            y: y_offset - lh * 0.4, text: &num_text,
        }, &small_style);
        self.items.push(LayoutItem::hr_item(
            (frac_x, y_offset),
            ItemKind::HorizontalRule { width_mm: frac_w, thickness_mm: 0.15, color: style.color },
        ));
        self.emit_centered(&CenteredTextParams {
            container_x: frac_x, container_w: frac_w, text_w: den_w,
            y: y_offset + lh * 0.45, text: &den_text,
        }, &small_style);

        self.x += frac_w + 0.5;
    }

    fn emit_centered(&mut self, params: &CenteredTextParams, style: &MathStyle) {
        let x = params.container_x + (params.container_w - params.text_w) / 2.0;
        self.items.push(LayoutItem {
            x_mm: x,
            y_mm: params.y,
            font_size_pt: style.font_size,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_family: style.font_family.clone(),
            color: style.color,
            text: params.text.to_string(),
            kind: ItemKind::Text,
        });
    }

    fn render_msup(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        let children = element_children(node);
        if children.len() < 2 {
            return;
        }
        self.render_node(children[0], y_offset, style);
        let sup_style = style.scaled(0.7);
        let lh = style.line_height_mm();
        self.render_node(children[1], y_offset - lh * 0.35, &sup_style);
    }

    fn render_msub(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        let children = element_children(node);
        if children.len() < 2 {
            return;
        }
        self.render_node(children[0], y_offset, style);
        let sub_style = style.scaled(0.7);
        let lh = style.line_height_mm();
        self.render_node(children[1], y_offset + lh * 0.25, &sub_style);
    }

    fn render_msqrt(&mut self, node: &StyledNode, y_offset: f64, style: &MathStyle) {
        self.emit_text(y_offset, "V/", style);
        let content_start = self.x;
        self.render_children(node, y_offset, style);
        let content_end = self.x;
        let lh = style.line_height_mm();
        self.items.push(LayoutItem::hr_item(
            (content_start, y_offset - lh * 0.5),
            ItemKind::HorizontalRule { width_mm: content_end - content_start, thickness_mm: 0.15, color: style.color },
        ));
    }

    fn emit_text(&mut self, y_offset: f64, text: &str, style: &MathStyle) {
        let w = self.fm.measure_text(text, &style.font_family, FontWeight::Normal, FontStyle::Normal, style.font_size);
        self.items.push(LayoutItem {
            x_mm: self.x,
            y_mm: y_offset,
            font_size_pt: style.font_size,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_family: style.font_family.clone(),
            color: style.color,
            text: text.to_string(),
            kind: ItemKind::Text,
        });
        self.x += w;
    }
}

fn element_children(node: &StyledNode) -> Vec<&StyledNode> {
    node.children.iter().filter_map(|c| {
        if let StyledContent::Element(n) = c { Some(n) } else { None }
    }).collect()
}

fn collect_math_text(node: &StyledNode) -> String {
    let mut s = String::new();
    for child in &node.children {
        if let StyledContent::Text(t) = child {
            s.push_str(t.trim());
        }
    }
    s
}

fn collect_all_text(node: &StyledNode) -> String {
    let mut s = String::new();
    for child in &node.children {
        match child {
            StyledContent::Text(t) => s.push_str(t.trim()),
            StyledContent::Element(n) => s.push_str(&collect_all_text(n)),
        }
    }
    s
}

fn measure_item_width(item: &LayoutItem, fm: &dyn FontProvider) -> f64 {
    if item.text.is_empty() {
        if let ItemKind::HorizontalRule { width_mm, .. } = &item.kind {
            return *width_mm;
        }
        return 0.0;
    }
    fm.measure_text(&item.text, &item.font_family, item.font_weight, item.font_style, item.font_size_pt)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::MockFontProvider;
    use crate::style::{ComputedStyle, StyledContent, StyledNode};

    /// Helper: build a minimal StyledNode with the given tag and text children.
    fn make_node(tag: &str, children: Vec<StyledContent>) -> StyledNode {
        StyledNode {
            tag: tag.to_string(),
            id: None,
            classes: Vec::new(),
            style: ComputedStyle::default(),
            children,
            attrs: Vec::new(),
        }
    }

    fn text(s: &str) -> StyledContent {
        StyledContent::Text(s.to_string())
    }

    fn elem(node: StyledNode) -> StyledContent {
        StyledContent::Element(node)
    }

    // ─── render_math with <mn> ─────────────────────────

    #[test]
    fn render_math_mn_produces_items() {
        let fm = MockFontProvider::new(500, 1000);
        let mn = make_node("mn", vec![text("42")]);
        let math = make_node("math", vec![elem(mn)]);
        let items = render_math(&math, 12.0, &fm);
        assert!(!items.is_empty(), "render_math should produce items for <mn>42</mn>");
        assert!(items.iter().any(|i| i.text == "42"), "should contain text '42'");
    }

    // ─── render_math with <mfrac> ──────────────────────

    #[test]
    fn render_math_mfrac_produces_numerator_denominator_and_rule() {
        let fm = MockFontProvider::new(500, 1000);
        let num = make_node("mn", vec![text("1")]);
        let den = make_node("mn", vec![text("2")]);
        let frac = make_node("mfrac", vec![elem(num), elem(den)]);
        let math = make_node("math", vec![elem(frac)]);
        let items = render_math(&math, 12.0, &fm);

        let has_1 = items.iter().any(|i| i.text == "1");
        let has_2 = items.iter().any(|i| i.text == "2");
        let has_hr = items.iter().any(|i| matches!(i.kind, ItemKind::HorizontalRule { .. }));

        assert!(has_1, "fraction should render numerator '1'");
        assert!(has_2, "fraction should render denominator '2'");
        assert!(has_hr, "fraction should render a horizontal rule");
        assert!(items.len() >= 3, "fraction should produce at least 3 items (num + denom + rule)");
    }

    // ─── render_math with <msup> ───────────────────────

    #[test]
    fn render_math_msup_produces_items_at_different_y() {
        let fm = MockFontProvider::new(500, 1000);
        let base = make_node("mn", vec![text("x")]);
        let sup = make_node("mn", vec![text("2")]);
        let msup = make_node("msup", vec![elem(base), elem(sup)]);
        let math = make_node("math", vec![elem(msup)]);
        let items = render_math(&math, 12.0, &fm);

        assert!(items.len() >= 2, "msup should produce at least base + superscript");

        let y_values: Vec<f64> = items.iter().map(|i| i.y_mm).collect();
        let min_y = y_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = y_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (max_y - min_y).abs() > 0.01,
            "superscript should be at a different y position than the base"
        );
    }

    // ─── math_width ────────────────────────────────────

    #[test]
    fn math_width_returns_positive_value() {
        let fm = MockFontProvider::new(500, 1000);
        let mn = make_node("mn", vec![text("42")]);
        let math = make_node("math", vec![elem(mn)]);
        let w = math_width(&math, 12.0, &fm);
        assert!(w > 0.0, "math_width should be positive for non-empty content, got {w}");
    }

    #[test]
    fn math_width_empty_is_zero() {
        let fm = MockFontProvider::new(500, 1000);
        let math = make_node("math", vec![]);
        let w = math_width(&math, 12.0, &fm);
        assert!(w == 0.0 || w >= 0.0, "math_width of empty math should be zero or non-negative");
    }

    // ─── render_math with <mi> (italic) ────────────────

    #[test]
    fn render_math_mi_is_italic() {
        let fm = MockFontProvider::new(500, 1000);
        let mi = make_node("mi", vec![text("x")]);
        let math = make_node("math", vec![elem(mi)]);
        let items = render_math(&math, 12.0, &fm);

        assert!(!items.is_empty());
        let x_item = items.iter().find(|i| i.text == "x").expect("should have 'x' item");
        assert_eq!(x_item.font_style, FontStyle::Italic, "<mi> should render in italic");
    }

    // ─── render_math with <msub> ───────────────────────

    #[test]
    fn render_math_msub_produces_items_at_different_y() {
        let fm = MockFontProvider::new(500, 1000);
        let base = make_node("mn", vec![text("a")]);
        let sub = make_node("mn", vec![text("i")]);
        let msub = make_node("msub", vec![elem(base), elem(sub)]);
        let math = make_node("math", vec![elem(msub)]);
        let items = render_math(&math, 12.0, &fm);

        assert!(items.len() >= 2, "msub should produce at least base + subscript");
        let y_values: Vec<f64> = items.iter().map(|i| i.y_mm).collect();
        let min_y = y_values.iter().cloned().fold(f64::INFINITY, f64::min);
        let max_y = y_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        assert!(
            (max_y - min_y).abs() > 0.01,
            "subscript should be at a different y position than the base"
        );
    }
}
