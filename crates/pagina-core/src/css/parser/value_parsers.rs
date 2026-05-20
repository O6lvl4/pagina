//! CSS value parsers (from string representations).

use super::super::values::*;

pub fn parse_length_value(s: &str) -> Option<Length> {
    let s = s.trim();
    if s == "0" {
        return Some(Length::Zero);
    }
    let (num_end, _) = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.' && *c != '-')?;
    let num: f64 = s[..num_end].parse().ok()?;
    let unit = s[num_end..].to_ascii_lowercase();
    match unit.as_str() {
        "mm" => Some(Length::Mm(num)),
        "cm" => Some(Length::Cm(num)),
        "in" => Some(Length::In(num)),
        "pt" => Some(Length::Pt(num)),
        "pc" => Some(Length::Pc(num)),
        "px" => Some(Length::Px(num)),
        "em" => Some(Length::Em(num)),
        "%" => Some(Length::Percent(num)),
        _ => None,
    }
}

pub fn parse_color_value(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.starts_with('#') {
        return Color::from_hex(s);
    }
    if s.starts_with("rgb") {
        return parse_rgb_color(s);
    }
    if s.starts_with("cmyk") || s.starts_with("device-cmyk") {
        return parse_cmyk_color(s);
    }
    Color::from_name(s)
}

fn extract_function_args(s: &str) -> Option<Vec<&str>> {
    let inner = s.split_once('(')?.1.strip_suffix(')')?.trim();
    Some(inner.split([',', ' ']).filter(|p| !p.is_empty()).collect())
}

fn parse_rgb_color(s: &str) -> Option<Color> {
    let parts = extract_function_args(s)?;
    if parts.len() < 3 {
        return None;
    }
    let r = parts[0].trim().parse().ok()?;
    let g = parts[1].trim().parse().ok()?;
    let b = parts[2].trim().parse().ok()?;
    let a = parts.get(3).and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
    Some(Color { r, g, b, a, cmyk: None })
}

fn parse_cmyk_color(s: &str) -> Option<Color> {
    let parts = extract_function_args(s)?;
    if parts.len() < 4 {
        return None;
    }
    let c = parse_cmyk_component(parts[0])?;
    let m = parse_cmyk_component(parts[1])?;
    let y = parse_cmyk_component(parts[2])?;
    let k = parse_cmyk_component(parts[3])?;
    Some(Color::cmyk(c, m, y, k))
}

fn parse_cmyk_component(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let v: f32 = pct.trim().parse().ok()?;
        Some(v / 100.0)
    } else {
        let v: f32 = s.parse().ok()?;
        Some(if v > 1.0 { v / 100.0 } else { v })
    }
}

pub fn parse_text_align_value(s: &str) -> Option<TextAlign> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "left" => TextAlign::Left,
        "center" => TextAlign::Center,
        "right" => TextAlign::Right,
        "justify" => TextAlign::Justify,
        _ => return None,
    })
}

pub fn parse_font_weight_value(s: &str) -> Option<FontWeight> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "bold" | "700" | "800" | "900" => FontWeight::Bold,
        "normal" | "400" | "100" | "200" | "300" => FontWeight::Normal,
        _ => return None,
    })
}

pub fn parse_font_style_value(s: &str) -> Option<FontStyle> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "italic" | "oblique" => FontStyle::Italic,
        "normal" => FontStyle::Normal,
        _ => return None,
    })
}

pub fn parse_break_value(s: &str) -> Option<BreakValue> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "page" | "always" => BreakValue::Page,
        "avoid" => BreakValue::Avoid,
        "auto" => BreakValue::Auto,
        _ => return None,
    })
}

pub fn parse_display_value(s: &str) -> Option<Display> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "block" => Display::Block,
        "inline" => Display::Inline,
        "none" => Display::None,
        "list-item" => Display::ListItem,
        _ => return None,
    })
}
