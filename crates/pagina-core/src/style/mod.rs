mod declarations;
mod ua;

use markup5ever_rcdom::{Handle, NodeData};

use crate::css::values::*;
use crate::css::{AncestorInfo, CssRule, Declaration, MatchTarget};
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
    build_styled_node(handle, rules, &ComputedStyle::default(), &[])
}

/// Context passed through the tree-building recursion.
struct StyleContext<'a> {
    rules: &'a [CssRule],
    parent_style: &'a ComputedStyle,
    ancestors: &'a [AncestorInfo],
}

fn build_styled_node(
    handle: &Handle,
    rules: &[CssRule],
    parent_style: &ComputedStyle,
    ancestors: &[AncestorInfo],
) -> Option<StyledNode> {
    let ctx = StyleContext { rules, parent_style, ancestors };
    match &handle.data {
        NodeData::Document => build_document_node(handle, &ctx),
        NodeData::Element { name, attrs, .. } => {
            let tag = name.local.as_ref().to_ascii_lowercase();
            let attrs_vec = collect_attrs(attrs);
            build_element_node(handle, &ctx, tag, attrs_vec)
        }
        _ => None,
    }
}

fn build_document_node(handle: &Handle, ctx: &StyleContext) -> Option<StyledNode> {
    let mut children = Vec::new();
    for child in handle.children.borrow().iter() {
        if let Some(node) = build_styled_node(child, ctx.rules, ctx.parent_style, ctx.ancestors) {
            children.push(StyledContent::Element(node));
            continue;
        }
        let NodeData::Text { contents } = &child.data else { continue };
        let text = contents.borrow().to_string();
        if !text.trim().is_empty() {
            children.push(StyledContent::Text(text));
        }
    }
    Some(StyledNode {
        tag: "#document".to_string(),
        id: None,
        classes: Vec::new(),
        style: ctx.parent_style.clone(),
        children,
        attrs: Vec::new(),
    })
}

fn collect_attrs(attrs: &std::cell::RefCell<Vec<markup5ever::Attribute>>) -> Vec<(String, String)> {
    attrs.borrow().iter()
        .map(|a| (a.name.local.as_ref().to_string(), a.value.to_string()))
        .collect()
}

fn find_attr<'a>(attrs: &'a [(String, String)], key: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn build_element_node(
    handle: &Handle,
    ctx: &StyleContext,
    tag: String,
    attrs_vec: Vec<(String, String)>,
) -> Option<StyledNode> {
    if matches!(tag.as_str(), "style" | "script" | "link" | "meta" | "title" | "head") {
        return None;
    }

    let id = find_attr(&attrs_vec, "id").map(String::from);
    let classes: Vec<String> = find_attr(&attrs_vec, "class")
        .map(|v| v.split_whitespace().map(String::from).collect())
        .unwrap_or_default();
    let inline_style_str = find_attr(&attrs_vec, "style").map(String::from);

    let target = MatchTarget { tag: &tag, id: &id, classes: &classes };
    let mut style = compute_element_style(&target, ctx);

    if let Some(inline_css) = inline_style_str {
        let decls = parser::parse_inline_style(&inline_css);
        declarations::apply_declarations(&mut style, &decls);
    }

    if style.display == Display::None {
        return None;
    }

    let child_ancestors = make_child_ancestors(&tag, &id, &classes, ctx.ancestors);
    let child_ctx = StyleContext { rules: ctx.rules, parent_style: &style, ancestors: &child_ancestors };
    let children = build_children(handle, &child_ctx, &tag);

    Some(StyledNode { tag, id, classes, style, children, attrs: attrs_vec })
}

fn compute_element_style(target: &MatchTarget, ctx: &StyleContext) -> ComputedStyle {
    let mut style = ua::ua_style(target.tag);
    inherit_from_parent(&mut style, ctx.parent_style);
    apply_matched_rules(&mut style, target, ctx);
    style
}

fn apply_matched_rules(style: &mut ComputedStyle, target: &MatchTarget, ctx: &StyleContext) {
    let mut matched: Vec<(u16, u16, u16, usize, &[Declaration])> = Vec::new();
    for (i, rule) in ctx.rules.iter().enumerate() {
        for sel in &rule.selectors {
            if sel.matches(target, ctx.ancestors) {
                let s = sel.specificity();
                matched.push((s.0, s.1, s.2, i, &rule.declarations));
            }
        }
    }
    matched.sort_by_key(|m| (m.0, m.1, m.2, m.3));
    for (_, _, _, _, decls) in &matched {
        declarations::apply_declarations(style, decls);
    }
}

fn inherit_from_parent(style: &mut ComputedStyle, parent: &ComputedStyle) {
    style.font_size_pt = if style.font_size_pt != 11.0 { style.font_size_pt } else { parent.font_size_pt };
    style.color = if matches!(style.color, Color { r: 0, g: 0, b: 0, a: 1.0, .. }) {
        parent.color
    } else {
        style.color
    };
    style.font_family = if style.font_family == "Helvetica" && parent.font_family != "Helvetica" {
        parent.font_family.clone()
    } else {
        std::mem::take(&mut style.font_family)
    };
    style.text_align = parent.text_align;
    style.line_height = parent.line_height;
}

fn make_child_ancestors(tag: &str, id: &Option<String>, classes: &[String], ancestors: &[AncestorInfo]) -> Vec<AncestorInfo> {
    let mut child_ancestors = vec![AncestorInfo {
        tag: tag.to_string(),
        id: id.clone(),
        classes: classes.to_vec(),
    }];
    child_ancestors.extend_from_slice(ancestors);
    child_ancestors
}

fn build_children(handle: &Handle, ctx: &StyleContext, parent_tag: &str) -> Vec<StyledContent> {
    let is_pre = parent_tag == "pre";
    let mut children = Vec::new();
    for child in handle.children.borrow().iter() {
        build_child(&child.data, child, ctx, is_pre, &mut children);
    }
    children
}

fn build_child(
    data: &NodeData,
    child: &Handle,
    ctx: &StyleContext,
    is_pre: bool,
    children: &mut Vec<StyledContent>,
) {
    match data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            if !text.trim().is_empty() || (is_pre && !text.is_empty()) {
                children.push(StyledContent::Text(text));
            }
        }
        NodeData::Element { .. } => {
            if let Some(node) = build_styled_node(child, ctx.rules, ctx.parent_style, ctx.ancestors) {
                children.push(StyledContent::Element(node));
            }
        }
        _ => {}
    }
}
