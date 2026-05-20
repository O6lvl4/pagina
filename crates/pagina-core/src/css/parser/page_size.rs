//! @page size and margin low-level parsing with cssparser.

use cssparser::{BasicParseError, BasicParseErrorKind, Parser, SourceLocation, Token};

use super::super::*;

pub(super) fn parse_size(parser: &mut Parser, style: &mut PageStyle) {
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

pub(super) fn parse_margin(parser: &mut Parser, style: &mut PageStyle) {
    let mut values = Vec::with_capacity(4);
    for _ in 0..4 {
        match try_length_mm(parser) {
            Some(v) => values.push(v),
            None => break,
        }
    }
    apply_margin_values(style, &values);
}

fn apply_margin_values(style: &mut PageStyle, values: &[f64]) {
    let (top, right, bottom, left) = match *values {
        [v] => (v, v, v, v),
        [tb, lr] => (tb, lr, tb, lr),
        [t, lr, b] => (t, lr, b, lr),
        [t, r, b, l] => (t, r, b, l),
        _ => return,
    };
    style.margin_top_mm = top;
    style.margin_right_mm = right;
    style.margin_bottom_mm = bottom;
    style.margin_left_mm = left;
}

pub(super) fn try_length_mm(parser: &mut Parser) -> Option<f64> {
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

const LENGTH_TO_MM_TABLE: &[(&str, f64, f64)] = &[
    ("mm", 1.0, 1.0),
    ("cm", 10.0, 1.0),
    ("in", 25.4, 1.0),
    ("pt", 25.4, 72.0),
    ("pc", 25.4, 6.0),
    ("px", 25.4, 96.0),
];

fn length_to_mm(value: f32, unit: &str) -> Option<f64> {
    let v = value as f64;
    let lower = unit.to_ascii_lowercase();
    LENGTH_TO_MM_TABLE.iter()
        .find(|(u, _, _)| *u == lower)
        .map(|(_, mul, div)| v * mul / div)
}

pub(super) fn skip_value(parser: &mut Parser) {
    while !parser.is_exhausted() {
        match parser.next() {
            Ok(Token::Semicolon) | Err(_) => break,
            _ => {}
        }
    }
}
