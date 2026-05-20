//! Content resolution: margin boxes, running strings, target counters.

use std::collections::HashMap;

use crate::css::values::*;
use crate::css::PageStyle;
use crate::style::{StyledContent, StyledNode};

use super::ResolvedMarginBox;

// ═══════════════════════════════════════════════════════════════
//  Content resolution
// ═══════════════════════════════════════════════════════════════

pub(super) struct ContentResolveContext<'a> {
    pub(super) page_num: usize,
    pub(super) total_pages: usize,
    pub(super) running_strings: &'a HashMap<String, String>,
}

pub(super) fn build_margin_boxes(
    page_style: &PageStyle,
    ctx: &ContentResolveContext,
) -> Vec<ResolvedMarginBox> {
    let mut boxes = Vec::new();
    for (pos, mb) in &page_style.margin_boxes {
        let text = resolve_content(&mb.content, ctx);
        if text.is_empty() {
            continue;
        }
        boxes.push(ResolvedMarginBox {
            position: *pos,
            text,
            font_size_pt: mb.font_size_pt.unwrap_or(9.0),
            color: mb.color.unwrap_or(Color::BLACK),
            text_align: mb.text_align.unwrap_or_else(|| default_text_align_for(*pos)),
        });
    }
    boxes
}

fn default_text_align_for(pos: MarginBoxPosition) -> TextAlign {
    match pos {
        MarginBoxPosition::TopLeft | MarginBoxPosition::BottomLeft => TextAlign::Left,
        MarginBoxPosition::TopCenter | MarginBoxPosition::BottomCenter => TextAlign::Center,
        MarginBoxPosition::TopRight | MarginBoxPosition::BottomRight => TextAlign::Right,
        _ => TextAlign::Center,
    }
}

fn resolve_content(items: &[ContentItem], ctx: &ContentResolveContext) -> String {
    let mut out = String::new();
    for item in items {
        resolve_single_content_item(item, ctx, &mut out);
    }
    out
}

fn resolve_single_content_item(item: &ContentItem, ctx: &ContentResolveContext, out: &mut String) {
    match item {
        ContentItem::String(s) => out.push_str(s),
        ContentItem::Counter(name) => resolve_counter(name, ctx, out),
        ContentItem::RunningString(name) => {
            if let Some(val) = ctx.running_strings.get(name) {
                out.push_str(val);
            }
        }
        ContentItem::TargetCounter(_, _) => {}
        _ => {}
    }
}

fn resolve_counter(name: &str, ctx: &ContentResolveContext, out: &mut String) {
    match name {
        "page" => out.push_str(&ctx.page_num.to_string()),
        "pages" => out.push_str(&ctx.total_pages.to_string()),
        _ => {}
    }
}

pub(super) fn resolve_target_placeholders(text: &mut String, id_map: &HashMap<String, usize>) {
    if !text.contains("__TARGET_PAGE:") {
        return;
    }
    let mut resolved = text.clone();
    while let Some(start) = resolved.find("__TARGET_PAGE:") {
        let rest = &resolved[start + 14..];
        let Some(end) = rest.find("__") else { break };
        let id = &rest[..end];
        let page_num = id_map.get(id).copied().unwrap_or(0);
        let replacement = if page_num > 0 { page_num.to_string() } else { "?".to_string() };
        resolved = format!("{}{}{}", &resolved[..start], replacement, &rest[end + 2..]);
    }
    *text = resolved;
}

pub(super) fn resolve_generated_content_item(ci: &ContentItem, node: &StyledNode) -> String {
    match ci {
        ContentItem::String(s) => s.clone(),
        ContentItem::Counter(name) => format!("__COUNTER:{name}__"),
        ContentItem::TargetCounter(_attr, _counter) => {
            let href = node.attrs.iter()
                .find(|(k, _)| k == "href")
                .map(|(_, v)| v.as_str())
                .unwrap_or("");
            let target_id = href.strip_prefix('#').unwrap_or(href);
            if target_id.is_empty() {
                "?".to_string()
            } else {
                format!("__TARGET_PAGE:{target_id}__")
            }
        }
        _ => String::new(),
    }
}

pub(super) fn collect_text_content(node: &StyledNode) -> String {
    let mut out = String::new();
    for child in &node.children {
        match child {
            StyledContent::Text(t) => out.push_str(t),
            StyledContent::Element(n) => out.push_str(&collect_text_content(n)),
        }
    }
    out
}
