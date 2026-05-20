//! Line breaking and inline word collection.

use crate::css::values::*;
use crate::font::FontManager;
use crate::style::{ComputedStyle, StyledContent, StyledNode};

use super::{LayoutItem, InlineStyle, StyledWord};
use super::content::{resolve_generated_content_item, collect_text_content};

// ═══════════════════════════════════════════════════════════════
//  Line layout types
// ═══════════════════════════════════════════════════════════════

pub(super) struct LayoutLine {
    pub(super) segments: Vec<LineSegment>,
    pub(super) total_width_mm: f64,
    pub(super) max_line_height_mm: f64,
}

pub(super) struct LineSegment {
    pub(super) text: String,
    pub(super) x_mm: f64,
    pub(super) width_mm: f64,
    pub(super) style: InlineStyle,
}

// ═══════════════════════════════════════════════════════════════
//  Inline run collection
// ═══════════════════════════════════════════════════════════════

/// Collector context for inline word gathering.
pub(super) struct WordCollector<'a> {
    fm: &'a FontManager,
    pub(super) words: Vec<StyledWord>,
    pub(super) footnotes: Vec<(String, InlineStyle)>,
    pub(super) footnote_counter: usize,
}

impl<'a> WordCollector<'a> {
    pub(super) fn new(fm: &'a FontManager, footnote_counter: usize) -> Self {
        Self { fm, words: Vec::new(), footnotes: Vec::new(), footnote_counter }
    }

    pub(super) fn collect(&mut self, node: &StyledNode) {
        for child in &node.children {
            match child {
                StyledContent::Text(text) => self.collect_text(text, &node.style),
                StyledContent::Element(child_node) => self.collect_element(child_node, &node.style),
            }
        }
    }

    fn collect_text(&mut self, text: &str, parent_style: &ComputedStyle) {
        let style = InlineStyle::from_computed(parent_style);
        let resolved = self.fm.resolve(&style.font_family, style.font_weight, style.font_style);
        let metrics = self.fm.metrics(&resolved);

        // Split into segments: spaces, CJK runs (char-by-char with kinsoku), Latin words
        let mut current_word = String::new();
        let chars: Vec<char> = text.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            if ch == ' ' {
                self.flush_word(&mut current_word, &style, metrics);
                let sw = metrics.space_width_mm(style.font_size_pt);
                self.words.push(StyledWord { text: " ".to_string(), style: style.clone(), width_mm: sw });
            } else if is_cjk(ch) {
                self.flush_word(&mut current_word, &style, metrics);

                // Build a CJK chunk: current char + any following kinsoku no-break-before chars
                let mut chunk = String::new();
                chunk.push(ch);
                // Absorb following chars that must not start a line (closing brackets, punctuation)
                while i + 1 < chars.len() && is_no_break_before(chars[i + 1]) {
                    i += 1;
                    chunk.push(chars[i]);
                }

                let w = metrics.text_width_mm(&chunk, style.font_size_pt);
                self.words.push(StyledWord { text: chunk, style: style.clone(), width_mm: w });
            } else if is_no_break_before(ch) {
                // ASCII closing bracket etc. at CJK boundary — attach to previous word
                if let Some(last) = self.words.last_mut() {
                    let w = metrics.text_width_mm(&ch.to_string(), style.font_size_pt);
                    last.text.push(ch);
                    last.width_mm += w;
                } else {
                    current_word.push(ch);
                }
            } else {
                current_word.push(ch);
            }
            i += 1;
        }
        self.flush_word(&mut current_word, &style, metrics);
    }

    fn flush_word(&mut self, word: &mut String, style: &InlineStyle, metrics: &crate::font::FontMetrics) {
        if word.is_empty() {
            return;
        }
        let w = metrics.text_width_mm(word, style.font_size_pt);
        self.words.push(StyledWord { text: std::mem::take(word), style: style.clone(), width_mm: w });
    }

    fn collect_element(&mut self, child_node: &StyledNode, parent_style: &ComputedStyle) {
        if child_node.style.is_footnote {
            self.collect_footnote(child_node, parent_style);
        } else if child_node.style.content.is_some() {
            self.collect_generated_content(child_node);
        } else {
            self.collect(child_node);
        }
    }

    fn collect_footnote(&mut self, child_node: &StyledNode, parent_style: &ComputedStyle) {
        self.footnote_counter += 1;
        let num = self.footnote_counter;
        let fn_text = collect_text_content(child_node);
        self.footnotes.push((fn_text, InlineStyle::from_computed(&child_node.style)));

        let ref_text = format!("[{num}]");
        let ref_style = InlineStyle {
            font_size_pt: parent_style.font_size_pt * 0.7,
            ..InlineStyle::from_computed(parent_style)
        };
        let resolved = self.fm.resolve(&ref_style.font_family, ref_style.font_weight, ref_style.font_style);
        let width = self.fm.metrics(&resolved).text_width_mm(&ref_text, ref_style.font_size_pt);
        self.words.push(StyledWord { text: ref_text, style: ref_style, width_mm: width });
    }

    fn collect_generated_content(&mut self, child_node: &StyledNode) {
        let content_items = child_node.style.content.as_ref().expect("checked is_some above");
        let style = InlineStyle::from_computed(&child_node.style);

        self.collect(child_node);

        for ci in content_items {
            let text = resolve_generated_content_item(ci, child_node);
            if text.is_empty() {
                continue;
            }
            let resolved = self.fm.resolve(&style.font_family, style.font_weight, style.font_style);
            let width = self.fm.metrics(&resolved).text_width_mm(&text, style.font_size_pt);
            self.words.push(StyledWord { text, style: style.clone(), width_mm: width });
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Line breaking with accurate widths
// ═══════════════════════════════════════════════════════════════

struct LineBreaker {
    lines: Vec<LayoutLine>,
    current_segments: Vec<LineSegment>,
    current_x: f64,
    max_lh: f64,
    max_width_mm: f64,
    default_lh: f64,
}

impl LineBreaker {
    fn new(max_width_mm: f64, default_lh: f64) -> Self {
        Self {
            lines: Vec::new(),
            current_segments: Vec::new(),
            current_x: 0.0,
            max_lh: default_lh,
            max_width_mm,
            default_lh,
        }
    }

    fn break_words(mut self, words: &[StyledWord]) -> Vec<LayoutLine> {
        for word in words {
            if word.text.contains('\n') {
                self.handle_newline_word(word);
            } else if word.text == " " {
                self.handle_space(word);
            } else {
                self.handle_regular_word(word);
            }
        }
        if !self.current_segments.is_empty() {
            self.finish_current_line();
        }
        self.lines
    }

    fn handle_newline_word(&mut self, word: &StyledWord) {
        for (i, part) in word.text.split('\n').enumerate() {
            if i > 0 {
                self.finish_current_line();
            }
            if !part.is_empty() {
                self.current_segments.push(LineSegment {
                    text: part.to_string(),
                    x_mm: self.current_x,
                    width_mm: word.width_mm,
                    style: word.style.clone(),
                });
                self.current_x += word.width_mm;
            }
        }
    }

    fn handle_space(&mut self, word: &StyledWord) {
        if self.current_x <= 0.0 {
            return;
        }
        if let Some(last) = self.current_segments.last_mut() {
            if last.style.same_as(&word.style) {
                last.text.push(' ');
                last.width_mm += word.width_mm;
            }
        }
        self.current_x += word.width_mm;
    }

    fn handle_regular_word(&mut self, word: &StyledWord) {
        let word_lh = word.style.font_size_pt * 1.4 * 25.4 / 72.0;

        if self.current_x + word.width_mm > self.max_width_mm && self.current_x > 0.0 {
            self.trim_trailing_space();
            self.finish_current_line();
        }

        self.max_lh = self.max_lh.max(word_lh);

        if self.try_merge_with_previous(word) {
            return;
        }

        self.current_segments.push(LineSegment {
            text: word.text.clone(),
            x_mm: self.current_x,
            width_mm: word.width_mm,
            style: word.style.clone(),
        });
        self.current_x += word.width_mm;
    }

    fn try_merge_with_previous(&mut self, word: &StyledWord) -> bool {
        let Some(last) = self.current_segments.last_mut() else { return false };
        if !last.style.same_as(&word.style) {
            return false;
        }
        // Merge if previous ends with space, or if this word is CJK (no space needed)
        let is_cjk_word = word.text.chars().next().map_or(false, is_cjk);
        let prev_ends_cjk = last.text.chars().last().map_or(false, is_cjk);
        if !last.text.ends_with(' ') && !is_cjk_word && !prev_ends_cjk {
            return false;
        }
        last.text.push_str(&word.text);
        last.width_mm = self.current_x + word.width_mm - last.x_mm;
        self.current_x += word.width_mm;
        true
    }

    fn trim_trailing_space(&mut self) {
        if let Some(last) = self.current_segments.last_mut() {
            if last.text.ends_with(' ') {
                last.text.pop();
            }
        }
    }

    fn finish_current_line(&mut self) {
        self.lines.push(LayoutLine {
            segments: std::mem::take(&mut self.current_segments),
            total_width_mm: self.current_x,
            max_line_height_mm: self.max_lh,
        });
        self.current_x = 0.0;
        self.max_lh = self.default_lh;
    }
}

pub(super) fn break_into_lines(words: &[StyledWord], max_width_mm: f64, default_lh: f64) -> Vec<LayoutLine> {
    LineBreaker::new(max_width_mm, default_lh).break_words(words)
}

// ═══════════════════════════════════════════════════════════════
//  Line emission helpers
// ═══════════════════════════════════════════════════════════════

pub(super) fn compute_align_offset(text_align: TextAlign, content_width: f64, line_width: f64) -> f64 {
    match text_align {
        TextAlign::Center => (content_width - line_width).max(0.0) / 2.0,
        TextAlign::Right => (content_width - line_width).max(0.0),
        _ => 0.0,
    }
}

pub(super) fn emit_line_segments(segments: &[LineSegment], y: f64, x_offset: f64, items: &mut Vec<LayoutItem>) {
    for seg in segments {
        items.push(LayoutItem::text_item(
            seg.x_mm + x_offset, y, seg.text.clone(), &seg.style,
        ));
    }
}

/// Characters that must not appear at the start of a line (行頭禁則).
/// Only fullwidth/CJK punctuation — ASCII punctuation is left to normal word wrap.
fn is_no_break_before(ch: char) -> bool {
    matches!(ch,
        '）' | '」' | '』' | '】' | '〉' | '》' | '〕' | '｝' | '］' |
        '。' | '、' | '，' | '．' | '！' | '？' | '：' | '；' |
        'ー' | '…' | '‥'
    )
}

/// Detect CJK characters that allow line breaking at character boundaries.
fn is_cjk(ch: char) -> bool {
    let cp = ch as u32;
    // CJK Unified Ideographs
    (0x4E00..=0x9FFF).contains(&cp)
    // CJK Extension A
    || (0x3400..=0x4DBF).contains(&cp)
    // CJK Compatibility Ideographs
    || (0xF900..=0xFAFF).contains(&cp)
    // Hiragana
    || (0x3040..=0x309F).contains(&cp)
    // Katakana
    || (0x30A0..=0x30FF).contains(&cp)
    // CJK Symbols and Punctuation
    || (0x3000..=0x303F).contains(&cp)
    // Fullwidth Forms
    || (0xFF00..=0xFFEF).contains(&cp)
    // Halfwidth Katakana
    || (0xFF65..=0xFF9F).contains(&cp)
    // CJK Extension B+
    || (0x20000..=0x2A6DF).contains(&cp)
}
