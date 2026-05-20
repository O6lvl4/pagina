//! Layout state: page management, footnotes, counters.

use std::collections::HashMap;

use crate::css::values::*;
use crate::css::PageStyleSet;
use crate::font::FontManager;

use super::{Page, LayoutItem, ItemKind, LoadedImage, InlineStyle};
use super::content;

pub(super) struct FootnoteData {
    pub(super) number: usize,
    pub(super) text: String,
    pub(super) style: InlineStyle,
}

pub(super) struct LayoutState<'a> {
    pub(super) page_styles: PageStyleSet,
    pub(super) fm: &'a FontManager,
    pub(super) content_width_mm: f64,
    pub(super) content_height_mm: f64,

    pub(super) pages: Vec<Page>,
    pub(super) current_y: f64,

    pub(super) running_strings: HashMap<String, String>,
    pub(super) footnotes_pending: Vec<FootnoteData>,
    pub(super) footnote_counter: usize,
    pub(super) footnote_area_height: f64,

    pub(super) images: Vec<LoadedImage>,

    /// Map from element ID to the page number (1-indexed) where it was laid out.
    pub(super) id_to_page: HashMap<String, usize>,
}

impl<'a> LayoutState<'a> {
    pub(super) fn new(page_styles: PageStyleSet, fm: &'a FontManager) -> Self {
        let cw = page_styles.base.content_width_mm();
        let ch = page_styles.base.content_height_mm();
        Self {
            page_styles,
            fm,
            content_width_mm: cw,
            content_height_mm: ch,
            pages: vec![Page::new()],
            current_y: 0.0,
            running_strings: HashMap::new(),
            footnotes_pending: Vec::new(),
            footnote_counter: 0,
            footnote_area_height: 0.0,
            images: Vec::new(),
            id_to_page: HashMap::new(),
        }
    }

    pub(super) fn new_page(&mut self) {
        self.flush_footnotes();
        self.pages.push(Page::new());
        self.current_y = 0.0;
        self.footnote_area_height = 0.0;
    }

    pub(super) fn available_height(&self) -> f64 {
        self.content_height_mm - self.current_y - self.footnote_area_height
    }

    pub(super) fn current_page_mut(&mut self) -> &mut Page {
        self.pages.last_mut().expect("pages should never be empty")
    }

    pub(super) fn current_page_has_items(&self) -> bool {
        self.pages.last().map_or(false, |p| !p.items.is_empty())
    }

    pub(super) fn push_item(&mut self, item: LayoutItem) {
        self.current_page_mut().items.push(item);
    }

    pub(super) fn ensure_space(&mut self, height_needed: f64) {
        if height_needed > self.available_height() && self.current_page_has_items() {
            self.new_page();
        }
    }

    pub(super) fn add_footnote(&mut self, text: String, style: &InlineStyle) {
        self.footnote_counter += 1;
        let num = self.footnote_counter;
        let fn_style = InlineStyle {
            font_size_pt: 8.0,
            ..style.clone()
        };
        self.footnotes_pending.push(FootnoteData { number: num, text, style: fn_style });
        let lh = 8.0 * 1.3 * 25.4 / 72.0;
        self.footnote_area_height += lh + 1.0;
    }

    pub(super) fn flush_footnotes(&mut self) {
        if self.footnotes_pending.is_empty() {
            return;
        }
        let page = self.pages.last_mut().expect("pages should never be empty");
        let footnotes = std::mem::take(&mut self.footnotes_pending);
        let mut fn_y = self.content_height_mm - self.footnote_area_height;

        page.footnotes.push(LayoutItem::hr_item(
            (0.0, fn_y),
            ItemKind::HorizontalRule { width_mm: self.content_width_mm * 0.3, thickness_mm: 0.15, color: Color::rgb(128, 128, 128) },
        ));
        fn_y += 2.0;

        for fnd in &footnotes {
            let lh = fnd.style.font_size_pt * 1.3 * 25.4 / 72.0;
            let text = format!("{}. {}", fnd.number, fnd.text);
            page.footnotes.push(LayoutItem::text_item(0.0, fn_y, text, &fnd.style));
            fn_y += lh;
        }
        self.footnote_area_height = 0.0;
    }

    pub(super) fn resolve_target_counters(&mut self) {
        let id_map = &self.id_to_page;
        for page in &mut self.pages {
            for item in page.items.iter_mut().chain(page.footnotes.iter_mut()) {
                content::resolve_target_placeholders(&mut item.text, id_map);
            }
        }
    }

    pub(super) fn resolve_margin_boxes(&mut self) {
        let total_pages = self.pages.len();
        for page_num in 0..total_pages {
            let page_style = self.page_styles.for_page(page_num + 1, total_pages);
            let ctx = content::ContentResolveContext {
                page_num: page_num + 1,
                total_pages,
                running_strings: &self.running_strings,
            };
            self.pages[page_num].margin_boxes = content::build_margin_boxes(&page_style, &ctx);
        }
    }
}
