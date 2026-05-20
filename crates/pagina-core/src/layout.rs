use std::collections::HashMap;

use crate::css::values::*;
use crate::css::PageStyleSet;
use crate::font::FontManager;
use crate::style::{ComputedStyle, StringSetSource, StyledContent, StyledNode};

// ═══════════════════════════════════════════════════════════════
//  Layout types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Page {
    pub items: Vec<LayoutItem>,
    pub footnotes: Vec<LayoutItem>,
    pub margin_boxes: Vec<ResolvedMarginBox>,
}

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub x_mm: f64,
    pub y_mm: f64,
    pub font_size_pt: f64,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub color: Color,
    pub text: String,
    pub kind: ItemKind,
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Text,
    HorizontalRule { width_mm: f64, thickness_mm: f64, color: Color },
    Image { id: usize, width_mm: f64, height_mm: f64 },
}

#[derive(Debug)]
pub struct ResolvedMarginBox {
    pub position: MarginBoxPosition,
    pub text: String,
    pub font_size_pt: f64,
    pub color: Color,
    pub text_align: TextAlign,
}

/// An image loaded from the document, ready for embedding.
#[derive(Debug)]
pub struct LoadedImage {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ═══════════════════════════════════════════════════════════════
//  Inline run types (for mixed-style text within a line)
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct InlineStyle {
    font_size_pt: f64,
    font_weight: FontWeight,
    font_style: FontStyle,
    font_family: String,
    color: Color,
}

impl InlineStyle {
    fn from_computed(s: &ComputedStyle) -> Self {
        Self {
            font_size_pt: s.font_size_pt,
            font_weight: s.font_weight,
            font_style: s.font_style,
            font_family: s.font_family.clone(),
            color: s.color,
        }
    }
}

/// A word (or non-breakable token) with its style.
#[derive(Debug, Clone)]
struct StyledWord {
    text: String,
    style: InlineStyle,
    width_mm: f64,
}

/// A laid-out line: sequence of segments, each with position and style.
struct LayoutLine {
    segments: Vec<LineSegment>,
    total_width_mm: f64,
    max_line_height_mm: f64,
}

struct LineSegment {
    text: String,
    x_mm: f64,
    width_mm: f64,
    style: InlineStyle,
}

// ═══════════════════════════════════════════════════════════════
//  Layout state
// ═══════════════════════════════════════════════════════════════

struct LayoutState<'a> {
    page_styles: PageStyleSet,
    fm: &'a FontManager,
    content_width_mm: f64,
    content_height_mm: f64,

    pages: Vec<Page>,
    current_y: f64,

    running_strings: HashMap<String, String>,
    footnotes_pending: Vec<FootnoteData>,
    footnote_counter: usize,
    footnote_area_height: f64,

    images: Vec<LoadedImage>,

    /// Map from element ID to the page number (1-indexed) where it was laid out.
    id_to_page: HashMap<String, usize>,
}

struct FootnoteData {
    number: usize,
    text: String,
    style: InlineStyle,
}

impl<'a> LayoutState<'a> {
    fn new(page_styles: PageStyleSet, fm: &'a FontManager) -> Self {
        let cw = page_styles.base.content_width_mm();
        let ch = page_styles.base.content_height_mm();
        Self {
            page_styles,
            fm,
            content_width_mm: cw,
            content_height_mm: ch,
            pages: vec![Page { items: Vec::new(), footnotes: Vec::new(), margin_boxes: Vec::new() }],
            current_y: 0.0,
            running_strings: HashMap::new(),
            footnotes_pending: Vec::new(),
            footnote_counter: 0,
            footnote_area_height: 0.0,
            images: Vec::new(),
            id_to_page: HashMap::new(),
        }
    }

    fn new_page(&mut self) {
        self.flush_footnotes();
        self.pages.push(Page { items: Vec::new(), footnotes: Vec::new(), margin_boxes: Vec::new() });
        self.current_y = 0.0;
        self.footnote_area_height = 0.0;
    }

    fn available_height(&self) -> f64 {
        self.content_height_mm - self.current_y - self.footnote_area_height
    }

    fn push_item(&mut self, item: LayoutItem) {
        self.pages.last_mut().unwrap().items.push(item);
    }

    fn add_footnote(&mut self, text: String, style: &InlineStyle) {
        self.footnote_counter += 1;
        let num = self.footnote_counter;
        let fn_style = InlineStyle {
            font_size_pt: 8.0,
            ..style.clone()
        };
        self.footnotes_pending.push(FootnoteData { number: num, text, style: fn_style });
        let lh = 8.0 * 1.3 * 25.4 / 72.0;
        self.footnote_area_height += lh + 1.0;
    }

    fn flush_footnotes(&mut self) {
        if self.footnotes_pending.is_empty() {
            return;
        }
        let page = self.pages.last_mut().unwrap();
        let footnotes = std::mem::take(&mut self.footnotes_pending);
        let fn_start_y = self.content_height_mm - self.footnote_area_height;
        let mut fn_y = fn_start_y;

        page.footnotes.push(LayoutItem {
            x_mm: 0.0, y_mm: fn_y,
            font_size_pt: 0.0, font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal, font_family: String::new(),
            color: Color::BLACK, text: String::new(),
            kind: ItemKind::HorizontalRule {
                width_mm: self.content_width_mm * 0.3,
                thickness_mm: 0.15,
                color: Color::rgb(128, 128, 128),
            },
        });
        fn_y += 2.0;

        for fnd in &footnotes {
            let lh = fnd.style.font_size_pt * 1.3 * 25.4 / 72.0;
            page.footnotes.push(LayoutItem {
                x_mm: 0.0, y_mm: fn_y,
                font_size_pt: fnd.style.font_size_pt,
                font_weight: fnd.style.font_weight,
                font_style: fnd.style.font_style,
                font_family: fnd.style.font_family.clone(),
                color: fnd.style.color,
                text: format!("{}. {}", fnd.number, fnd.text),
                kind: ItemKind::Text,
            });
            fn_y += lh;
        }
        self.footnote_area_height = 0.0;
    }

    /// Replace `__TARGET_PAGE:id__` placeholders with actual page numbers.
    fn resolve_target_counters(&mut self) {
        let id_map = &self.id_to_page;
        for page in &mut self.pages {
            for item in page.items.iter_mut().chain(page.footnotes.iter_mut()) {
                if item.text.contains("__TARGET_PAGE:") {
                    let mut resolved = item.text.clone();
                    // Find all __TARGET_PAGE:xxx__ placeholders
                    while let Some(start) = resolved.find("__TARGET_PAGE:") {
                        let rest = &resolved[start + 14..];
                        if let Some(end) = rest.find("__") {
                            let id = &rest[..end];
                            let page_num = id_map.get(id).copied().unwrap_or(0);
                            let replacement = if page_num > 0 {
                                page_num.to_string()
                            } else {
                                "?".to_string()
                            };
                            resolved = format!(
                                "{}{}{}",
                                &resolved[..start],
                                replacement,
                                &rest[end + 2..]
                            );
                        } else {
                            break;
                        }
                    }
                    item.text = resolved;
                }
            }
        }
    }

    fn resolve_margin_boxes(&mut self) {
        let total_pages = self.pages.len();
        for page_num in 0..total_pages {
            let page_style = self.page_styles.for_page(page_num + 1, total_pages);
            let mut boxes = Vec::new();
            for (pos, mb) in &page_style.margin_boxes {
                let text = resolve_content(&mb.content, page_num + 1, total_pages, &self.running_strings, &self.id_to_page);
                if !text.is_empty() {
                    boxes.push(ResolvedMarginBox {
                        position: *pos,
                        text,
                        font_size_pt: mb.font_size_pt.unwrap_or(9.0),
                        color: mb.color.unwrap_or(Color::BLACK),
                        text_align: mb.text_align.unwrap_or(match pos {
                            MarginBoxPosition::TopLeft | MarginBoxPosition::BottomLeft => TextAlign::Left,
                            MarginBoxPosition::TopCenter | MarginBoxPosition::BottomCenter => TextAlign::Center,
                            MarginBoxPosition::TopRight | MarginBoxPosition::BottomRight => TextAlign::Right,
                            _ => TextAlign::Center,
                        }),
                    });
                }
            }
            self.pages[page_num].margin_boxes = boxes;
        }
    }
}

fn resolve_content(
    items: &[ContentItem],
    page_num: usize,
    total_pages: usize,
    running_strings: &HashMap<String, String>,
    id_to_page: &HashMap<String, usize>,
) -> String {
    let mut out = String::new();
    for item in items {
        match item {
            ContentItem::String(s) => out.push_str(s),
            ContentItem::Counter(name) => match name.as_str() {
                "page" => out.push_str(&page_num.to_string()),
                "pages" => out.push_str(&total_pages.to_string()),
                _ => {}
            },
            ContentItem::RunningString(name) => {
                if let Some(val) = running_strings.get(name) {
                    out.push_str(val);
                }
            }
            ContentItem::TargetCounter(attr_name, counter_name) => {
                // target-counter(attr(href), page) → look up the page of the target element
                // attr_name is "href", counter_name is "page"
                // The actual href value needs to be resolved from the element context.
                // In margin boxes this isn't used; it's used in inline content (::after).
                // For now, skip in margin box context.
                let _ = (attr_name, counter_name);
            }
            _ => {}
        }
    }
    out
}

// ═══════════════════════════════════════════════════════════════
//  Inline run collection
// ═══════════════════════════════════════════════════════════════

/// Collect inline content from a block node into styled words for line breaking.
fn collect_styled_words(
    node: &StyledNode,
    fm: &FontManager,
    words: &mut Vec<StyledWord>,
    footnotes: &mut Vec<(String, InlineStyle)>,
    footnote_counter: &mut usize,
) {
    for child in &node.children {
        match child {
            StyledContent::Text(text) => {
                let style = InlineStyle::from_computed(&node.style);
                let resolved = fm.resolve(&style.font_family, style.font_weight, style.font_style);
                let metrics = fm.metrics(&resolved);

                // Split text into words, preserving spaces as separate tokens
                for segment in text.split_inclusive(' ') {
                    let word_part = segment.trim_end_matches(' ');
                    let has_trailing_space = segment.ends_with(' ');

                    if !word_part.is_empty() {
                        let width = metrics.text_width_mm(word_part, style.font_size_pt);
                        words.push(StyledWord {
                            text: word_part.to_string(),
                            style: style.clone(),
                            width_mm: width,
                        });
                    }

                    if has_trailing_space {
                        let sw = metrics.space_width_mm(style.font_size_pt);
                        words.push(StyledWord {
                            text: " ".to_string(),
                            style: style.clone(),
                            width_mm: sw,
                        });
                    }
                }
            }
            StyledContent::Element(child_node) => {
                if child_node.style.is_footnote {
                    *footnote_counter += 1;
                    let num = *footnote_counter;
                    let fn_text = collect_text_content(child_node);
                    footnotes.push((fn_text, InlineStyle::from_computed(&child_node.style)));

                    // Insert superscript reference [n]
                    let ref_text = format!("[{num}]");
                    let ref_style = InlineStyle {
                        font_size_pt: node.style.font_size_pt * 0.7,
                        ..InlineStyle::from_computed(&node.style)
                    };
                    let resolved = fm.resolve(&ref_style.font_family, ref_style.font_weight, ref_style.font_style);
                    let width = fm.metrics(&resolved).text_width_mm(&ref_text, ref_style.font_size_pt);
                    words.push(StyledWord { text: ref_text, style: ref_style, width_mm: width });
                } else if let Some(content_items) = &child_node.style.content {
                    // Element with CSS `content` property (e.g. TOC link with target-counter)
                    let style = InlineStyle::from_computed(&child_node.style);

                    // First, render the element's own inline children
                    collect_styled_words(child_node, fm, words, footnotes, footnote_counter);

                    // Then append generated content
                    for ci in content_items {
                        let text = match ci {
                            ContentItem::String(s) => s.clone(),
                            ContentItem::Counter(name) => format!("__COUNTER:{name}__"),
                            ContentItem::TargetCounter(_attr, _counter) => {
                                // Look up href attribute on this element
                                let href = child_node.attrs.iter()
                                    .find(|(k, _)| k == "href")
                                    .map(|(_, v)| v.as_str())
                                    .unwrap_or("");
                                let target_id = href.strip_prefix('#').unwrap_or(href);
                                if !target_id.is_empty() {
                                    format!("__TARGET_PAGE:{target_id}__")
                                } else {
                                    "?".to_string()
                                }
                            }
                            _ => String::new(),
                        };
                        if !text.is_empty() {
                            let resolved = fm.resolve(&style.font_family, style.font_weight, style.font_style);
                            let width = fm.metrics(&resolved).text_width_mm(&text, style.font_size_pt);
                            words.push(StyledWord { text, style: style.clone(), width_mm: width });
                        }
                    }
                } else {
                    // Recurse into inline children (strong, em, a, span, etc.)
                    collect_styled_words(child_node, fm, words, footnotes, footnote_counter);
                }
            }
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Line breaking with accurate widths
// ═══════════════════════════════════════════════════════════════

fn break_into_lines(words: &[StyledWord], max_width_mm: f64, default_lh: f64) -> Vec<LayoutLine> {
    let mut lines: Vec<LayoutLine> = Vec::new();
    let mut current_segments: Vec<LineSegment> = Vec::new();
    let mut current_x = 0.0_f64;
    let mut max_lh = default_lh;

    for word in words {
        // Newline handling
        if word.text.contains('\n') {
            for (i, part) in word.text.split('\n').enumerate() {
                if i > 0 {
                    lines.push(finish_line(&mut current_segments, current_x, max_lh));
                    current_x = 0.0;
                    max_lh = default_lh;
                }
                if !part.is_empty() {
                    let resolved = crate::font::resolve_builtin(word.style.font_weight, word.style.font_style, &word.style.font_family);
                    let _ = resolved; // width already computed
                    current_segments.push(LineSegment {
                        text: part.to_string(),
                        x_mm: current_x,
                        width_mm: word.width_mm,
                        style: word.style.clone(),
                    });
                    current_x += word.width_mm;
                }
            }
            continue;
        }

        // Space handling: append to previous segment if same style
        if word.text == " " {
            if current_x > 0.0 {
                if let Some(last) = current_segments.last_mut() {
                    if same_inline_style(&last.style, &word.style) {
                        last.text.push(' ');
                        last.width_mm += word.width_mm;
                    }
                }
                current_x += word.width_mm;
            }
            continue;
        }

        let word_lh = word.style.font_size_pt * 1.4 * 25.4 / 72.0;

        // Check if word fits on current line
        if current_x + word.width_mm > max_width_mm && current_x > 0.0 {
            // Trim trailing space from last segment before wrapping
            if let Some(last) = current_segments.last_mut() {
                if last.text.ends_with(' ') {
                    last.text.pop();
                }
            }
            lines.push(finish_line(&mut current_segments, current_x, max_lh));
            current_x = 0.0;
            max_lh = default_lh;
        }

        max_lh = max_lh.max(word_lh);

        // Merge with previous segment if same style and was space-terminated
        if let Some(last) = current_segments.last_mut() {
            if same_inline_style(&last.style, &word.style) && last.text.ends_with(' ') {
                last.text.push_str(&word.text);
                last.width_mm = current_x + word.width_mm - last.x_mm;
                current_x += word.width_mm;
                continue;
            }
        }

        current_segments.push(LineSegment {
            text: word.text.clone(),
            x_mm: current_x,
            width_mm: word.width_mm,
            style: word.style.clone(),
        });
        current_x += word.width_mm;

        // Add inter-word space width after the word for next word's position
        // (the actual space word will add it)
    }

    if !current_segments.is_empty() {
        lines.push(finish_line(&mut current_segments, current_x, max_lh));
    }

    lines
}

fn same_inline_style(a: &InlineStyle, b: &InlineStyle) -> bool {
    a.font_size_pt == b.font_size_pt
        && a.font_weight == b.font_weight
        && a.font_style == b.font_style
        && a.font_family == b.font_family
        && a.color.r == b.color.r
        && a.color.g == b.color.g
        && a.color.b == b.color.b
}

fn finish_line(segments: &mut Vec<LineSegment>, total_width: f64, max_lh: f64) -> LayoutLine {
    LayoutLine {
        segments: std::mem::take(segments),
        total_width_mm: total_width,
        max_line_height_mm: max_lh,
    }
}

// ═══════════════════════════════════════════════════════════════
//  Main layout
// ═══════════════════════════════════════════════════════════════

pub fn lay_out(page_styles: &PageStyleSet, tree: &StyledNode, fm: &FontManager) -> (Vec<Page>, Vec<LoadedImage>) {
    let mut state = LayoutState::new(page_styles.clone(), fm);
    lay_out_node(tree, &mut state, true);
    state.flush_footnotes();
    state.resolve_target_counters();
    state.resolve_margin_boxes();

    while state.pages.len() > 1
        && state.pages.last().map_or(false, |p| p.items.is_empty() && p.footnotes.is_empty())
    {
        state.pages.pop();
    }

    let images = std::mem::take(&mut state.images);
    (state.pages, images)
}

fn lay_out_node(node: &StyledNode, state: &mut LayoutState, is_first_block: bool) {
    // Update running strings
    if let Some((name, source)) = &node.style.string_set {
        let value = match source {
            StringSetSource::Content => collect_text_content(node),
            StringSetSource::Attr(attr) => node.attrs.iter()
                .find(|(k, _)| k == attr)
                .map(|(_, v)| v.clone())
                .unwrap_or_default(),
        };
        state.running_strings.insert(name.clone(), value);
    }

    // Handle break-before
    if node.style.break_before == BreakValue::Page && !is_first_block && state.current_y > 0.0 {
        state.new_page();
    }

    // Record element ID → current page number (after break-before)
    if let Some(id) = &node.id {
        state.id_to_page.insert(id.clone(), state.pages.len());
    }

    match node.tag.as_str() {
        "#document" | "html" | "body" | "main" | "article" | "section" | "div"
        | "header" | "footer" | "nav" | "aside" | "figure" => {
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
        "hr" => {
            state.current_y += node.style.margin_top_mm;
            state.push_item(LayoutItem {
                x_mm: 0.0, y_mm: state.current_y,
                font_size_pt: 0.0, font_weight: FontWeight::Normal,
                font_style: FontStyle::Normal, font_family: String::new(),
                color: Color::BLACK, text: String::new(),
                kind: ItemKind::HorizontalRule {
                    width_mm: state.content_width_mm,
                    thickness_mm: node.style.border_bottom_width_mm.max(0.2),
                    color: node.style.border_bottom_color,
                },
            });
            state.current_y += node.style.margin_bottom_mm + 0.5;
        }
        "img" => {
            lay_out_image(node, state);
        }
        "math" => {
            lay_out_math(node, state);
        }
        "ul" | "ol" => {
            state.current_y += node.style.margin_top_mm;
            let mut counter = 0;
            for child in &node.children {
                if let StyledContent::Element(li) = child {
                    counter += 1;
                    let prefix = if node.tag == "ol" {
                        format!("{}. ", counter)
                    } else {
                        "- ".to_string()
                    };
                    lay_out_list_item(li, &prefix, state);
                }
            }
            state.current_y += node.style.margin_bottom_mm;
        }
        "table" => {
            lay_out_table(node, state);
        }
        _ => {
            lay_out_block(node, state);
        }
    }

    if node.style.break_after == BreakValue::Page && state.current_y > 0.0 {
        state.new_page();
    }
}

/// Layout a block element with inline children (the core mixed-style path).
fn lay_out_block(node: &StyledNode, state: &mut LayoutState) {
    let style = &node.style;

    // Collect inline content into styled words
    let mut words = Vec::new();
    let mut footnote_refs = Vec::new();
    collect_styled_words(node, state.fm, &mut words, &mut footnote_refs, &mut state.footnote_counter);

    // Register footnotes
    for (fn_text, fn_style) in &footnote_refs {
        state.add_footnote(fn_text.clone(), fn_style);
    }

    // Check if there's any content
    let has_text = words.iter().any(|w| w.text.trim().len() > 0);
    if !has_text && style.border_bottom_width_mm == 0.0 {
        return;
    }

    state.current_y += style.margin_top_mm + style.padding_top_mm;

    if has_text {
        let default_lh = state.fm.metrics(
            &state.fm.resolve(&style.font_family, style.font_weight, style.font_style)
        ).line_height_mm(style.font_size_pt, style.line_height);

        let lines = break_into_lines(&words, state.content_width_mm, default_lh);

        for line in &lines {
            if state.current_y + line.max_line_height_mm > state.available_height()
                && !state.pages.last().unwrap().items.is_empty()
            {
                state.new_page();
            }

            // Apply text-align offset
            let align_offset = match style.text_align {
                TextAlign::Center => (state.content_width_mm - line.total_width_mm).max(0.0) / 2.0,
                TextAlign::Right => (state.content_width_mm - line.total_width_mm).max(0.0),
                _ => 0.0,
            };

            for seg in &line.segments {
                state.push_item(LayoutItem {
                    x_mm: seg.x_mm + align_offset,
                    y_mm: state.current_y,
                    font_size_pt: seg.style.font_size_pt,
                    font_weight: seg.style.font_weight,
                    font_style: seg.style.font_style,
                    font_family: seg.style.font_family.clone(),
                    color: seg.style.color,
                    text: seg.text.clone(),
                    kind: ItemKind::Text,
                });
            }

            state.current_y += line.max_line_height_mm;
        }
    }

    // Border bottom
    if style.border_bottom_width_mm > 0.0 {
        state.push_item(LayoutItem {
            x_mm: 0.0, y_mm: state.current_y,
            font_size_pt: 0.0, font_weight: FontWeight::Normal,
            font_style: FontStyle::Normal, font_family: String::new(),
            color: Color::BLACK, text: String::new(),
            kind: ItemKind::HorizontalRule {
                width_mm: state.content_width_mm,
                thickness_mm: style.border_bottom_width_mm,
                color: style.border_bottom_color,
            },
        });
        state.current_y += style.border_bottom_width_mm + 0.5;
    }

    state.current_y += style.padding_bottom_mm + style.margin_bottom_mm;
}

/// Simple text layout (fallback, single style).
fn lay_out_simple_text(text: &str, style: &ComputedStyle, state: &mut LayoutState) {
    let inline_style = InlineStyle::from_computed(style);
    let resolved = state.fm.resolve(&style.font_family, style.font_weight, style.font_style);
    let metrics = state.fm.metrics(&resolved);
    let lh = metrics.line_height_mm(style.font_size_pt, style.line_height);

    // Simple word wrap
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
            && !state.pages.last().unwrap().items.is_empty()
        {
            state.new_page();
        }
        for seg in &line.segments {
            state.push_item(LayoutItem {
                x_mm: seg.x_mm,
                y_mm: state.current_y,
                font_size_pt: seg.style.font_size_pt,
                font_weight: seg.style.font_weight,
                font_style: seg.style.font_style,
                font_family: seg.style.font_family.clone(),
                color: seg.style.color,
                text: seg.text.clone(),
                kind: ItemKind::Text,
            });
        }
        state.current_y += line.max_line_height_mm;
    }
}

fn lay_out_list_item(li: &StyledNode, prefix: &str, state: &mut LayoutState) {
    state.current_y += li.style.margin_top_mm;

    // Collect inline content with prefix prepended
    let inline_style = InlineStyle::from_computed(&li.style);
    let resolved = state.fm.resolve(&li.style.font_family, li.style.font_weight, li.style.font_style);
    let metrics = state.fm.metrics(&resolved);

    let mut words = Vec::new();
    // Add prefix as first word
    let prefix_w = metrics.text_width_mm(prefix, li.style.font_size_pt);
    words.push(StyledWord { text: prefix.to_string(), style: inline_style.clone(), width_mm: prefix_w });

    let mut footnote_refs = Vec::new();
    collect_styled_words(li, state.fm, &mut words, &mut footnote_refs, &mut state.footnote_counter);
    for (fn_text, fn_style) in &footnote_refs {
        state.add_footnote(fn_text.clone(), fn_style);
    }

    let lh = metrics.line_height_mm(li.style.font_size_pt, li.style.line_height);
    let lines = break_into_lines(&words, state.content_width_mm, lh);

    for line in &lines {
        if state.current_y + line.max_line_height_mm > state.available_height()
            && !state.pages.last().unwrap().items.is_empty()
        {
            state.new_page();
        }
        for seg in &line.segments {
            state.push_item(LayoutItem {
                x_mm: seg.x_mm,
                y_mm: state.current_y,
                font_size_pt: seg.style.font_size_pt,
                font_weight: seg.style.font_weight,
                font_style: seg.style.font_style,
                font_family: seg.style.font_family.clone(),
                color: seg.style.color,
                text: seg.text.clone(),
                kind: ItemKind::Text,
            });
        }
        state.current_y += line.max_line_height_mm;
    }

    state.current_y += li.style.margin_bottom_mm;
}

fn lay_out_image(node: &StyledNode, state: &mut LayoutState) {
    let src = node.attrs.iter().find(|(k, _)| k == "src").map(|(_, v)| v.as_str());
    let src = match src {
        Some(s) => s,
        None => return,
    };

    // SVG support: rasterize via resvg
    if src.ends_with(".svg") {
        if let Some(loaded) = crate::svg::render_svg_file(src, 300.0) {
            embed_loaded_image(loaded, node, state);
        }
        return;
    }

    // Raster image (PNG, JPEG)
    let img = match image::open(src) {
        Ok(img) => img,
        Err(_) => return,
    };

    let (img_w, img_h) = (img.width(), img.height());
    let rgb = img.to_rgb8();

    // Scale to fit content width (max 100% of content area, maintain aspect ratio)
    let dpi = 96.0;
    let natural_width_mm = img_w as f64 / dpi * 25.4;
    let natural_height_mm = img_h as f64 / dpi * 25.4;

    let scale = if natural_width_mm > state.content_width_mm {
        state.content_width_mm / natural_width_mm
    } else {
        1.0
    };

    let display_w = natural_width_mm * scale;
    let display_h = natural_height_mm * scale;

    state.current_y += node.style.margin_top_mm;

    if state.current_y + display_h > state.available_height()
        && !state.pages.last().unwrap().items.is_empty()
    {
        state.new_page();
    }

    let image_id = state.images.len();
    state.images.push(LoadedImage {
        pixels: rgb.into_raw(),
        width: img_w,
        height: img_h,
    });

    state.push_item(LayoutItem {
        x_mm: 0.0,
        y_mm: state.current_y,
        font_size_pt: 0.0,
        font_weight: FontWeight::Normal,
        font_style: FontStyle::Normal,
        font_family: String::new(),
        color: Color::BLACK,
        text: String::new(),
        kind: ItemKind::Image { id: image_id, width_mm: display_w, height_mm: display_h },
    });

    state.current_y += display_h + node.style.margin_bottom_mm;
}

fn embed_loaded_image(loaded: LoadedImage, node: &StyledNode, state: &mut LayoutState) {
    let dpi = 300.0;
    let natural_w = loaded.width as f64 / dpi * 25.4;
    let natural_h = loaded.height as f64 / dpi * 25.4;
    let scale = if natural_w > state.content_width_mm {
        state.content_width_mm / natural_w
    } else {
        1.0
    };
    let display_w = natural_w * scale;
    let display_h = natural_h * scale;

    state.current_y += node.style.margin_top_mm;
    if state.current_y + display_h > state.available_height()
        && !state.pages.last().unwrap().items.is_empty()
    {
        state.new_page();
    }

    let image_id = state.images.len();
    state.images.push(loaded);
    state.push_item(LayoutItem {
        x_mm: 0.0, y_mm: state.current_y,
        font_size_pt: 0.0, font_weight: FontWeight::Normal,
        font_style: FontStyle::Normal, font_family: String::new(),
        color: Color::BLACK, text: String::new(),
        kind: ItemKind::Image { id: image_id, width_mm: display_w, height_mm: display_h },
    });
    state.current_y += display_h + node.style.margin_bottom_mm;
}

fn lay_out_table(node: &StyledNode, state: &mut LayoutState) {
    state.current_y += node.style.margin_top_mm;

    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut is_header: Vec<bool> = Vec::new();

    for child in &node.children {
        if let StyledContent::Element(child_node) = child {
            match child_node.tag.as_str() {
                "thead" | "tbody" | "tfoot" => {
                    for row_child in &child_node.children {
                        if let StyledContent::Element(tr) = row_child {
                            if tr.tag == "tr" {
                                let (cells, is_hdr) = collect_table_row(tr);
                                rows.push(cells);
                                is_header.push(is_hdr);
                            }
                        }
                    }
                }
                "tr" => {
                    let (cells, is_hdr) = collect_table_row(child_node);
                    rows.push(cells);
                    is_header.push(is_hdr);
                }
                _ => {}
            }
        }
    }

    if rows.is_empty() {
        return;
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let col_width = state.content_width_mm / num_cols as f64;
    let cell_padding = 1.5;
    let resolved = state.fm.resolve(&node.style.font_family, node.style.font_weight, node.style.font_style);
    let lh = state.fm.metrics(&resolved).line_height_mm(node.style.font_size_pt, node.style.line_height);

    for (row_idx, row) in rows.iter().enumerate() {
        if state.current_y + lh + cell_padding * 2.0 > state.content_height_mm {
            state.new_page();
        }
        let is_hdr = is_header.get(row_idx).copied().unwrap_or(false);
        for (col_idx, cell_text) in row.iter().enumerate() {
            let x = col_idx as f64 * col_width + cell_padding;
            state.push_item(LayoutItem {
                x_mm: x,
                y_mm: state.current_y + cell_padding,
                font_size_pt: node.style.font_size_pt,
                font_weight: if is_hdr { FontWeight::Bold } else { node.style.font_weight },
                font_style: node.style.font_style,
                font_family: node.style.font_family.clone(),
                color: node.style.color,
                text: cell_text.clone(),
                kind: ItemKind::Text,
            });
        }
        state.current_y += lh + cell_padding * 2.0;
        if is_hdr || row_idx == rows.len() - 1 {
            state.push_item(LayoutItem {
                x_mm: 0.0, y_mm: state.current_y,
                font_size_pt: 0.0, font_weight: FontWeight::Normal,
                font_style: FontStyle::Normal, font_family: String::new(),
                color: Color::BLACK, text: String::new(),
                kind: ItemKind::HorizontalRule {
                    width_mm: state.content_width_mm,
                    thickness_mm: if is_hdr { 0.3 } else { 0.15 },
                    color: Color::rgb(180, 180, 180),
                },
            });
            state.current_y += 0.5;
        }
    }
    state.current_y += node.style.margin_bottom_mm;
}

fn collect_table_row(tr: &StyledNode) -> (Vec<String>, bool) {
    let mut cells = Vec::new();
    let mut is_header = false;
    for child in &tr.children {
        if let StyledContent::Element(td) = child {
            if td.tag == "th" { is_header = true; }
            cells.push(collect_text_content(td).trim().to_string());
        }
    }
    (cells, is_header)
}

fn lay_out_math(node: &StyledNode, state: &mut LayoutState) {
    let items = crate::mathml::render_math(node, node.style.font_size_pt, state.fm);
    let lh = node.style.font_size_pt * 1.6 * 25.4 / 72.0;

    state.current_y += node.style.margin_top_mm;

    if state.current_y + lh > state.available_height()
        && !state.pages.last().unwrap().items.is_empty()
    {
        state.new_page();
    }

    for mut item in items {
        item.y_mm += state.current_y;
        state.push_item(item);
    }

    state.current_y += lh + node.style.margin_bottom_mm;
}

fn collect_text_content(node: &StyledNode) -> String {
    let mut out = String::new();
    for child in &node.children {
        match child {
            StyledContent::Text(t) => out.push_str(t),
            StyledContent::Element(n) => out.push_str(&collect_text_content(n)),
        }
    }
    out
}
