//! @page rule parsing and margin box extraction.

use cssparser::{BasicParseError, BasicParseErrorKind, ParseError, Parser, SourceLocation, Token};

use super::super::values::*;
use super::super::*;
use super::content::parse_content_value;
use super::page_size::{parse_margin, parse_size, skip_value, try_length_mm};
use super::value_parsers::{parse_color_value, parse_length_value, parse_text_align_value};

// ═══════════════════════════════════════════════════════════════
//  @page parsing
// ═══════════════════════════════════════════════════════════════

pub(super) fn parse_at_page(parser: &mut Parser, pss: &mut PageStyleSet) {
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

    if !skip_to_curly_block(parser) {
        return;
    }

    let _ = parser.parse_nested_block(|block| -> Result<(), ParseError<'_, ()>> {
        parse_page_block(block, pss, page_selector.as_deref());
        Ok(())
    });
}

fn parse_page_block(parser: &mut Parser, pss: &mut PageStyleSet, selector: Option<&str>) {
    while !parser.is_exhausted() {
        let next_kind = match parser.next() {
            Ok(t) => classify_page_block_token(t),
            Err(_) => break,
        };
        handle_page_block_token(next_kind, parser, pss, selector);
    }
}

fn handle_page_block_token(token: PageBlockToken, parser: &mut Parser, pss: &mut PageStyleSet, selector: Option<&str>) {
    match token {
        PageBlockToken::AtRule(name) => handle_page_at_rule(&name, parser, pss, selector),
        PageBlockToken::Ident(name) => handle_page_declaration(&name, parser, pss),
        PageBlockToken::Semicolon | PageBlockToken::Other => {}
    }
}

fn handle_page_at_rule(name: &str, parser: &mut Parser, pss: &mut PageStyleSet, selector: Option<&str>) {
    if let Some(pos) = MarginBoxPosition::from_name(name) {
        parse_margin_box_rule(parser, pss, selector, pos);
    } else {
        skip_at_rule(parser);
    }
}

fn handle_page_declaration(name: &str, parser: &mut Parser, pss: &mut PageStyleSet) {
    if parser.expect_colon().is_err() {
        return;
    }
    apply_page_declaration(name, parser, &mut pss.base);
    let _ = parser.try_parse(|p| p.expect_semicolon());
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
        "margin-top" => apply_page_margin_side(parser, &mut style.margin_top_mm),
        "margin-right" => apply_page_margin_side(parser, &mut style.margin_right_mm),
        "margin-bottom" => apply_page_margin_side(parser, &mut style.margin_bottom_mm),
        "margin-left" => apply_page_margin_side(parser, &mut style.margin_left_mm),
        _ => skip_value(parser),
    }
}

fn apply_page_margin_side(parser: &mut Parser, target: &mut f64) {
    if let Some(v) = try_length_mm(parser) {
        *target = v;
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
    if !skip_to_curly_block(parser) {
        return;
    }

    let _ = parser.parse_nested_block(|block| -> Result<(), ParseError<'_, ()>> {
        let parsed = parse_margin_box_declarations(block);
        apply_margin_box(pss, page_selector, pos, parsed);
        Ok(())
    });
}

struct ParsedMarginBox {
    content_items: Vec<ContentItem>,
    font_size: Option<f64>,
    color: Option<Color>,
    text_align: Option<TextAlign>,
    is_none: bool,
}

fn parse_margin_box_declarations(block: &mut Parser) -> ParsedMarginBox {
    let decls = super::parse_declaration_list(block);
    let mut result = ParsedMarginBox {
        content_items: Vec::new(),
        font_size: None,
        color: None,
        text_align: None,
        is_none: false,
    };

    for decl in &decls {
        apply_margin_box_declaration(&mut result, &decl.property, &decl.value);
    }
    result
}

fn apply_margin_box_declaration(result: &mut ParsedMarginBox, property: &str, value: &str) {
    match property {
        "content" if value.trim() == "none" => result.is_none = true,
        "content" => result.content_items = parse_content_value(value),
        "font-size" => result.font_size = parse_length_value(value).map(|l| l.to_pt(11.0)),
        "color" => result.color = parse_color_value(value),
        "text-align" => result.text_align = parse_text_align_value(value),
        _ => {}
    }
}

fn apply_margin_box(
    pss: &mut PageStyleSet,
    page_selector: Option<&str>,
    pos: MarginBoxPosition,
    parsed: ParsedMarginBox,
) {
    let mb = MarginBox {
        content: parsed.content_items,
        font_size_pt: parsed.font_size,
        color: parsed.color,
        text_align: parsed.text_align,
    };

    let is_override = matches!(page_selector, Some("first" | "left" | "right"));
    if is_override {
        apply_override_margin_box(pss, page_selector.unwrap(), pos, mb, parsed.is_none);
    } else {
        apply_base_margin_box(pss, pos, mb, parsed.is_none);
    }
}

fn apply_override_margin_box(pss: &mut PageStyleSet, sel: &str, pos: MarginBoxPosition, mb: MarginBox, is_none: bool) {
    let target = get_or_create_override(pss, sel);
    if is_none {
        target.suppress_boxes.push(pos);
    } else if !mb.content.is_empty() {
        target.margin_boxes.insert(pos, mb);
    }
}

fn apply_base_margin_box(pss: &mut PageStyleSet, pos: MarginBoxPosition, mb: MarginBox, is_none: bool) {
    if is_none {
        pss.base.margin_boxes.remove(&pos);
    } else if !mb.content.is_empty() {
        pss.base.margin_boxes.insert(pos, mb);
    }
}

fn get_or_create_override<'a>(pss: &'a mut PageStyleSet, selector: &str) -> &'a mut PageStyleOverride {
    match selector {
        "first" => pss.first.get_or_insert_with(PageStyleOverride::default),
        "left" => pss.left.get_or_insert_with(PageStyleOverride::default),
        "right" => pss.right.get_or_insert_with(PageStyleOverride::default),
        _ => unreachable!("only called with first/left/right"),
    }
}

// ═══════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════

fn skip_to_curly_block(parser: &mut Parser) -> bool {
    loop {
        match parser.next() {
            Ok(t) if matches!(t, Token::CurlyBracketBlock) => return true,
            Ok(_) => continue,
            Err(_) => return false,
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
