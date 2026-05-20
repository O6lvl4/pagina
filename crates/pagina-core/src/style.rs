use markup5ever_rcdom::{Handle, NodeData};

use crate::css::values::*;
use crate::css::{CssRule, Declaration, Selector};
use crate::css::parser;

// ═══════════════════════════════════════════════════════════════
//  Computed style
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub font_size_pt: f64,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub color: Color,
    pub text_align: TextAlign,
    pub line_height: f64,
    pub margin_top_mm: f64,
    pub margin_bottom_mm: f64,
    pub padding_top_mm: f64,
    pub padding_bottom_mm: f64,
    pub break_before: BreakValue,
    pub break_after: BreakValue,
    pub border_bottom_width_mm: f64,
    pub border_bottom_color: Color,
    // CSS Paged Media
    pub string_set: Option<(String, StringSetSource)>,
    pub is_footnote: bool,
    /// CSS `content` property for generated content (e.g. target-counter in TOC links).
    pub content: Option<Vec<ContentItem>>,
}

#[derive(Debug, Clone)]
pub enum StringSetSource {
    Content,
    Attr(String),
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display: Display::Block,
            font_size_pt: 11.0,
            font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal,
            font_family: "Helvetica".to_string(),
            color: Color::BLACK,
            text_align: TextAlign::Left,
            line_height: 1.4,
            margin_top_mm: 0.0,
            margin_bottom_mm: 0.0,
            padding_top_mm: 0.0,
            padding_bottom_mm: 0.0,
            break_before: BreakValue::Auto,
            break_after: BreakValue::Auto,
            border_bottom_width_mm: 0.0,
            border_bottom_color: Color::BLACK,
            string_set: None,
            is_footnote: false,
            content: None,
        }
    }
}

/// UA defaults for common HTML tags.
fn ua_style(tag: &str) -> ComputedStyle {
    let mut s = ComputedStyle::default();
    match tag {
        "h1" => {
            s.font_size_pt = 26.0;
            s.font_weight = FontWeight::Bold;
            s.margin_top_mm = 6.0;
            s.margin_bottom_mm = 4.0;
        }
        "h2" => {
            s.font_size_pt = 20.0;
            s.font_weight = FontWeight::Bold;
            s.margin_top_mm = 5.0;
            s.margin_bottom_mm = 3.0;
        }
        "h3" => {
            s.font_size_pt = 16.0;
            s.font_weight = FontWeight::Bold;
            s.margin_top_mm = 4.0;
            s.margin_bottom_mm = 2.5;
        }
        "h4" => {
            s.font_size_pt = 14.0;
            s.font_weight = FontWeight::Bold;
            s.margin_top_mm = 3.0;
            s.margin_bottom_mm = 2.0;
        }
        "h5" | "h6" => {
            s.font_size_pt = 12.0;
            s.font_weight = FontWeight::Bold;
            s.margin_top_mm = 2.5;
            s.margin_bottom_mm = 1.5;
        }
        "p" => {
            s.margin_bottom_mm = 3.5;
        }
        "blockquote" => {
            s.margin_top_mm = 3.0;
            s.margin_bottom_mm = 3.0;
            s.padding_top_mm = 0.0;
            s.padding_bottom_mm = 0.0;
        }
        "pre" => {
            s.font_family = "Courier".to_string();
            s.font_size_pt = 10.0;
            s.margin_top_mm = 2.0;
            s.margin_bottom_mm = 2.0;
        }
        "code" | "kbd" | "samp" => {
            s.font_family = "Courier".to_string();
            s.display = Display::Inline;
        }
        "strong" | "b" => {
            s.font_weight = FontWeight::Bold;
            s.display = Display::Inline;
        }
        "em" | "i" => {
            s.font_style = FontStyle::Italic;
            s.display = Display::Inline;
        }
        "li" => {
            s.display = Display::ListItem;
            s.margin_bottom_mm = 1.5;
        }
        "hr" => {
            s.margin_top_mm = 4.0;
            s.margin_bottom_mm = 4.0;
            s.border_bottom_width_mm = 0.2;
        }
        "span" | "a" | "abbr" | "small" | "sub" | "sup" => {
            s.display = Display::Inline;
        }
        "table" => {
            s.margin_top_mm = 3.0;
            s.margin_bottom_mm = 3.0;
        }
        "td" => {
            s.padding_top_mm = 1.0;
            s.padding_bottom_mm = 1.0;
        }
        "th" => {
            s.padding_top_mm = 1.0;
            s.padding_bottom_mm = 1.0;
            s.font_weight = FontWeight::Bold;
        }
        _ => {}
    }
    s
}

// ═══════════════════════════════════════════════════════════════
//  Styled tree
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct StyledNode {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub style: ComputedStyle,
    pub children: Vec<StyledContent>,
    pub attrs: Vec<(String, String)>,
}

#[derive(Debug)]
pub enum StyledContent {
    Element(StyledNode),
    Text(String),
}

/// Build a styled tree from a DOM handle + CSS rules.
pub fn build_styled_tree(handle: &Handle, rules: &[CssRule]) -> Option<StyledNode> {
    build_styled_node(handle, rules, &ComputedStyle::default())
}

fn build_styled_node(
    handle: &Handle,
    rules: &[CssRule],
    parent_style: &ComputedStyle,
) -> Option<StyledNode> {
    match &handle.data {
        NodeData::Document => {
            let mut children = Vec::new();
            for child in handle.children.borrow().iter() {
                if let Some(node) = build_styled_node(child, rules, parent_style) {
                    children.push(StyledContent::Element(node));
                } else if let NodeData::Text { contents } = &child.data {
                    let text = contents.borrow().to_string();
                    if !text.trim().is_empty() {
                        children.push(StyledContent::Text(text));
                    }
                }
            }
            Some(StyledNode {
                tag: "#document".to_string(),
                id: None,
                classes: Vec::new(),
                style: parent_style.clone(),
                children,
                attrs: Vec::new(),
            })
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref().to_ascii_lowercase();

            // Skip non-content elements
            if matches!(tag.as_str(), "style" | "script" | "link" | "meta" | "title") {
                return None;
            }

            let attrs_vec: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.as_ref().to_string(), a.value.to_string()))
                .collect();

            let id = attrs_vec.iter().find(|(k, _)| k == "id").map(|(_, v)| v.clone());
            let classes: Vec<String> = attrs_vec
                .iter()
                .find(|(k, _)| k == "class")
                .map(|(_, v)| v.split_whitespace().map(String::from).collect())
                .unwrap_or_default();
            let inline_style_str = attrs_vec
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| v.as_str());

            // Compute style: UA defaults → CSS rules → inline style → inheritance
            let mut style = ua_style(&tag);

            // Inherit from parent
            style.font_size_pt = inherit_font_size(&style, parent_style);
            style.color = if matches!(style.color, Color { r: 0, g: 0, b: 0, a: 1.0, .. }) {
                parent_style.color
            } else {
                style.color
            };
            style.font_family = if style.font_family == "Helvetica" && parent_style.font_family != "Helvetica" {
                parent_style.font_family.clone()
            } else {
                style.font_family
            };
            style.text_align = parent_style.text_align; // text-align inherits
            style.line_height = parent_style.line_height; // line-height inherits

            // Apply CSS rules (sorted by specificity)
            let mut matched: Vec<(u16, u16, u16, usize, &[Declaration])> = Vec::new();
            for (i, rule) in rules.iter().enumerate() {
                for sel in &rule.selectors {
                    if selector_matches(sel, &tag, &id, &classes) {
                        matched.push((sel.specificity().0, sel.specificity().1, sel.specificity().2, i, &rule.declarations));
                    }
                }
            }
            matched.sort_by_key(|m| (m.0, m.1, m.2, m.3));
            for (_, _, _, _, decls) in &matched {
                apply_declarations(&mut style, decls);
            }

            // Apply inline style
            if let Some(inline_css) = inline_style_str {
                let decls = parser::parse_inline_style(inline_css);
                apply_declarations(&mut style, &decls);
            }

            // Skip display:none
            if style.display == Display::None {
                return None;
            }

            // Head element: skip entirely
            if tag == "head" {
                return None;
            }

            // Build children
            let mut children = Vec::new();
            for child in handle.children.borrow().iter() {
                match &child.data {
                    NodeData::Text { contents } => {
                        let text = contents.borrow().to_string();
                        if !text.trim().is_empty() || (tag == "pre" && !text.is_empty()) {
                            children.push(StyledContent::Text(text));
                        }
                    }
                    NodeData::Element { .. } => {
                        if let Some(child_node) = build_styled_node(child, rules, &style) {
                            children.push(StyledContent::Element(child_node));
                        }
                    }
                    _ => {}
                }
            }

            Some(StyledNode {
                tag,
                id,
                classes,
                style,
                children,
                attrs: attrs_vec,
            })
        }
        _ => None,
    }
}

fn inherit_font_size(element: &ComputedStyle, parent: &ComputedStyle) -> f64 {
    // If the element has a specific font size from UA, keep it.
    // Otherwise inherit from parent.
    // UA sets specific sizes for h1-h6; others get the default 11pt.
    if element.font_size_pt != 11.0 {
        element.font_size_pt
    } else {
        parent.font_size_pt
    }
}

fn selector_matches(
    sel: &Selector,
    tag: &str,
    id: &Option<String>,
    classes: &[String],
) -> bool {
    match sel {
        Selector::Universal => true,
        Selector::Type(t) => t == tag,
        Selector::Class(c) => classes.iter().any(|cl| cl == c),
        Selector::Id(i) => id.as_deref() == Some(i.as_str()),
        Selector::TypeAndClass(t, c) => t == tag && classes.iter().any(|cl| cl == c),
    }
}

fn apply_declarations(style: &mut ComputedStyle, decls: &[Declaration]) {
    let base_pt = style.font_size_pt;
    for decl in decls {
        match decl.property.as_str() {
            "font-size" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.font_size_pt = len.to_pt(base_pt);
                }
            }
            "font-weight" => {
                if let Some(fw) = parser::parse_font_weight_value(&decl.value) {
                    style.font_weight = fw;
                }
            }
            "font-style" => {
                if let Some(fs) = parser::parse_font_style_value(&decl.value) {
                    style.font_style = fs;
                }
            }
            "color" => {
                if let Some(c) = parser::parse_color_value(&decl.value) {
                    style.color = c;
                }
            }
            "text-align" => {
                if let Some(ta) = parser::parse_text_align_value(&decl.value) {
                    style.text_align = ta;
                }
            }
            "line-height" => {
                if let Ok(v) = decl.value.trim().parse::<f64>() {
                    style.line_height = v;
                } else if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.line_height = len.to_pt(base_pt) / base_pt;
                }
            }
            "margin" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    let mm = len.to_mm(base_pt);
                    style.margin_top_mm = mm;
                    style.margin_bottom_mm = mm;
                }
            }
            "margin-top" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.margin_top_mm = len.to_mm(base_pt);
                }
            }
            "margin-bottom" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.margin_bottom_mm = len.to_mm(base_pt);
                }
            }
            "padding-top" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.padding_top_mm = len.to_mm(base_pt);
                }
            }
            "padding-bottom" => {
                if let Some(len) = parser::parse_length_value(&decl.value) {
                    style.padding_bottom_mm = len.to_mm(base_pt);
                }
            }
            "break-before" | "page-break-before" => {
                if let Some(bv) = parser::parse_break_value(&decl.value) {
                    style.break_before = bv;
                }
            }
            "break-after" | "page-break-after" => {
                if let Some(bv) = parser::parse_break_value(&decl.value) {
                    style.break_after = bv;
                }
            }
            "display" => {
                if let Some(d) = parser::parse_display_value(&decl.value) {
                    style.display = d;
                }
            }
            "string-set" => {
                // string-set: chapter-title content()
                let parts: Vec<&str> = decl.value.split_whitespace().collect();
                if parts.len() >= 2 {
                    let name = parts[0].to_string();
                    let source = if parts[1].starts_with("content") {
                        StringSetSource::Content
                    } else if parts[1].starts_with("attr(") {
                        let attr = parts[1]
                            .strip_prefix("attr(")
                            .and_then(|s| s.strip_suffix(')'))
                            .unwrap_or("title")
                            .to_string();
                        StringSetSource::Attr(attr)
                    } else {
                        StringSetSource::Content
                    };
                    style.string_set = Some((name, source));
                }
            }
            "float" => {
                if decl.value.trim() == "footnote" {
                    style.is_footnote = true;
                }
            }
            "content" => {
                let items = parser::parse_content_value(&decl.value);
                if !items.is_empty() {
                    style.content = Some(items);
                }
            }
            "border-bottom" => {
                // Simplified: "1px solid black"
                let parts: Vec<&str> = decl.value.split_whitespace().collect();
                if let Some(first) = parts.first() {
                    if let Some(len) = parser::parse_length_value(first) {
                        style.border_bottom_width_mm = len.to_mm(base_pt);
                    }
                }
                if let Some(color_str) = parts.last() {
                    if let Some(c) = parser::parse_color_value(color_str) {
                        style.border_bottom_color = c;
                    }
                }
            }
            _ => {}
        }
    }
}
