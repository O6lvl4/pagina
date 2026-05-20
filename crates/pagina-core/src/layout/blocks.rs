//! Block-level layout: containers, lists, inline text blocks.

use crate::css::values::*;
use crate::style::{ComputedStyle, StyledContent, StyledNode};

use super::{LayoutItem, ItemKind, LayoutState, InlineStyle, StyledWord, Bookmark, LinkAnnotation, LinkTarget};
use super::content::collect_text_content;
use super::line_break::{
    break_into_lines, compute_align_offset, emit_line_segments, WordCollector,
};
use super::table_image;

// ═══════════════════════════════════════════════════════════════
//  Node dispatch
// ═══════════════════════════════════════════════════════════════

pub(super) fn lay_out_node(node: &StyledNode, state: &mut LayoutState, is_first_block: bool) {
    update_running_strings(node, state);
    handle_break_before(node, state, is_first_block);
    record_element_id(node, state);

    dispatch_node(node, state, is_first_block);

    if node.style.break_after == BreakValue::Page && state.current_y > 0.0 {
        state.new_page();
    }
}

fn dispatch_node(node: &StyledNode, state: &mut LayoutState, is_first_block: bool) {
    match node.tag.as_str() {
        "#document" | "html" | "body" | "main" | "article" | "section" | "div"
        | "header" | "footer" | "nav" | "aside" | "figure" => {
            lay_out_container(node, state, is_first_block);
        }
        "hr" => lay_out_hr(node, state),
        "img" => table_image::lay_out_image(node, state),
        "math" => table_image::lay_out_math(node, state),
        "ul" | "ol" => lay_out_list(node, state),
        "table" => table_image::lay_out_table(node, state),
        _ => lay_out_block(node, state),
    }
}

fn update_running_strings(node: &StyledNode, state: &mut LayoutState) {
    let Some((name, source)) = &node.style.string_set else { return };
    let value = match source {
        crate::style::StringSetSource::Content => collect_text_content(node),
        crate::style::StringSetSource::Attr(attr) => node.attrs.iter()
            .find(|(k, _)| k == attr)
            .map(|(_, v)| v.clone())
            .unwrap_or_default(),
    };
    state.running_strings.insert(name.clone(), value);
}

fn handle_break_before(node: &StyledNode, state: &mut LayoutState, is_first_block: bool) {
    if node.style.break_before == BreakValue::Page && !is_first_block && state.current_y > 0.0 {
        state.new_page();
    }
}

fn record_element_id(node: &StyledNode, state: &mut LayoutState) {
    if let Some(id) = &node.id {
        state.id_to_page.insert(id.clone(), state.pages.len());
    }
}

// ═══════════════════════════════════════════════════════════════
//  Container layout
// ═══════════════════════════════════════════════════════════════

fn lay_out_container(node: &StyledNode, state: &mut LayoutState, is_first_block: bool) {
    let mut first = true;
    for child in &node.children {
        match child {
            StyledContent::Element(child_node) => {
                lay_out_node(child_node, state, first && is_first_block);
                first = false;
            }
            StyledContent::Text(text) => {
                let trimmed = text.trim();
                if !trimmed.is_empty() {
                    lay_out_simple_text(trimmed, &node.style, state);
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Block layout (inline children)
// ═══════════════════════════════════════════════════════════════

fn lay_out_block(node: &StyledNode, state: &mut LayoutState) {
    let style = &node.style;

    let mut collector = WordCollector::new(state.fm, state.footnote_counter);
    collector.collect(node);
    state.footnote_counter = collector.footnote_counter;

    for (fn_text, fn_style) in &collector.footnotes {
        state.add_footnote(fn_text.clone(), fn_style);
    }

    let has_text = collector.words.iter().any(|w| !w.text.trim().is_empty());
    if !has_text && style.border_bottom_width_mm == 0.0 {
        return;
    }

    state.current_y += style.margin_top_mm + style.padding_top_mm;

    // Heading orphan control: keep heading + at least 1 body line together
    if matches!(node.tag.as_str(), "h3" | "h4" | "h5" | "h6") {
        let heading_lh = style.font_size_pt * style.line_height * 25.4 / 72.0;
        let body_lh = 11.0 * 1.4 * 25.4 / 72.0;
        let min_needed = heading_lh + body_lh + style.margin_bottom_mm;
        if state.available_height() < min_needed && state.current_page_has_items() {
            state.new_page();
        }
    }

    emit_heading_bookmark(node, state);

    if has_text {
        emit_block_text_lines(&collector.words, style, state);
    }

    emit_border_bottom(style, state);
    collect_links_from_node(node, state);
    state.current_y += style.padding_bottom_mm + style.margin_bottom_mm;
}

fn emit_heading_bookmark(node: &StyledNode, state: &mut LayoutState) {
    if !matches!(node.tag.as_str(), "h1" | "h2" | "h3" | "h4" | "h5" | "h6") {
        return;
    }
    let level = node.tag.as_bytes()[1] - b'0';
    let title = collect_text_content(node).trim().to_string();
    if title.is_empty() {
        return;
    }
    let y = state.current_y;
    state.current_page_mut().bookmarks.push(Bookmark { title, level, y_mm: y });
}

fn emit_block_text_lines(words: &[StyledWord], style: &ComputedStyle, state: &mut LayoutState) {
    let default_lh = state.fm.metrics(
        &state.fm.resolve(&style.font_family, style.font_weight, style.font_style)
    ).line_height_mm(style.font_size_pt, style.line_height);

    let lines = break_into_lines(words, state.content_width_mm, default_lh);

    for line in &lines {
        state.ensure_space(line.max_line_height_mm);
        let align_offset = compute_align_offset(style.text_align, state.content_width_mm, line.total_width_mm);
        let y = state.current_y;
        let items = &mut state.current_page_mut().items;
        emit_line_segments(&line.segments, y, align_offset, items);
        state.current_y += line.max_line_height_mm;
    }
}

fn emit_border_bottom(style: &ComputedStyle, state: &mut LayoutState) {
    if style.border_bottom_width_mm <= 0.0 {
        return;
    }
    state.push_item(LayoutItem::hr_item(
        (0.0, state.current_y),
        ItemKind::HorizontalRule { width_mm: state.content_width_mm, thickness_mm: style.border_bottom_width_mm, color: style.border_bottom_color },
    ));
    state.current_y += style.border_bottom_width_mm + 0.5;
}

// ═══════════════════════════════════════════════════════════════
//  Simple text layout
// ═══════════════════════════════════════════════════════════════

fn lay_out_simple_text(text: &str, style: &ComputedStyle, state: &mut LayoutState) {
    let inline_style = InlineStyle::from_computed(style);
    let resolved = state.fm.resolve(&style.font_family, style.font_weight, style.font_style);
    let metrics = state.fm.metrics(&resolved);
    let lh = metrics.line_height_mm(style.font_size_pt, style.line_height);

    let mut words = Vec::new();
    for word in text.split_whitespace() {
        let w = metrics.text_width_mm(word, style.font_size_pt);
        words.push(StyledWord { text: word.to_string(), style: inline_style.clone(), width_mm: w });
        let sw = metrics.space_width_mm(style.font_size_pt);
        words.push(StyledWord { text: " ".to_string(), style: inline_style.clone(), width_mm: sw });
    }

    let lines = break_into_lines(&words, state.content_width_mm, lh);
    for line in &lines {
        if state.current_y + line.max_line_height_mm > state.content_height_mm
            && state.current_page_has_items()
        {
            state.new_page();
        }
        let y = state.current_y;
        let items = &mut state.current_page_mut().items;
        emit_line_segments(&line.segments, y, 0.0, items);
        state.current_y += line.max_line_height_mm;
    }
}

// ═══════════════════════════════════════════════════════════════
//  HR layout
// ═══════════════════════════════════════════════════════════════

fn lay_out_hr(node: &StyledNode, state: &mut LayoutState) {
    state.current_y += node.style.margin_top_mm;
    state.push_item(LayoutItem::hr_item(
        (0.0, state.current_y),
        ItemKind::HorizontalRule { width_mm: state.content_width_mm, thickness_mm: node.style.border_bottom_width_mm.max(0.2), color: node.style.border_bottom_color },
    ));
    state.current_y += node.style.margin_bottom_mm + 0.5;
}

// ═══════════════════════════════════════════════════════════════
//  List layout
// ═══════════════════════════════════════════════════════════════

fn lay_out_list(node: &StyledNode, state: &mut LayoutState) {
    state.current_y += node.style.margin_top_mm;
    let mut counter = 0;
    for child in &node.children {
        let StyledContent::Element(li) = child else { continue };
        counter += 1;
        let prefix = if node.tag == "ol" {
            format!("{}. ", counter)
        } else {
            "- ".to_string()
        };
        lay_out_list_item(li, &prefix, state);
    }
    state.current_y += node.style.margin_bottom_mm;
}

fn lay_out_list_item(li: &StyledNode, prefix: &str, state: &mut LayoutState) {
    state.current_y += li.style.margin_top_mm;

    let inline_style = InlineStyle::from_computed(&li.style);
    let resolved = state.fm.resolve(&li.style.font_family, li.style.font_weight, li.style.font_style);
    let metrics = state.fm.metrics(&resolved);

    let mut words = Vec::new();
    let prefix_w = metrics.text_width_mm(prefix, li.style.font_size_pt);
    words.push(StyledWord { text: prefix.to_string(), style: inline_style, width_mm: prefix_w });

    let mut collector = WordCollector::new(state.fm, state.footnote_counter);
    collector.collect(li);
    state.footnote_counter = collector.footnote_counter;
    words.extend(collector.words);
    for (fn_text, fn_style) in &collector.footnotes {
        state.add_footnote(fn_text.clone(), fn_style);
    }

    let lh = metrics.line_height_mm(li.style.font_size_pt, li.style.line_height);
    let lines = break_into_lines(&words, state.content_width_mm, lh);

    for line in &lines {
        state.ensure_space(line.max_line_height_mm);
        let y = state.current_y;
        let items = &mut state.current_page_mut().items;
        emit_line_segments(&line.segments, y, 0.0, items);
        state.current_y += line.max_line_height_mm;
    }

    state.current_y += li.style.margin_bottom_mm;
}

// ═══════════════════════════════════════════════════════════════
//  Link annotations
// ═══════════════════════════════════════════════════════════════

fn collect_links_from_node(node: &StyledNode, state: &mut LayoutState) {
    if node.tag == "a" {
        emit_link_annotation(node, state);
    }
    for child in &node.children {
        if let StyledContent::Element(child_node) = child {
            collect_links_from_node(child_node, state);
        }
    }
}

fn emit_link_annotation(node: &StyledNode, state: &mut LayoutState) {
    let Some(href) = node.attrs.iter().find(|(k, _)| k == "href").map(|(_, v)| v.clone()) else {
        return;
    };
    let text = collect_text_content(node);
    let resolved = state.fm.resolve(&node.style.font_family, node.style.font_weight, node.style.font_style);
    let text_width = state.fm.metrics(&resolved).text_width_mm(text.trim(), node.style.font_size_pt);
    let lh = node.style.font_size_pt * node.style.line_height * 25.4 / 72.0;

    let target = if let Some(id) = href.strip_prefix('#') {
        LinkTarget::Internal(id.to_string())
    } else {
        LinkTarget::Uri(href)
    };

    let annotation = LinkAnnotation {
        x_mm: 0.0,
        y_mm: state.current_y - lh,
        width_mm: text_width.min(state.content_width_mm),
        height_mm: lh,
        target,
    };
    state.current_page_mut().links.push(annotation);
}
