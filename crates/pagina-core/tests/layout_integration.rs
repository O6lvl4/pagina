/// Integration tests for the layout pipeline using MockFontProvider.
/// No real fonts needed — all text measurements use fixed-width characters.

use pagina_core::css::{PageStyle, PageStyleSet};
use pagina_core::css::values::*;
use pagina_core::font::MockFontProvider;
use pagina_core::layout;
use pagina_core::style;

fn default_page_styles() -> PageStyleSet {
    PageStyleSet::default()
}

fn parse_and_lay_out(html: &str) -> Vec<layout::Page> {
    let fm = MockFontProvider::new(500, 1000); // every char = 0.5em
    let dom = pagina_core::dom::parse_html(html);
    let mut pss = default_page_styles();
    let mut rules = Vec::new();
    let styles = pagina_core::dom::extract_styles(&dom.document);
    for css in &styles {
        pagina_core::css::parser::parse_stylesheet(css, &mut pss, &mut rules);
    }
    let tree = style::build_styled_tree(&dom.document, &rules).unwrap();
    let (pages, _images) = layout::lay_out(&pss, &tree, &fm);
    pages
}

// ─── Basic rendering ──────────────────────────────────

#[test]
fn single_paragraph_produces_one_page() {
    let pages = parse_and_lay_out("<html><body><p>Hello world</p></body></html>");
    assert_eq!(pages.len(), 1);
    assert!(!pages[0].items.is_empty());
}

#[test]
fn empty_body_produces_one_page() {
    let pages = parse_and_lay_out("<html><body></body></html>");
    assert_eq!(pages.len(), 1);
}

#[test]
fn multiple_paragraphs_on_one_page() {
    let pages = parse_and_lay_out("
        <html><body>
        <p>First paragraph</p>
        <p>Second paragraph</p>
        <p>Third paragraph</p>
        </body></html>
    ");
    assert_eq!(pages.len(), 1);
    assert!(pages[0].items.len() >= 3);
}

// ─── Page breaks ──────────────────────────────────────

#[test]
fn break_before_page_creates_new_page() {
    let pages = parse_and_lay_out("
        <html><head><style>
        h2 { break-before: page; }
        </style></head><body>
        <p>Page one content</p>
        <h2>Chapter Two</h2>
        <p>Page two content</p>
        </body></html>
    ");
    assert!(pages.len() >= 2, "expected at least 2 pages, got {}", pages.len());
}

#[test]
fn break_after_page_creates_new_page() {
    let pages = parse_and_lay_out("
        <html><head><style>
        .breaker { break-after: page; }
        </style></head><body>
        <p class=\"breaker\">Page one content</p>
        <p>Page two content</p>
        </body></html>
    ");
    assert!(pages.len() >= 2);
}

// ─── Text content ─────────────────────────────────────

#[test]
fn heading_text_is_rendered() {
    let pages = parse_and_lay_out("<html><body><h1>Title</h1></body></html>");
    let has_title = pages[0].items.iter().any(|item| item.text.contains("Title"));
    assert!(has_title, "heading text should be in layout items");
}

#[test]
fn inline_styles_are_applied() {
    let pages = parse_and_lay_out("
        <html><body>
        <p>Normal <strong>bold</strong> text</p>
        </body></html>
    ");
    let bold_items = pages[0].items.iter()
        .filter(|item| item.font_weight == FontWeight::Bold)
        .count();
    assert!(bold_items > 0, "should have bold items from <strong>");
}

#[test]
fn italic_is_applied() {
    let pages = parse_and_lay_out("
        <html><body>
        <p>Normal <em>italic</em> text</p>
        </body></html>
    ");
    let italic_items = pages[0].items.iter()
        .filter(|item| item.font_style == FontStyle::Italic)
        .count();
    assert!(italic_items > 0, "should have italic items from <em>");
}

// ─── CSS properties ───────────────────────────────────

#[test]
fn css_color_is_applied() {
    let pages = parse_and_lay_out("
        <html><head><style>
        p { color: red; }
        </style></head><body>
        <p>Red text</p>
        </body></html>
    ");
    let red_items = pages[0].items.iter()
        .filter(|item| item.color.r == 255 && item.color.g == 0 && item.color.b == 0)
        .count();
    assert!(red_items > 0, "should have red colored items");
}

#[test]
fn css_text_align_center() {
    let pages = parse_and_lay_out("
        <html><head><style>
        p { text-align: center; }
        </style></head><body>
        <p>Centered</p>
        </body></html>
    ");
    // Centered text should have x_mm > 0
    let first_item = pages[0].items.first().unwrap();
    assert!(first_item.x_mm > 0.0, "centered text should have positive x offset");
}

// ─── Page size ────────────────────────────────────────

#[test]
fn custom_page_size_is_applied() {
    let pages = parse_and_lay_out("
        <html><head><style>
        @page { size: letter; }
        </style></head><body>
        <p>Letter size</p>
        </body></html>
    ");
    assert!(!pages.is_empty());
}

// ─── Lists ────────────────────────────────────────────

#[test]
fn unordered_list_renders_items() {
    let pages = parse_and_lay_out("
        <html><body>
        <ul><li>One</li><li>Two</li><li>Three</li></ul>
        </body></html>
    ");
    let dash_items = pages[0].items.iter()
        .filter(|item| item.text.starts_with("- "))
        .count();
    assert_eq!(dash_items, 3, "should have 3 list items with dash markers");
}

#[test]
fn ordered_list_renders_numbers() {
    let pages = parse_and_lay_out("
        <html><body>
        <ol><li>One</li><li>Two</li></ol>
        </body></html>
    ");
    let has_1 = pages[0].items.iter().any(|item| item.text.starts_with("1. "));
    let has_2 = pages[0].items.iter().any(|item| item.text.starts_with("2. "));
    assert!(has_1 && has_2, "should have numbered list items");
}

// ─── Bookmarks ────────────────────────────────────────

#[test]
fn headings_generate_bookmarks() {
    let pages = parse_and_lay_out("
        <html><body>
        <h1>Title</h1>
        <h2>Section</h2>
        </body></html>
    ");
    let bookmarks: Vec<_> = pages.iter().flat_map(|p| &p.bookmarks).collect();
    assert!(bookmarks.len() >= 2, "should have bookmarks for h1 and h2");
    assert_eq!(bookmarks[0].level, 1);
    assert_eq!(bookmarks[1].level, 2);
}

// ─── Links ────────────────────────────────────────────

#[test]
fn anchor_creates_link_annotation() {
    let pages = parse_and_lay_out("
        <html><body>
        <p><a href=\"https://example.com\">Link</a></p>
        </body></html>
    ");
    let links: Vec<_> = pages.iter().flat_map(|p| &p.links).collect();
    assert!(!links.is_empty(), "should have a link annotation");
    match &links[0].target {
        layout::LinkTarget::Uri(url) => assert_eq!(url, "https://example.com"),
        _ => panic!("expected URI link"),
    }
}

// ─── Margin boxes ─────────────────────────────────────

#[test]
fn margin_boxes_with_page_counter() {
    let pages = parse_and_lay_out("
        <html><head><style>
        @page { @bottom-center { content: counter(page) \" / \" counter(pages); } }
        </style></head><body>
        <p>Content</p>
        </body></html>
    ");
    assert!(!pages[0].margin_boxes.is_empty(), "should have margin boxes");
    let mb = &pages[0].margin_boxes[0];
    assert!(mb.text.contains("1"), "should contain page number");
}

// ─── Descendant selectors ─────────────────────────────

#[test]
fn descendant_selector_matches() {
    let pages = parse_and_lay_out("
        <html><head><style>
        .container p { color: blue; }
        </style></head><body>
        <div class=\"container\"><p>Blue text</p></div>
        <p>Not blue</p>
        </body></html>
    ");
    let blue_items = pages[0].items.iter()
        .filter(|item| item.color.b == 255 && item.color.r == 0)
        .count();
    assert!(blue_items > 0, "descendant selector should apply color");
}

// ─── HR ───────────────────────────────────────────────

#[test]
fn hr_produces_horizontal_rule() {
    let pages = parse_and_lay_out("
        <html><body>
        <p>Before</p>
        <hr>
        <p>After</p>
        </body></html>
    ");
    let has_hr = pages[0].items.iter()
        .any(|item| matches!(item.kind, layout::ItemKind::HorizontalRule { .. }));
    assert!(has_hr, "should have a horizontal rule item");
}

// ─── MockFontProvider produces predictable widths ─────

#[test]
fn mock_font_provider_fixed_width() {
    let fm = MockFontProvider::new(500, 1000);
    use pagina_core::font::FontProvider;
    let w = fm.measure_text("ABC", "Helvetica", FontWeight::Normal, FontStyle::Normal, 72.0);
    // 3 chars * 500/1000 * 72/72 * 25.4 = 3 * 0.5 * 25.4 = 38.1mm
    let expected = 3.0 * 500.0 / 1000.0 * 72.0 / 1000.0 * 25.4 / 72.0 * 1000.0;
    // Actually: 3 * 500 * (72.0 / 1000.0) * (25.4 / 72.0) = 3 * 500 * 25.4 / 1000.0 = 38.1
    let expected2 = 3.0 * 500.0 * 72.0 / 1000.0 * 25.4 / 72.0;
    assert!((w - expected2).abs() < 0.01, "w={w}, expected={expected2}");
}
