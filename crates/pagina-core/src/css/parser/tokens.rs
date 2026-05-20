//! CSS value token conversion and block token collection.

use cssparser::{ParseError, Parser, Token};

/// Convert a simple value token to its string representation.
pub(super) fn value_token_to_string(token: &Token) -> Option<String> {
    simple_token_string(token).or_else(|| composite_token_string(token))
}

fn simple_token_string(token: &Token) -> Option<String> {
    match token {
        Token::Ident(s) => Some(s.as_ref().to_string()),
        Token::QuotedString(s) => Some(format!("\"{}\"", s.as_ref())),
        Token::Number { value, .. } => Some(format!("{value}")),
        Token::Delim('/') => Some("/".to_string()),
        Token::Comma => Some(",".to_string()),
        _ => None,
    }
}

fn composite_token_string(token: &Token) -> Option<String> {
    match token {
        Token::Percentage { unit_value, .. } => Some(format!("{}%", unit_value * 100.0)),
        Token::Dimension { value, unit, .. } => Some(format!("{value}{}", unit.as_ref())),
        Token::Hash(s) | Token::IDHash(s) => Some(format!("#{}", s.as_ref())),
        _ => None,
    }
}

pub(super) fn parse_function_args(parser: &mut Parser) -> String {
    parser
        .parse_nested_block(|block| -> Result<String, ParseError<'_, ()>> {
            Ok(collect_block_tokens(block))
        })
        .unwrap_or_default()
}

fn collect_block_tokens(block: &mut Parser) -> String {
    let mut parts = Vec::new();
    loop {
        let classified = match block.next() {
            Ok(token) => classify_value_token(token),
            Err(_) => break,
        };
        match classified {
            ValueToken::Simple(s) => parts.push(s),
            ValueToken::NestedFunction(fname) => {
                let inner_args = collect_inner_function_idents(block);
                parts.push(format!("{fname}({inner_args})"));
            }
            ValueToken::Skip => {}
        }
    }
    parts.join(" ")
}

enum ValueToken {
    Simple(String),
    NestedFunction(String),
    Skip,
}

fn classify_value_token(token: &Token) -> ValueToken {
    match token {
        Token::Ident(s) => ValueToken::Simple(s.as_ref().to_string()),
        Token::QuotedString(s) => ValueToken::Simple(format!("\"{}\"", s.as_ref())),
        Token::Number { value, .. } => ValueToken::Simple(format!("{value}")),
        Token::Comma => ValueToken::Simple(",".to_string()),
        Token::Dimension { value, unit, .. } => {
            ValueToken::Simple(format!("{value}{}", unit.as_ref()))
        }
        Token::Function(name) => ValueToken::NestedFunction(name.as_ref().to_string()),
        _ => ValueToken::Skip,
    }
}

fn collect_inner_function_idents(block: &mut Parser) -> String {
    block
        .parse_nested_block(|ib| -> Result<String, ParseError<'_, ()>> {
            let mut ip = Vec::new();
            while let Ok(it) = ib.next() {
                if let Token::Ident(s) = it {
                    ip.push(s.as_ref().to_string());
                }
            }
            Ok(ip.join(" "))
        })
        .unwrap_or_default()
}
