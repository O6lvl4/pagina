//! Content property value parsing.

use cssparser::{ParseError, Parser, ParserInput, Token};

use super::super::values::*;

/// Parse a `content` value string into ContentItems.
pub fn parse_content_value(raw: &str) -> Vec<ContentItem> {
    let mut items = Vec::new();
    let mut input = ParserInput::new(raw);
    let mut parser = Parser::new(&mut input);

    while !parser.is_exhausted() {
        if let Some(item) = parse_next_content_item(&mut parser) {
            items.push(item);
        }
    }

    items
}

fn parse_next_content_item(parser: &mut Parser) -> Option<ContentItem> {
    match parser.next() {
        Ok(Token::QuotedString(s)) => Some(ContentItem::String(s.as_ref().to_string())),
        Ok(Token::Function(name)) => {
            let fname = name.as_ref().to_ascii_lowercase();
            parse_content_function(parser, &fname)
        }
        Ok(Token::Ident(name)) if name.eq_ignore_ascii_case("none") => {
            Some(ContentItem::None)
        }
        _ => None,
    }
}

fn parse_content_function(parser: &mut Parser, fname: &str) -> Option<ContentItem> {
    parser
        .parse_nested_block(|block| -> Result<ContentItem, ParseError<'_, ()>> {
            match fname {
                "counter" => parse_simple_function(block, ContentItem::Counter),
                "string" => parse_simple_function(block, ContentItem::RunningString),
                "attr" => parse_simple_function(block, ContentItem::Attr),
                "target-counter" => parse_target_counter(block),
                _ => Err(block.new_custom_error(())),
            }
        })
        .ok()
}

fn parse_simple_function<'i, F>(block: &mut Parser<'i, '_>, ctor: F) -> Result<ContentItem, ParseError<'i, ()>>
where
    F: FnOnce(String) -> ContentItem,
{
    let name = block.expect_ident()?.as_ref().to_owned();
    Ok(ctor(name))
}

fn parse_target_counter<'i>(block: &mut Parser<'i, '_>) -> Result<ContentItem, ParseError<'i, ()>> {
    let _fn_token = block.expect_function_matching("attr")?;
    let attr_name = block
        .parse_nested_block(|inner| -> Result<String, ParseError<'_, ()>> {
            Ok(inner.expect_ident()?.as_ref().to_owned())
        })?;
    let _ = block.expect_comma();
    let counter_name = block.expect_ident()?.as_ref().to_owned();
    Ok(ContentItem::TargetCounter(attr_name, counter_name))
}
