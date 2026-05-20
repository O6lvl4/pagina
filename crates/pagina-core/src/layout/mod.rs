mod blocks;
mod content;
mod line_break;
mod state;
mod table_image;

use crate::css::values::*;
use crate::css::PageStyleSet;
use crate::font::FontManager;
use crate::style::ComputedStyle;

use state::LayoutState;

// ═══════════════════════════════════════════════════════════════
//  Layout types
// ═══════════════════════════════════════════════════════════════

#[derive(Debug)]
pub struct Page {
    pub items: Vec<LayoutItem>,
    pub footnotes: Vec<LayoutItem>,
    pub margin_boxes: Vec<ResolvedMarginBox>,
    pub bookmarks: Vec<Bookmark>,
    pub links: Vec<LinkAnnotation>,
}

impl Page {
    fn new() -> Self {
        Self {
            items: Vec::new(),
            footnotes: Vec::new(),
            margin_boxes: Vec::new(),
            bookmarks: Vec::new(),
            links: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Bookmark {
    pub title: String,
    pub level: u8,
    pub y_mm: f64,
}

#[derive(Debug, Clone)]
pub struct LinkAnnotation {
    pub x_mm: f64,
    pub y_mm: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub target: LinkTarget,
}

#[derive(Debug, Clone)]
pub enum LinkTarget {
    Uri(String),
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct LayoutItem {
    pub x_mm: f64,
    pub y_mm: f64,
    pub font_size_pt: f64,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub font_family: String,
    pub color: Color,
    pub text: String,
    pub kind: ItemKind,
}

impl LayoutItem {
    fn text_item(x: f64, y: f64, text: String, style: &InlineStyle) -> Self {
        Self {
            x_mm: x, y_mm: y,
            font_size_pt: style.font_size_pt,
            font_weight: style.font_weight,
            font_style: style.font_style,
            font_family: style.font_family.clone(),
            color: style.color, text,
            kind: ItemKind::Text,
        }
    }

    pub(crate) fn hr_item(pos: (f64, f64), kind: ItemKind) -> Self {
        Self {
            x_mm: pos.0, y_mm: pos.1,
            font_size_pt: 0.0, font_weight: FontWeight::Normal, font_style: FontStyle::Normal,
            font_family: String::new(), color: Color::BLACK, text: String::new(), kind,
        }
    }

    pub(crate) fn image_item(pos: (f64, f64), kind: ItemKind) -> Self {
        Self {
            x_mm: pos.0, y_mm: pos.1,
            font_size_pt: 0.0, font_weight: FontWeight::Normal, font_style: FontStyle::Normal,
            font_family: String::new(), color: Color::BLACK, text: String::new(), kind,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ItemKind {
    Text,
    HorizontalRule { width_mm: f64, thickness_mm: f64, color: Color },
    Image { id: usize, width_mm: f64, height_mm: f64 },
}

#[derive(Debug)]
pub struct ResolvedMarginBox {
    pub position: MarginBoxPosition,
    pub text: String,
    pub font_size_pt: f64,
    pub color: Color,
    pub text_align: TextAlign,
}

/// An image loaded from the document, ready for embedding.
#[derive(Debug)]
pub struct LoadedImage {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

// ═══════════════════════════════════════════════════════════════
//  Inline style (shared between submodules)
// ═══════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
struct InlineStyle {
    font_size_pt: f64,
    font_weight: FontWeight,
    font_style: FontStyle,
    font_family: String,
    color: Color,
}

impl InlineStyle {
    fn from_computed(s: &ComputedStyle) -> Self {
        Self {
            font_size_pt: s.font_size_pt, font_weight: s.font_weight,
            font_style: s.font_style, font_family: s.font_family.clone(), color: s.color,
        }
    }

    fn same_as(&self, other: &Self) -> bool {
        self.font_size_pt == other.font_size_pt
            && self.font_weight == other.font_weight
            && self.font_style == other.font_style
            && self.font_family == other.font_family
            && self.color.r == other.color.r
            && self.color.g == other.color.g
            && self.color.b == other.color.b
    }
}

/// A word (or non-breakable token) with its style.
#[derive(Debug, Clone)]
struct StyledWord {
    text: String,
    style: InlineStyle,
    width_mm: f64,
}

// ═══════════════════════════════════════════════════════════════
//  Main layout entry point
// ═══════════════════════════════════════════════════════════════

pub fn lay_out(page_styles: &PageStyleSet, tree: &crate::style::StyledNode, fm: &FontManager) -> (Vec<Page>, Vec<LoadedImage>) {
    let mut state = LayoutState::new(page_styles.clone(), fm);
    blocks::lay_out_node(tree, &mut state, true);
    state.flush_footnotes();
    state.resolve_target_counters();
    state.resolve_margin_boxes();

    while state.pages.len() > 1
        && state.pages.last().map_or(false, |p| p.items.is_empty() && p.footnotes.is_empty())
    {
        state.pages.pop();
    }

    let images = std::mem::take(&mut state.images);
    (state.pages, images)
}
