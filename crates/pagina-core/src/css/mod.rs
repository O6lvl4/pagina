pub mod parser;
pub mod values;

use std::collections::HashMap;
use values::*;

/// Resolved `@page` style.
#[derive(Debug, Clone)]
pub struct PageStyle {
    pub width_mm: f64,
    pub height_mm: f64,
    pub margin_top_mm: f64,
    pub margin_right_mm: f64,
    pub margin_bottom_mm: f64,
    pub margin_left_mm: f64,
    pub margin_boxes: HashMap<MarginBoxPosition, MarginBox>,
}

impl PageStyle {
    pub fn content_width_mm(&self) -> f64 {
        self.width_mm - self.margin_left_mm - self.margin_right_mm
    }

    pub fn content_height_mm(&self) -> f64 {
        self.height_mm - self.margin_top_mm - self.margin_bottom_mm
    }
}

impl Default for PageStyle {
    fn default() -> Self {
        Self {
            width_mm: 210.0,
            height_mm: 297.0,
            margin_top_mm: 25.0,
            margin_right_mm: 20.0,
            margin_bottom_mm: 25.0,
            margin_left_mm: 20.0,
            margin_boxes: HashMap::new(),
        }
    }
}

/// Content and style of a page-margin box.
#[derive(Debug, Clone)]
pub struct MarginBox {
    pub content: Vec<ContentItem>,
    pub font_size_pt: Option<f64>,
    pub color: Option<Color>,
    pub text_align: Option<TextAlign>,
}

/// Named page sizes in mm (width, height in portrait).
pub fn named_page_size(name: &str) -> Option<(f64, f64)> {
    Some(match name.to_ascii_lowercase().as_str() {
        "a3" => (297.0, 420.0),
        "a4" => (210.0, 297.0),
        "a5" => (148.0, 210.0),
        "b4" => (250.0, 353.0),
        "b5" => (176.0, 250.0),
        "letter" => (215.9, 279.4),
        "legal" => (215.9, 355.6),
        "ledger" => (279.4, 431.8),
        _ => return None,
    })
}

/// A parsed CSS rule: selector(s) + declarations.
#[derive(Debug, Clone)]
pub struct CssRule {
    pub selectors: Vec<Selector>,
    pub declarations: Vec<Declaration>,
}

/// A simple selector (matches a single element).
#[derive(Debug, Clone)]
pub enum SimpleSelector {
    Universal,
    Type(String),
    Class(String),
    Id(String),
    TypeAndClass(String, String),
}

impl SimpleSelector {
    pub fn specificity(&self) -> (u16, u16, u16) {
        match self {
            SimpleSelector::Universal => (0, 0, 0),
            SimpleSelector::Type(_) => (0, 0, 1),
            SimpleSelector::Class(_) => (0, 1, 0),
            SimpleSelector::Id(_) => (1, 0, 0),
            SimpleSelector::TypeAndClass(_, _) => (0, 1, 1),
        }
    }

    pub fn matches(&self, tag: &str, id: &Option<String>, classes: &[String]) -> bool {
        match self {
            SimpleSelector::Universal => true,
            SimpleSelector::Type(t) => t == tag,
            SimpleSelector::Class(c) => classes.iter().any(|cl| cl == c),
            SimpleSelector::Id(i) => id.as_deref() == Some(i.as_str()),
            SimpleSelector::TypeAndClass(t, c) => t == tag && classes.iter().any(|cl| cl == c),
        }
    }
}

/// Combinator between simple selectors.
#[derive(Debug, Clone, Copy)]
pub enum Combinator {
    /// ` ` (descendant)
    Descendant,
    /// `>` (child)
    Child,
}

/// A compound selector: a chain of simple selectors joined by combinators.
/// Read right-to-left: the last element is the subject.
#[derive(Debug, Clone)]
pub struct Selector {
    /// Chain of (combinator, simple_selector) pairs, from outermost ancestor to subject.
    /// The first entry has a dummy Descendant combinator (ignored).
    pub parts: Vec<(Combinator, SimpleSelector)>,
}

impl Selector {
    /// Create a simple (single-element) selector.
    pub fn simple(s: SimpleSelector) -> Self {
        Self { parts: vec![(Combinator::Descendant, s)] }
    }

    pub fn specificity(&self) -> (u16, u16, u16) {
        let mut a = 0u16;
        let mut b = 0u16;
        let mut c = 0u16;
        for (_, s) in &self.parts {
            let (sa, sb, sc) = s.specificity();
            a += sa;
            b += sb;
            c += sc;
        }
        (a, b, c)
    }

    /// The subject (rightmost) simple selector.
    pub fn subject(&self) -> &SimpleSelector {
        &self.parts.last().unwrap().1
    }

    /// Match this selector against an element with its ancestor chain.
    /// `ancestors` is ordered from closest parent to root.
    pub fn matches(
        &self,
        tag: &str,
        id: &Option<String>,
        classes: &[String],
        ancestors: &[AncestorInfo],
    ) -> bool {
        let n = self.parts.len();
        if n == 0 {
            return false;
        }

        // Subject must match
        if !self.parts[n - 1].1.matches(tag, id, classes) {
            return false;
        }

        if n == 1 {
            return true;
        }

        // Walk the ancestor chain for remaining parts (right-to-left)
        let mut ancestor_idx = 0;
        for part_idx in (0..n - 1).rev() {
            let (combinator, ref simple) = self.parts[part_idx];
            match combinator {
                Combinator::Child => {
                    // Must match the immediate parent
                    if ancestor_idx >= ancestors.len() {
                        return false;
                    }
                    let anc = &ancestors[ancestor_idx];
                    if !simple.matches(&anc.tag, &anc.id, &anc.classes) {
                        return false;
                    }
                    ancestor_idx += 1;
                }
                Combinator::Descendant => {
                    // Search up the ancestor chain for a match
                    let mut found = false;
                    while ancestor_idx < ancestors.len() {
                        let anc = &ancestors[ancestor_idx];
                        ancestor_idx += 1;
                        if simple.matches(&anc.tag, &anc.id, &anc.classes) {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        return false;
                    }
                }
            }
        }

        true
    }
}

/// Info about an ancestor element, for selector matching.
#[derive(Debug, Clone)]
pub struct AncestorInfo {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
}

/// A single CSS declaration.
#[derive(Debug, Clone)]
pub struct Declaration {
    pub property: String,
    pub value: String,
}

/// Resolved page style for a specific page type.
#[derive(Debug, Clone)]
pub struct PageStyleSet {
    pub base: PageStyle,
    pub first: Option<PageStyleOverride>,
    pub left: Option<PageStyleOverride>,
    pub right: Option<PageStyleOverride>,
}

impl Default for PageStyleSet {
    fn default() -> Self {
        Self {
            base: PageStyle::default(),
            first: None,
            left: None,
            right: None,
        }
    }
}

/// Override for specific page types (`:first`, `:left`, `:right`).
#[derive(Debug, Clone, Default)]
pub struct PageStyleOverride {
    pub margin_boxes: HashMap<MarginBoxPosition, MarginBox>,
    // Content `none` entries to suppress base margin boxes
    pub suppress_boxes: Vec<MarginBoxPosition>,
}

impl PageStyleSet {
    /// Get effective page style for a given page number (1-indexed).
    pub fn for_page(&self, page_num: usize, total_pages: usize) -> PageStyle {
        let mut style = self.base.clone();

        // Apply :first override
        if page_num == 1 {
            if let Some(first) = &self.first {
                for pos in &first.suppress_boxes {
                    style.margin_boxes.remove(pos);
                }
                for (pos, mb) in &first.margin_boxes {
                    style.margin_boxes.insert(*pos, mb.clone());
                }
            }
        }

        // Apply :left / :right (even pages are left in a left-to-right book)
        let is_left = page_num % 2 == 0;
        let side = if is_left { &self.left } else { &self.right };
        if let Some(s) = side {
            for pos in &s.suppress_boxes {
                style.margin_boxes.remove(pos);
            }
            for (pos, mb) in &s.margin_boxes {
                style.margin_boxes.insert(*pos, mb.clone());
            }
        }

        let _ = total_pages; // reserved for future use
        style
    }
}
