//! CSS declaration application to computed styles.

use crate::css::parser;
use super::{ComputedStyle, StringSetSource};

pub(super) fn apply_declarations(style: &mut ComputedStyle, decls: &[crate::css::Declaration]) {
    for decl in decls {
        apply_single_declaration(style, decl);
    }
}

fn apply_single_declaration(style: &mut ComputedStyle, decl: &crate::css::Declaration) {
    let base_pt = style.font_size_pt;
    if !apply_font_declaration(style, &decl.property, &decl.value, base_pt) {
        apply_box_declaration(style, &decl.property, &decl.value, base_pt);
    }
}

/// Apply font/text-related declarations. Returns true if the property was handled.
fn apply_font_declaration(style: &mut ComputedStyle, prop: &str, value: &str, base_pt: f64) -> bool {
    match prop {
        "font-size" => apply_opt_length_pt(value, base_pt, &mut style.font_size_pt),
        "font-weight" => set_if_some(&mut style.font_weight, parser::parse_font_weight_value(value)),
        "font-style" => set_if_some(&mut style.font_style, parser::parse_font_style_value(value)),
        "color" => set_if_some(&mut style.color, parser::parse_color_value(value)),
        "text-align" => set_if_some(&mut style.text_align, parser::parse_text_align_value(value)),
        "line-height" => apply_line_height(style, value, base_pt),
        "display" => set_if_some(&mut style.display, parser::parse_display_value(value)),
        _ => return false,
    }
    true
}

fn set_if_some<T>(target: &mut T, value: Option<T>) {
    if let Some(v) = value {
        *target = v;
    }
}

fn apply_opt_length_pt(value: &str, base_pt: f64, target: &mut f64) {
    if let Some(len) = parser::parse_length_value(value) {
        *target = len.to_pt(base_pt);
    }
}

/// Apply box-model and other declarations.
fn apply_box_declaration(style: &mut ComputedStyle, prop: &str, value: &str, base_pt: f64) {
    if !apply_spacing_declaration(style, prop, value, base_pt) {
        apply_misc_declaration(style, prop, value);
    }
}

fn apply_spacing_declaration(style: &mut ComputedStyle, prop: &str, value: &str, base_pt: f64) -> bool {
    match prop {
        "margin" => apply_margin_shorthand(style, value, base_pt),
        "margin-top" => apply_opt_length_mm(value, base_pt, &mut style.margin_top_mm),
        "margin-bottom" => apply_opt_length_mm(value, base_pt, &mut style.margin_bottom_mm),
        "padding-top" => apply_opt_length_mm(value, base_pt, &mut style.padding_top_mm),
        "padding-bottom" => apply_opt_length_mm(value, base_pt, &mut style.padding_bottom_mm),
        "border-bottom" => apply_border_bottom(style, value, base_pt),
        _ => return false,
    }
    true
}

fn apply_opt_length_mm(value: &str, base_pt: f64, target: &mut f64) {
    if let Some(len) = parser::parse_length_value(value) {
        *target = len.to_mm(base_pt);
    }
}

fn apply_misc_declaration(style: &mut ComputedStyle, prop: &str, value: &str) {
    match prop {
        "break-before" | "page-break-before" => {
            set_if_some(&mut style.break_before, parser::parse_break_value(value));
        }
        "break-after" | "page-break-after" => {
            set_if_some(&mut style.break_after, parser::parse_break_value(value));
        }
        "string-set" => apply_string_set(style, value),
        "float" => {
            if value.trim() == "footnote" { style.is_footnote = true; }
        }
        "content" => {
            let items = parser::parse_content_value(value);
            if !items.is_empty() { style.content = Some(items); }
        }
        _ => {}
    }
}

fn apply_line_height(style: &mut ComputedStyle, value: &str, base_pt: f64) {
    if let Ok(v) = value.trim().parse::<f64>() {
        style.line_height = v;
    } else if let Some(len) = parser::parse_length_value(value) {
        style.line_height = len.to_pt(base_pt) / base_pt;
    }
}

fn apply_margin_shorthand(style: &mut ComputedStyle, value: &str, base_pt: f64) {
    if let Some(len) = parser::parse_length_value(value) {
        let mm = len.to_mm(base_pt);
        style.margin_top_mm = mm;
        style.margin_bottom_mm = mm;
    }
}

fn apply_string_set(style: &mut ComputedStyle, value: &str) {
    let parts: Vec<&str> = value.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let name = parts[0].to_string();
    let source = if parts[1].starts_with("attr(") {
        let attr = parts[1].strip_prefix("attr(").and_then(|s| s.strip_suffix(')'))
            .unwrap_or("title").to_string();
        StringSetSource::Attr(attr)
    } else {
        StringSetSource::Content
    };
    style.string_set = Some((name, source));
}

fn apply_border_bottom(style: &mut ComputedStyle, value: &str, base_pt: f64) {
    let parts: Vec<&str> = value.split_whitespace().collect();
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
