//! CSS selector parsing.

use super::super::{Combinator, Selector, SimpleSelector};

pub(super) fn parse_selector_list(text: &str) -> Vec<Selector> {
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
    let tokens = tokenize_selector(text);
    let parts = build_selector_parts(&tokens);

    match parts.len() {
        0 => Selector::simple(SimpleSelector::Universal),
        1 => {
            let (_, simple) = parts.into_iter().next().expect("len checked");
            Selector::simple(simple)
        }
        _ => Selector { parts },
    }
}

fn tokenize_selector(text: &str) -> Vec<&str> {
    let mut tokens = Vec::new();
    let mut rest = text.trim();
    while !rest.is_empty() {
        rest = rest.trim_start();
        if rest.starts_with('>') {
            tokens.push(">");
            rest = &rest[1..];
            continue;
        }
        let end = rest.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(rest.len());
        if end == 0 {
            break;
        }
        tokens.push(&rest[..end]);
        rest = &rest[end..];
    }
    tokens
}

fn build_selector_parts(tokens: &[&str]) -> Vec<(Combinator, SimpleSelector)> {
    let mut parts = Vec::new();
    let mut next_combinator = Combinator::Descendant;
    for token in tokens {
        if *token == ">" {
            next_combinator = Combinator::Child;
            continue;
        }
        parts.push((next_combinator, parse_simple_selector(token)));
        next_combinator = Combinator::Descendant;
    }
    parts
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
    if let Some((tag, class)) = s.split_once('.') {
        if !tag.is_empty() && !class.is_empty() {
            return SimpleSelector::TypeAndClass(tag.to_ascii_lowercase(), class.to_owned());
        }
    }
    SimpleSelector::Type(s.to_ascii_lowercase())
}
