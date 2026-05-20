use cssparser::{BasicParseError, BasicParseErrorKind, ParseError, Parser, ParserInput, SourceLocation, Token};

use super::values::*;
use super::*;

// ═══════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════

/// Parse a stylesheet: extract @page rules into `page_styles` and regular rules into `rules`.
pub fn parse_stylesheet(css: &str, page_styles: &mut PageStyleSet, rules: &mut Vec<CssRule>) {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    loop {
        let token_kind = {
            match parser.next() {
                Ok(token) => classify_token(token),
                Err(_) => break,
            }
        };

        match token_kind {
            TokenKind::AtPage => parse_at_page(&mut parser, page_styles),
            TokenKind::CurlyBlock => {
                // Stray block (e.g. from a qualified rule we couldn't parse) — skip
                let _ = parser.parse_nested_block(
                    |_| -> Result<(), ParseError<'_, ()>> { Ok(()) },
                );
            }
            TokenKind::Ident(name) => {
                // Start of a qualified rule (selector starts with ident)
                try_parse_qualified_rule(&mut parser, &name, rules);
            }
            TokenKind::Hash(id) => {
                try_parse_qualified_rule(&mut parser, &format!("#{id}"), rules);
            }
            TokenKind::Dot => {
                // .classname selector
                if let Ok(class) = parser.expect_ident().map(|s| s.as_ref().to_owned()) {
                    try_parse_qualified_rule(&mut parser, &format!(".{class}"), rules);
                }
            }
            _ => {}
        }
    }
}

/// Convenience: apply only @page rules (backwards compat).
pub fn apply_page_rules(css: &str, style: &mut PageStyle) {
    let mut pss = PageStyleSet {
        base: style.clone(),
        ..Default::default()
    };
    let mut rules = Vec::new();
    parse_stylesheet(css, &mut pss, &mut rules);
    *style = pss.base;
}

/// Parse an inline `style="..."` attribute into declarations.
pub fn parse_inline_style(css: &str) -> Vec<Declaration> {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);
    parse_declaration_list(&mut parser)
}

// ═══════════════════════════════════════════════════════════════
//  Token classification (avoids borrow issues)
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
enum TokenKind {
    AtPage,
    CurlyBlock,
    Ident(String),
    Hash(String),
    Dot,
    Other,
}

fn classify_token(token: &Token) -> TokenKind {
    match token {
        Token::AtKeyword(kw) if kw.eq_ignore_ascii_case("page") => TokenKind::AtPage,
        Token::CurlyBracketBlock => TokenKind::CurlyBlock,
        Token::Ident(name) => TokenKind::Ident(name.as_ref().to_owned()),
        Token::IDHash(id) => TokenKind::Hash(id.as_ref().to_owned()),
        Token::Delim('.') => TokenKind::Dot,
        _ => TokenKind::Other,
    }
}

// ═══════════════════════════════════════════════════════════════
//  @page parsing
// ═══════════════════════════════════════════════════════════════

fn parse_at_page(parser: &mut Parser, pss: &mut PageStyleSet) {
    // Optional page selector: :first, :left, :right
    let page_selector = parser
        .try_parse(|p| {
            let t = p.next()?.clone();
            match t {
                Token::Colon => {
                    let ident = p.expect_ident()?.as_ref().to_ascii_lowercase();
                    Ok::<_, BasicParseError<'_>>(ident)
                }
                _ => Err(BasicParseError {
                    kind: BasicParseErrorKind::QualifiedRuleInvalid,
                    location: SourceLocation { line: 0, column: 0 },
                }),
            }
        })
        .ok();

    // Expect CurlyBracketBlock
    let found_block = loop {
        match parser.next() {
            Ok(t) if matches!(t, Token::CurlyBracketBlock) => break true,
            Ok(_) => continue,
            Err(_) => break false,
        }
    };

    if !found_block {
        return;
    }

    let _ = parser.parse_nested_block(|block| -> Result<(), ParseError<'_, ()>> {
        parse_page_block(block, pss, page_selector.as_deref());
        Ok(())
    });
}

fn parse_page_block(parser: &mut Parser, pss: &mut PageStyleSet, selector: Option<&str>) {
    while !parser.is_exhausted() {
        // Check for nested at-rules (margin boxes)
        let next_kind = {
            match parser.next() {
                Ok(t) => classify_page_block_token(t),
                Err(_) => break,
            }
        };

        match next_kind {
            PageBlockToken::AtRule(name) => {
                if let Some(pos) = MarginBoxPosition::from_name(&name) {
                    parse_margin_box_rule(parser, pss, selector, pos);
                } else {
                    skip_at_rule(parser);
                }
            }
            PageBlockToken::Ident(name) => {
                // Declaration
                if parser.expect_colon().is_err() {
                    continue;
                }
                let style = match selector {
                    Some("first") | Some("left") | Some("right") => &mut pss.base,
                    _ => &mut pss.base,
                };
                apply_page_declaration(&name, parser, style);
                let _ = parser.try_parse(|p| p.expect_semicolon());
            }
            PageBlockToken::Semicolon => continue,
            PageBlockToken::Other => continue,
        }
    }
}

#[derive(Debug)]
enum PageBlockToken {
    AtRule(String),
    Ident(String),
    Semicolon,
    Other,
}

fn classify_page_block_token(token: &Token) -> PageBlockToken {
    match token {
        Token::AtKeyword(name) => PageBlockToken::AtRule(name.as_ref().to_owned()),
        Token::Ident(name) => PageBlockToken::Ident(name.as_ref().to_ascii_lowercase()),
        Token::Semicolon => PageBlockToken::Semicolon,
        _ => PageBlockToken::Other,
    }
}

fn apply_page_declaration(name: &str, parser: &mut Parser, style: &mut PageStyle) {
    match name {
        "size" => parse_size(parser, style),
        "margin" => parse_margin(parser, style),
        "margin-top" => {
            if let Some(v) = try_length_mm(parser) {
                style.margin_top_mm = v;
            }
        }
        "margin-right" => {
            if let Some(v) = try_length_mm(parser) {
                style.margin_right_mm = v;
            }
        }
        "margin-bottom" => {
            if let Some(v) = try_length_mm(parser) {
                style.margin_bottom_mm = v;
            }
        }
        "margin-left" => {
            if let Some(v) = try_length_mm(parser) {
                style.margin_left_mm = v;
            }
        }
        _ => skip_value(parser),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Margin box parsing
// ═══════════════════════════════════════════════════════════════

fn parse_margin_box_rule(
    parser: &mut Parser,
    pss: &mut PageStyleSet,
    page_selector: Option<&str>,
    pos: MarginBoxPosition,
) {
    // Expect CurlyBracketBlock
    let found = loop {
        match parser.next() {
            Ok(t) if matches!(t, Token::CurlyBracketBlock) => break true,
            Ok(_) => continue,
            Err(_) => break false,
        }
    };

    if !found {
        return;
    }

    let _ = parser.parse_nested_block(|block| -> Result<(), ParseError<'_, ()>> {
        let decls = parse_declaration_list(block);

        let mut content_items = Vec::new();
        let mut font_size = None;
        let mut color = None;
        let mut text_align = None;
        let mut is_none = false;

        for decl in &decls {
            match decl.property.as_str() {
                "content" => {
                    if decl.value.trim() == "none" {
                        is_none = true;
                    } else {
                        content_items = parse_content_value(&decl.value);
                    }
                }
                "font-size" => {
                    font_size = parse_length_value(&decl.value).map(|l| l.to_pt(11.0));
                }
                "color" => {
                    color = parse_color_value(&decl.value);
                }
                "text-align" => {
                    text_align = parse_text_align_value(&decl.value);
                }
                _ => {}
            }
        }

        let target = match page_selector {
            Some("first") => {
                if pss.first.is_none() {
                    pss.first = Some(PageStyleOverride::default());
                }
                pss.first.as_mut().unwrap()
            }
            Some("left") => {
                if pss.left.is_none() {
                    pss.left = Some(PageStyleOverride::default());
                }
                pss.left.as_mut().unwrap()
            }
            Some("right") => {
                if pss.right.is_none() {
                    pss.right = Some(PageStyleOverride::default());
                }
                pss.right.as_mut().unwrap()
            }
            _ => {
                // Base page: add directly to pss.base.margin_boxes
                if is_none {
                    pss.base.margin_boxes.remove(&pos);
                } else if !content_items.is_empty() {
                    pss.base.margin_boxes.insert(
                        pos,
                        MarginBox { content: content_items, font_size_pt: font_size, color, text_align },
                    );
                }
                return Ok(());
            }
        };

        if is_none {
            target.suppress_boxes.push(pos);
        } else if !content_items.is_empty() {
            target.margin_boxes.insert(
                pos,
                MarginBox { content: content_items, font_size_pt: font_size, color, text_align },
            );
        }

        Ok(())
    });
}

// ═══════════════════════════════════════════════════════════════
//  content property parsing
// ═══════════════════════════════════════════════════════════════

/// Parse a `content` value string into ContentItems.
pub fn parse_content_value(raw: &str) -> Vec<ContentItem> {
    let mut items = Vec::new();
    let mut input = ParserInput::new(raw);
    let mut parser = Parser::new(&mut input);

    while !parser.is_exhausted() {
        let item = {
            match parser.next() {
                Ok(Token::QuotedString(s)) => Some(ContentItem::String(s.as_ref().to_string())),
                Ok(Token::Function(name)) => {
                    let fname = name.as_ref().to_ascii_lowercase();
                    parse_content_function(&mut parser, &fname)
                }
                Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("none") => {
                    Some(ContentItem::None)
                }
                _ => None,
            }
        };
        if let Some(item) = item {
            items.push(item);
        }
    }

    items
}

fn parse_content_function(parser: &mut Parser, fname: &str) -> Option<ContentItem> {
    parser
        .parse_nested_block(|block| -> Result<ContentItem, ParseError<'_, ()>> {
            match fname {
                "counter" => {
                    let name = block.expect_ident()?.as_ref().to_owned();
                    Ok(ContentItem::Counter(name))
                }
                "string" => {
                    let name = block.expect_ident()?.as_ref().to_owned();
                    Ok(ContentItem::RunningString(name))
                }
                "attr" => {
                    let name = block.expect_ident()?.as_ref().to_owned();
                    Ok(ContentItem::Attr(name))
                }
                "target-counter" => {
                    // target-counter(attr(href), page)
                    let _fn_token = block.expect_function_matching("attr")?;
                    let attr_name = block
                        .parse_nested_block(|inner| -> Result<String, ParseError<'_, ()>> {
                            Ok(inner.expect_ident()?.as_ref().to_owned())
                        })?;
                    let _ = block.expect_comma();
                    let counter_name = block.expect_ident()?.as_ref().to_owned();
                    Ok(ContentItem::TargetCounter(attr_name, counter_name))
                }
                _ => Err(block.new_custom_error(())),
            }
        })
        .ok()
}

// ═══════════════════════════════════════════════════════════════
//  Qualified rule (selector + block) parsing
// ═══════════════════════════════════════════════════════════════

fn try_parse_qualified_rule(parser: &mut Parser, first_token: &str, rules: &mut Vec<CssRule>) {
    // Collect the rest of the selector text
    let mut selector_text = first_token.to_owned();
    loop {
        match parser.next() {
            Ok(Token::CurlyBracketBlock) => break,
            Ok(Token::Ident(s)) => {
                selector_text.push(' ');
                selector_text.push_str(s.as_ref());
            }
            Ok(Token::Delim('.')) => selector_text.push('.'),
            Ok(Token::IDHash(s)) => {
                selector_text.push('#');
                selector_text.push_str(s.as_ref());
            }
            Ok(Token::Comma) => selector_text.push(','),
            Ok(Token::Colon) => selector_text.push(':'),
            Ok(Token::WhiteSpace(_)) => selector_text.push(' '),
            Err(_) => return,
            _ => continue,
        }
    }

    let selectors = parse_selector_list(&selector_text);
    if selectors.is_empty() {
        let _ = parser.parse_nested_block(|_| -> Result<(), ParseError<'_, ()>> { Ok(()) });
        return;
    }

    let declarations =
        parser
            .parse_nested_block(|block| -> Result<Vec<Declaration>, ParseError<'_, ()>> {
                Ok(parse_declaration_list(block))
            })
            .unwrap_or_default();

    if !declarations.is_empty() {
        rules.push(CssRule { selectors, declarations });
    }
}

fn parse_selector_list(text: &str) -> Vec<Selector> {
    text.split(',')
        .filter_map(|s| {
            let s = s.trim();
            if s.is_empty() {
                return None;
            }
            Some(parse_compound_selector(s))
        })
        .collect()
}

/// Parse a single compound selector (e.g. ".toc > a", "table td.highlight").
fn parse_compound_selector(text: &str) -> Selector {
    // Tokenize: split by whitespace and `>`, preserving `>` as a token
    let mut tokens: Vec<&str> = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.starts_with('>') {
            tokens.push(">");
            rest = &rest[1..];
        } else {
            let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
            if end > 0 {
                tokens.push(&rest[..end]);
                rest = &rest[end..];
            } else {
                break;
            }
        }
    }

    let mut parts = Vec::new();
    let mut next_combinator = Combinator::Descendant;

    for token in &tokens {
        if *token == ">" {
            next_combinator = Combinator::Child;
            continue;
        }
        let simple = parse_simple_selector(token);
        parts.push((next_combinator, simple));
        next_combinator = Combinator::Descendant;
    }

    if parts.is_empty() {
        Selector::simple(SimpleSelector::Universal)
    } else if parts.len() == 1 {
        Selector::simple(parts.pop().unwrap().1)
    } else {
        Selector { parts }
    }
}

fn parse_simple_selector(s: &str) -> SimpleSelector {
    if s == "*" {
        return SimpleSelector::Universal;
    }
    if let Some(id) = s.strip_prefix('#') {
        return SimpleSelector::Id(id.to_owned());
    }
    if let Some(class) = s.strip_prefix('.') {
        return SimpleSelector::Class(class.to_owned());
    }
    // tag.class
    if let Some((tag, class)) = s.split_once('.') {
        if !tag.is_empty() && !class.is_empty() {
            return SimpleSelector::TypeAndClass(tag.to_ascii_lowercase(), class.to_owned());
        }
    }
    SimpleSelector::Type(s.to_ascii_lowercase())
}

// ═══════════════════════════════════════════════════════════════
//  Declaration list parsing
// ═══════════════════════════════════════════════════════════════

fn parse_declaration_list(parser: &mut Parser) -> Vec<Declaration> {
    let mut decls = Vec::new();
    while !parser.is_exhausted() {
        let prop = match parser.expect_ident() {
            Ok(name) => name.as_ref().to_ascii_lowercase(),
            Err(_) => {
                let _ = parser.next();
                continue;
            }
        };

        if parser.expect_colon().is_err() {
            continue;
        }

        let mut value_parts = Vec::new();
        loop {
            match parser.next() {
                Ok(Token::Semicolon) | Err(_) => break,
                Ok(Token::Ident(s)) => value_parts.push(s.as_ref().to_string()),
                Ok(Token::QuotedString(s)) => {
                    value_parts.push(format!("\"{}\"", s.as_ref()));
                }
                Ok(Token::Number { value, .. }) => {
                    value_parts.push(format!("{value}"));
                }
                Ok(Token::Percentage { unit_value, .. }) => {
                    value_parts.push(format!("{}%", unit_value * 100.0));
                }
                Ok(Token::Dimension { value, unit, .. }) => {
                    value_parts.push(format!("{value}{}", unit.as_ref()));
                }
                Ok(Token::Hash(s)) | Ok(Token::IDHash(s)) => {
                    value_parts.push(format!("#{}", s.as_ref()));
                }
                Ok(Token::Function(name)) => {
                    let fname = name.as_ref().to_string();
                    let inner = parser
                        .parse_nested_block(|block| -> Result<String, ParseError<'_, ()>> {
                            let mut parts = Vec::new();
                            while let Ok(t) = block.next() {
                                match t {
                                    Token::Ident(s) => parts.push(s.as_ref().to_string()),
                                    Token::QuotedString(s) => {
                                        parts.push(format!("\"{}\"", s.as_ref()))
                                    }
                                    Token::Number { value, .. } => {
                                        parts.push(format!("{value}"))
                                    }
                                    Token::Comma => parts.push(",".to_string()),
                                    Token::Dimension { value, unit, .. } => {
                                        parts.push(format!("{value}{}", unit.as_ref()))
                                    }
                                    Token::Function(inner_name) => {
                                        let inner_fname = inner_name.as_ref().to_string();
                                        let inner_args = block
                                            .parse_nested_block(
                                                |ib| -> Result<String, ParseError<'_, ()>> {
                                                    let mut ip = Vec::new();
                                                    while let Ok(it) = ib.next() {
                                                        match it {
                                                            Token::Ident(s) => {
                                                                ip.push(s.as_ref().to_string())
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                    Ok(ip.join(" "))
                                                },
                                            )
                                            .unwrap_or_default();
                                        parts.push(format!("{inner_fname}({inner_args})"));
                                    }
                                    _ => {}
                                }
                            }
                            Ok(parts.join(" "))
                        })
                        .unwrap_or_default();
                    value_parts.push(format!("{fname}({inner})"));
                }
                Ok(Token::Delim('/')) => value_parts.push("/".to_string()),
                Ok(Token::Comma) => value_parts.push(",".to_string()),
                _ => {}
            }
        }

        let value = value_parts.join(" ").trim().to_string();
        if !value.is_empty() {
            decls.push(Declaration { property: prop, value });
        }
    }
    decls
}

// ═══════════════════════════════════════════════════════════════
//  CSS value parsers (from string)
// ═══════════════════════════════════════════════════════════════

pub fn parse_length_value(s: &str) -> Option<Length> {
    let s = s.trim();
    if s == "0" {
        return Some(Length::Zero);
    }
    // Try to parse `<number><unit>`
    let (num_end, _) = s
        .char_indices()
        .find(|(_, c)| !c.is_ascii_digit() && *c != '.' && *c != '-')?;
    let num: f64 = s[..num_end].parse().ok()?;
    let unit = &s[num_end..];
    Some(match unit.to_ascii_lowercase().as_str() {
        "mm" => Length::Mm(num),
        "cm" => Length::Cm(num),
        "in" => Length::In(num),
        "pt" => Length::Pt(num),
        "pc" => Length::Pc(num),
        "px" => Length::Px(num),
        "em" => Length::Em(num),
        "%" => Length::Percent(num),
        _ => return None,
    })
}

pub fn parse_color_value(s: &str) -> Option<Color> {
    let s = s.trim();
    if s.starts_with('#') {
        return Color::from_hex(s);
    }
    if s.starts_with("rgb") {
        // rgb(r, g, b) or rgba(r, g, b, a)
        let inner = s.split_once('(')?.1.strip_suffix(')')?.trim();
        let parts: Vec<&str> = inner.split([',', ' ']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 3 {
            let r = parts[0].trim().parse().ok()?;
            let g = parts[1].trim().parse().ok()?;
            let b = parts[2].trim().parse().ok()?;
            let a = parts.get(3).and_then(|s| s.trim().parse().ok()).unwrap_or(1.0);
            return Some(Color { r, g, b, a, cmyk: None });
        }
    }
    // cmyk(c, m, y, k) or device-cmyk(c, m, y, k)
    if s.starts_with("cmyk") || s.starts_with("device-cmyk") {
        let inner = s.split_once('(')?.1.strip_suffix(')')?.trim();
        let parts: Vec<&str> = inner.split([',', ' ']).filter(|p| !p.is_empty()).collect();
        if parts.len() >= 4 {
            let c: f32 = parse_cmyk_component(parts[0])?;
            let m: f32 = parse_cmyk_component(parts[1])?;
            let y: f32 = parse_cmyk_component(parts[2])?;
            let k: f32 = parse_cmyk_component(parts[3])?;
            return Some(Color::cmyk(c, m, y, k));
        }
    }
    Color::from_name(s)
}

fn parse_cmyk_component(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(pct) = s.strip_suffix('%') {
        let v: f32 = pct.trim().parse().ok()?;
        Some(v / 100.0)
    } else {
        let v: f32 = s.parse().ok()?;
        // Normalize: if > 1.0, treat as percentage
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

// ═══════════════════════════════════════════════════════════════
//  @page size / margin parsing (low-level, uses cssparser Parser)
// ═══════════════════════════════════════════════════════════════

fn parse_size(parser: &mut Parser, style: &mut PageStyle) {
    if let Ok(name) = parser.try_parse(|p| {
        let s = p.expect_ident()?.as_ref().to_owned();
        Ok::<_, BasicParseError<'_>>(s)
    }) {
        if let Some((w, h)) = named_page_size(&name) {
            let landscape = parser
                .try_parse(|p| {
                    let o = p.expect_ident()?.as_ref().to_ascii_lowercase();
                    Ok::<_, BasicParseError<'_>>(o == "landscape")
                })
                .unwrap_or(false);
            if landscape {
                style.width_mm = h;
                style.height_mm = w;
            } else {
                style.width_mm = w;
                style.height_mm = h;
            }
            return;
        }
    }
    if let Some(w) = try_length_mm(parser) {
        let h = try_length_mm(parser).unwrap_or(w);
        style.width_mm = w;
        style.height_mm = h;
    }
}

fn parse_margin(parser: &mut Parser, style: &mut PageStyle) {
    let mut values = Vec::with_capacity(4);
    for _ in 0..4 {
        match try_length_mm(parser) {
            Some(v) => values.push(v),
            None => break,
        }
    }
    match values.len() {
        1 => {
            style.margin_top_mm = values[0];
            style.margin_right_mm = values[0];
            style.margin_bottom_mm = values[0];
            style.margin_left_mm = values[0];
        }
        2 => {
            style.margin_top_mm = values[0];
            style.margin_bottom_mm = values[0];
            style.margin_right_mm = values[1];
            style.margin_left_mm = values[1];
        }
        3 => {
            style.margin_top_mm = values[0];
            style.margin_right_mm = values[1];
            style.margin_bottom_mm = values[2];
            style.margin_left_mm = values[1];
        }
        4 => {
            style.margin_top_mm = values[0];
            style.margin_right_mm = values[1];
            style.margin_bottom_mm = values[2];
            style.margin_left_mm = values[3];
        }
        _ => {}
    }
}

fn try_length_mm(parser: &mut Parser) -> Option<f64> {
    parser
        .try_parse(|p| {
            let token = p.next()?.clone();
            match token {
                Token::Dimension { value, ref unit, .. } => {
                    length_to_mm(value, unit.as_ref()).ok_or(BasicParseError {
                        kind: BasicParseErrorKind::QualifiedRuleInvalid,
                        location: SourceLocation { line: 0, column: 0 },
                    })
                }
                Token::Number { value, .. } if value == 0.0 => Ok(0.0),
                other => Err(BasicParseError {
                    kind: BasicParseErrorKind::UnexpectedToken(other),
                    location: SourceLocation { line: 0, column: 0 },
                }),
            }
        })
        .ok()
}

fn length_to_mm(value: f32, unit: &str) -> Option<f64> {
    let v = value as f64;
    Some(match unit.to_ascii_lowercase().as_str() {
        "mm" => v,
        "cm" => v * 10.0,
        "in" => v * 25.4,
        "pt" => v * 25.4 / 72.0,
        "pc" => v * 25.4 / 6.0,
        "px" => v * 25.4 / 96.0,
        _ => return None,
    })
}

fn skip_value(parser: &mut Parser) {
    while !parser.is_exhausted() {
        match parser.next() {
            Ok(Token::Semicolon) | Err(_) => break,
            _ => {}
        }
    }
}

fn skip_at_rule(parser: &mut Parser) {
    loop {
        match parser.next() {
            Ok(Token::CurlyBracketBlock) => {
                let _ = parser
                    .parse_nested_block(|_| -> Result<(), ParseError<'_, ()>> { Ok(()) });
                break;
            }
            Ok(Token::Semicolon) | Err(_) => break,
            _ => {}
        }
    }
}

// ═══════════════════════════════════════════════════════════════
//  Tests
// ═══════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_a4() {
        let mut style = PageStyle::default();
        apply_page_rules("@page { size: A4; }", &mut style);
        assert!((style.width_mm - 210.0).abs() < 0.01);
        assert!((style.height_mm - 297.0).abs() < 0.01);
    }

    #[test]
    fn parse_a4_landscape() {
        let mut style = PageStyle::default();
        apply_page_rules("@page { size: A4 landscape; }", &mut style);
        assert!((style.width_mm - 297.0).abs() < 0.01);
        assert!((style.height_mm - 210.0).abs() < 0.01);
    }

    #[test]
    fn parse_margin_shorthand() {
        let mut style = PageStyle::default();
        apply_page_rules("@page { margin: 10mm 20mm; }", &mut style);
        assert!((style.margin_top_mm - 10.0).abs() < 0.01);
        assert!((style.margin_right_mm - 20.0).abs() < 0.01);
    }

    #[test]
    fn parse_margin_boxes() {
        let mut pss = PageStyleSet::default();
        let mut rules = Vec::new();
        parse_stylesheet(
            r#"@page {
                size: A4;
                @top-center { content: "Header"; font-size: 9pt; }
                @bottom-center { content: counter(page) " / " counter(pages); }
            }"#,
            &mut pss,
            &mut rules,
        );
        assert!(pss.base.margin_boxes.contains_key(&MarginBoxPosition::TopCenter));
        assert!(pss.base.margin_boxes.contains_key(&MarginBoxPosition::BottomCenter));
    }

    #[test]
    fn parse_css_rules() {
        let mut pss = PageStyleSet::default();
        let mut rules = Vec::new();
        parse_stylesheet(
            "h1 { font-size: 24pt; color: navy; } .note { font-size: 9pt; }",
            &mut pss,
            &mut rules,
        );
        assert_eq!(rules.len(), 2);
        assert!(matches!(rules[0].selectors[0].subject(), SimpleSelector::Type(t) if t == "h1"));
        assert!(matches!(rules[1].selectors[0].subject(), SimpleSelector::Class(c) if c == "note"));
    }

    #[test]
    fn parse_content_items() {
        let items = parse_content_value(r#""Chapter " counter(page) " of " counter(pages)"#);
        assert_eq!(items.len(), 4);
        assert!(matches!(&items[0], ContentItem::String(s) if s == "Chapter "));
        assert!(matches!(&items[1], ContentItem::Counter(c) if c == "page"));
    }

    #[test]
    fn parse_first_page_override() {
        let mut pss = PageStyleSet::default();
        let mut rules = Vec::new();
        parse_stylesheet(
            r#"
            @page {
                @top-center { content: "Header"; }
            }
            @page :first {
                @top-center { content: none; }
            }"#,
            &mut pss,
            &mut rules,
        );
        assert!(pss.base.margin_boxes.contains_key(&MarginBoxPosition::TopCenter));
        assert!(pss.first.is_some());
        assert!(pss.first.as_ref().unwrap().suppress_boxes.contains(&MarginBoxPosition::TopCenter));
    }
}
