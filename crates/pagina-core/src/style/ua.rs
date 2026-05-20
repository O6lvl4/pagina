//! User-agent default styles for HTML elements.

use crate::css::values::*;
use super::ComputedStyle;

/// UA defaults for common HTML tags.
pub(super) fn ua_style(tag: &str) -> ComputedStyle {
    let mut s = ComputedStyle::default();
    apply_ua_defaults(&mut s, tag);
    s
}

fn apply_ua_defaults(s: &mut ComputedStyle, tag: &str) {
    if !apply_ua_heading(s, tag) && !apply_ua_inline(s, tag) {
        apply_ua_block(s, tag);
    }
}

fn apply_ua_heading(s: &mut ComputedStyle, tag: &str) -> bool {
    let (size, mt, mb) = match tag {
        "h1" => (26.0, 6.0, 4.0),
        "h2" => (20.0, 5.0, 3.0),
        "h3" => (16.0, 4.0, 2.5),
        "h4" => (14.0, 3.0, 2.0),
        "h5" | "h6" => (12.0, 2.5, 1.5),
        _ => return false,
    };
    s.font_size_pt = size;
    s.font_weight = FontWeight::Bold;
    s.margin_top_mm = mt;
    s.margin_bottom_mm = mb;
    true
}

fn apply_ua_inline(s: &mut ComputedStyle, tag: &str) -> bool {
    match tag {
        "code" | "kbd" | "samp" => { s.font_family = "Courier".to_string(); s.display = Display::Inline; }
        "strong" | "b" => { s.font_weight = FontWeight::Bold; s.display = Display::Inline; }
        "em" | "i" => { s.font_style = FontStyle::Italic; s.display = Display::Inline; }
        "span" | "a" | "abbr" | "small" | "sub" | "sup" => { s.display = Display::Inline; }
        _ => return false,
    }
    true
}

fn apply_ua_block(s: &mut ComputedStyle, tag: &str) {
    match tag {
        "li" => apply_ua_li(s),
        "hr" => apply_ua_hr(s),
        "th" => apply_ua_th(s),
        _ => apply_ua_block_from_table(s, tag),
    }
}

fn apply_ua_li(s: &mut ComputedStyle) {
    s.display = Display::ListItem;
    s.margin_bottom_mm = 1.5;
}

fn apply_ua_hr(s: &mut ComputedStyle) {
    s.margin_top_mm = 4.0;
    s.margin_bottom_mm = 4.0;
    s.border_bottom_width_mm = 0.2;
}

fn apply_ua_th(s: &mut ComputedStyle) {
    s.padding_top_mm = 1.0;
    s.padding_bottom_mm = 1.0;
    s.font_weight = FontWeight::Bold;
}

/// Table-driven block defaults: (tag, margin_top, margin_bottom, padding_top, padding_bottom, font_family, font_size).
/// Zero values mean "keep default".
const BLOCK_DEFAULTS: &[(&str, f64, f64, f64, f64, &str, f64)] = &[
    ("p",          0.0, 3.5, 0.0, 0.0, "",        0.0),
    ("blockquote", 3.0, 3.0, 0.0, 0.0, "",        0.0),
    ("pre",        2.0, 2.0, 0.0, 0.0, "Courier", 10.0),
    ("table",      3.0, 3.0, 0.0, 0.0, "",        0.0),
    ("td",         0.0, 0.0, 1.0, 1.0, "",        0.0),
];

fn apply_ua_block_from_table(s: &mut ComputedStyle, tag: &str) {
    let Some(entry) = BLOCK_DEFAULTS.iter().find(|(t, ..)| *t == tag) else { return };
    apply_block_entry(s, entry);
}

fn apply_block_entry(s: &mut ComputedStyle, e: &(&str, f64, f64, f64, f64, &str, f64)) {
    if e.1 != 0.0 { s.margin_top_mm = e.1; }
    if e.2 != 0.0 { s.margin_bottom_mm = e.2; }
    if e.3 != 0.0 { s.padding_top_mm = e.3; }
    if e.4 != 0.0 { s.padding_bottom_mm = e.4; }
    if !e.5.is_empty() { s.font_family = e.5.to_string(); }
    if e.6 != 0.0 { s.font_size_pt = e.6; }
}
