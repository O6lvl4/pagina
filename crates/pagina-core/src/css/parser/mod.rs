mod content;
mod page;
mod page_size;
mod selector;
mod tokens;
pub mod value_parsers;

use cssparser::{ParseError, Parser, ParserInput, Token};

use super::*;

// Re-export all public value parser functions at this module level
pub use value_parsers::{
    parse_break_value, parse_color_value, parse_display_value, parse_font_style_value,
    parse_font_weight_value, parse_length_value, parse_text_align_value,
};

// Re-export content parsing
pub use content::parse_content_value;

// ═══════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════

/// Parse a stylesheet: extract @page rules into `page_styles` and regular rules into `rules`.
pub fn parse_stylesheet(css: &str, page_styles: &mut PageStyleSet, rules: &mut Vec<CssRule>) {
    let mut input = ParserInput::new(css);
    let mut parser = Parser::new(&mut input);

    loop {
        let token_kind = match parser.next() {
            Ok(token) => classify_token(token),
            Err(_) => break,
        };
        dispatch_stylesheet_token(token_kind, &mut parser, page_styles, rules);
    }
}

fn dispatch_stylesheet_token(kind: TokenKind, parser: &mut Parser, page_styles: &mut PageStyleSet, rules: &mut Vec<CssRule>) {
    match kind {
        TokenKind::AtPage => page::parse_at_page(parser, page_styles),
        TokenKind::CurlyBlock => {
            let _ = parser.parse_nested_block(|_| -> Result<(), ParseError<'_, ()>> { Ok(()) });
        }
        TokenKind::Ident(name) => try_parse_qualified_rule(parser, &name, rules),
        TokenKind::Hash(id) => try_parse_qualified_rule(parser, &format!("#{id}"), rules),
        TokenKind::Dot => {
            if let Ok(class) = parser.expect_ident().map(|s| s.as_ref().to_owned()) {
                try_parse_qualified_rule(parser, &format!(".{class}"), rules);
            }
        }
        _ => {}
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
//  Token classification
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
//  Qualified rule parsing
// ═══════════════════════════════════════════════════════════════

fn try_parse_qualified_rule(parser: &mut Parser, first_token: &str, rules: &mut Vec<CssRule>) {
    let Some(selector_text) = collect_selector_text(parser, first_token) else { return };

    let selectors = selector::parse_selector_list(&selector_text);
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

fn collect_selector_text(parser: &mut Parser, first_token: &str) -> Option<String> {
    let mut text = first_token.to_owned();
    loop {
        match parser.next() {
            Ok(Token::CurlyBracketBlock) => return Some(text),
            Ok(ref token) => append_selector_token(&mut text, token),
            Err(_) => return None,
        }
    }
}

fn append_selector_token(text: &mut String, token: &Token) {
    match token {
        Token::Ident(s) => { text.push(' '); text.push_str(s.as_ref()); }
        Token::Delim('.') => text.push('.'),
        Token::IDHash(s) => { text.push('#'); text.push_str(s.as_ref()); }
        Token::Comma => text.push(','),
        Token::Colon => text.push(':'),
        Token::WhiteSpace(_) => text.push(' '),
        _ => {}
    }
}

// ═══════════════════════════════════════════════════════════════
//  Declaration list parsing
// ═══════════════════════════════════════════════════════════════

pub(crate) fn parse_declaration_list(parser: &mut Parser) -> Vec<Declaration> {
    let mut decls = Vec::new();
    while !parser.is_exhausted() {
        if let Some(decl) = try_parse_declaration(parser) {
            decls.push(decl);
        }
    }
    decls
}

fn try_parse_declaration(parser: &mut Parser) -> Option<Declaration> {
    let prop = match parser.expect_ident() {
        Ok(name) => name.as_ref().to_ascii_lowercase(),
        Err(_) => {
            let _ = parser.next();
            return None;
        }
    };

    if parser.expect_colon().is_err() {
        return None;
    }

    let value_parts = collect_value_tokens(parser);
    let value = value_parts.join(" ").trim().to_string();
    if value.is_empty() {
        return None;
    }
    Some(Declaration { property: prop, value })
}

fn collect_value_tokens(parser: &mut Parser) -> Vec<String> {
    let mut parts = Vec::new();
    loop {
        match parser.next() {
            Ok(Token::Semicolon) | Err(_) => break,
            Ok(token) => {
                if let Some(s) = tokens::value_token_to_string(token) {
                    parts.push(s);
                } else if let Token::Function(name) = token {
                    let fname = name.as_ref().to_string();
                    let inner = tokens::parse_function_args(parser);
                    parts.push(format!("{fname}({inner})"));
                }
            }
        }
    }
    parts
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
