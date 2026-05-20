use printpdf::{
    Line, LinePoint, Mm, Op, PdfDocument, PdfPage, PdfSaveOptions,
    Cmyk, Point, Pt, RawImage, RawImageData, RawImageFormat, Rgb, TextItem, XObjectId, XObjectTransform,
};

use crate::css::values::{Color, FontStyle, FontWeight, MarginBoxPosition, TextAlign};
use crate::css::PageStyle;
use crate::font::FontManager;
use crate::layout::{ItemKind, LoadedImage, Page, ResolvedMarginBox};

/// Render laid-out pages to PDF bytes.
pub fn render(
    style: &PageStyle,
    pages: &[Page],
    images: &[LoadedImage],
    fm: &mut FontManager,
) -> Vec<u8> {
    let w = Mm(style.width_mm as f32);
    let h = Mm(style.height_mm as f32);

    let mut doc = PdfDocument::new("pagina output");

    // Register external fonts
    fm.register_with_document(&mut doc);

    // Register images as XObjects
    let image_ids: Vec<XObjectId> = images
        .iter()
        .map(|img| {
            let raw = RawImage {
                pixels: RawImageData::U8(img.pixels.clone()),
                width: img.width as usize,
                height: img.height as usize,
                data_format: RawImageFormat::RGB8,
                tag: Vec::new(),
            };
            doc.add_image(&raw)
        })
        .collect();

    let pdf_pages: Vec<PdfPage> = pages
        .iter()
        .map(|page| {
            let ops = build_page_ops(style, page, fm, &image_ids);
            PdfPage::new(w, h, ops)
        })
        .collect();

    doc.with_pages(pdf_pages);

    // Add bookmarks from all pages
    for (page_idx, page) in pages.iter().enumerate() {
        for bm in &page.bookmarks {
            doc.add_bookmark(&bm.title, page_idx);
        }
    }

    let mut warnings = Vec::new();
    doc.save(&PdfSaveOptions::default(), &mut warnings)
}

fn color_to_printpdf(c: &Color) -> printpdf::Color {
    if let Some(cmyk) = &c.cmyk {
        printpdf::Color::Cmyk(Cmyk {
            c: cmyk.c,
            m: cmyk.m,
            y: cmyk.y,
            k: cmyk.k,
            icc_profile: None,
        })
    } else {
        printpdf::Color::Rgb(Rgb {
            r: c.r as f32 / 255.0,
            g: c.g as f32 / 255.0,
            b: c.b as f32 / 255.0,
            icc_profile: None,
        })
    }
}

fn build_page_ops(
    style: &PageStyle,
    page: &Page,
    fm: &FontManager,
    image_ids: &[XObjectId],
) -> Vec<Op> {
    let mut ops = Vec::new();
    render_items(&mut ops, style, &page.items, fm, image_ids);
    render_items(&mut ops, style, &page.footnotes, fm, image_ids);
    render_margin_boxes(&mut ops, style, &page.margin_boxes, fm);

    // Link annotations
    for link in &page.links {
        let x = (style.margin_left_mm + link.x_mm) as f32;
        let y_bottom = (style.height_mm - style.margin_top_mm - link.y_mm - link.height_mm) as f32;
        let rect = printpdf::Rect::from_xywh(
            Pt(x * 72.0 / 25.4),
            Pt(y_bottom * 72.0 / 25.4),
            Pt(link.width_mm as f32 * 72.0 / 25.4),
            Pt(link.height_mm as f32 * 72.0 / 25.4),
        );
        let actions = match &link.target {
            crate::layout::LinkTarget::Uri(url) => printpdf::Actions::Uri(url.clone()),
            crate::layout::LinkTarget::Internal(_id) => {
                // For internal links, we'd need to resolve page number.
                // Use page 0 as fallback; lopdf post-processing can fix this.
                printpdf::Actions::Goto(printpdf::Destination::Xyz {
                    page: 0,
                    left: None,
                    top: None,
                    zoom: None,
                })
            }
        };
        ops.push(Op::LinkAnnotation {
            link: printpdf::LinkAnnotation::new(rect, actions, None, None, None),
        });
    }

    ops
}

fn render_items(
    ops: &mut Vec<Op>,
    style: &PageStyle,
    items: &[crate::layout::LayoutItem],
    fm: &FontManager,
    image_ids: &[XObjectId],
) {
    for item in items {
        match &item.kind {
            ItemKind::Text => render_text_item(ops, style, item, fm),
            ItemKind::HorizontalRule { width_mm, thickness_mm, color } => {
                render_hr(ops, style, item, *width_mm, *thickness_mm, color);
            }
            ItemKind::Image { id, width_mm, height_mm } => {
                if let Some(xobj_id) = image_ids.get(*id) {
                    render_image(ops, style, item, xobj_id, *width_mm, *height_mm);
                }
            }
        }
    }
}

fn render_text_item(
    ops: &mut Vec<Op>,
    style: &PageStyle,
    item: &crate::layout::LayoutItem,
    fm: &FontManager,
) {
    let x = (style.margin_left_mm + item.x_mm) as f32;
    let y = (style.height_mm - style.margin_top_mm - item.y_mm
        - item.font_size_pt * 25.4 / 72.0) as f32;

    let resolved = fm.resolve(&item.font_family, item.font_weight, item.font_style);
    let handle = fm.pdf_handle(&resolved);

    ops.push(Op::StartTextSection);
    ops.push(Op::SetFillColor { col: color_to_printpdf(&item.color) });
    ops.push(Op::SetFont { font: handle, size: Pt(item.font_size_pt as f32) });
    ops.push(Op::SetTextCursor { pos: Point::new(Mm(x), Mm(y)) });
    ops.push(Op::ShowText { items: vec![TextItem::Text(item.text.clone())] });
    ops.push(Op::EndTextSection);
}

fn render_hr(
    ops: &mut Vec<Op>,
    style: &PageStyle,
    item: &crate::layout::LayoutItem,
    width_mm: f64,
    thickness_mm: f64,
    color: &Color,
) {
    let x_start = style.margin_left_mm + item.x_mm;
    let y = style.height_mm - style.margin_top_mm - item.y_mm;

    ops.push(Op::SaveGraphicsState);
    ops.push(Op::SetOutlineColor { col: color_to_printpdf(color) });
    ops.push(Op::SetOutlineThickness {
        pt: Pt(thickness_mm as f32 * 72.0 / 25.4),
    });
    ops.push(Op::DrawLine {
        line: Line {
            points: vec![
                LinePoint { p: Point::new(Mm(x_start as f32), Mm(y as f32)), bezier: false },
                LinePoint { p: Point::new(Mm((x_start + width_mm) as f32), Mm(y as f32)), bezier: false },
            ],
            is_closed: false,
        },
    });
    ops.push(Op::RestoreGraphicsState);
}

fn render_image(
    ops: &mut Vec<Op>,
    style: &PageStyle,
    item: &crate::layout::LayoutItem,
    xobj_id: &XObjectId,
    width_mm: f64,
    height_mm: f64,
) {
    let x = style.margin_left_mm + item.x_mm;
    // Y: bottom-up, image positioned from its bottom-left corner
    let y = style.height_mm - style.margin_top_mm - item.y_mm - height_mm;

    ops.push(Op::UseXobject {
        id: xobj_id.clone(),
        transform: XObjectTransform {
            translate_x: Some(Pt(x as f32 * 72.0 / 25.4)),
            translate_y: Some(Pt(y as f32 * 72.0 / 25.4)),
            scale_x: Some(width_mm as f32 * 72.0 / 25.4),
            scale_y: Some(height_mm as f32 * 72.0 / 25.4),
            dpi: Some(72.0),
            rotate: None,
        },
    });
}

fn render_margin_boxes(
    ops: &mut Vec<Op>,
    style: &PageStyle,
    boxes: &[ResolvedMarginBox],
    fm: &FontManager,
) {
    for mb in boxes {
        let resolved = fm.resolve("Helvetica", FontWeight::Normal, FontStyle::Normal);
        let metrics = fm.metrics(&resolved);
        let text_width_mm = metrics.text_width_mm(&mb.text, mb.font_size_pt);
        let area_width = margin_box_area_width(style, &mb.position);
        let area_x = margin_box_area_x(style, &mb.position);

        let x = match mb.text_align {
            TextAlign::Center => area_x + (area_width - text_width_mm).max(0.0) / 2.0,
            TextAlign::Right => area_x + (area_width - text_width_mm).max(0.0),
            _ => area_x,
        };

        let font_height_mm = mb.font_size_pt * 25.4 / 72.0;
        let y = if mb.position.is_top() {
            style.height_mm - style.margin_top_mm / 2.0 - font_height_mm / 2.0
        } else if mb.position.is_bottom() {
            style.margin_bottom_mm / 2.0 - font_height_mm / 2.0
        } else {
            style.height_mm / 2.0
        };

        let handle = fm.pdf_handle(&resolved);

        ops.push(Op::StartTextSection);
        ops.push(Op::SetFillColor { col: color_to_printpdf(&mb.color) });
        ops.push(Op::SetFont { font: handle, size: Pt(mb.font_size_pt as f32) });
        ops.push(Op::SetTextCursor { pos: Point::new(Mm(x as f32), Mm(y as f32)) });
        ops.push(Op::ShowText { items: vec![TextItem::Text(mb.text.clone())] });
        ops.push(Op::EndTextSection);
    }
}

fn margin_box_area_x(style: &PageStyle, pos: &MarginBoxPosition) -> f64 {
    match pos {
        MarginBoxPosition::LeftTop | MarginBoxPosition::LeftMiddle | MarginBoxPosition::LeftBottom => 0.0,
        MarginBoxPosition::RightTop | MarginBoxPosition::RightMiddle | MarginBoxPosition::RightBottom => {
            style.width_mm - style.margin_right_mm
        }
        _ => style.margin_left_mm,
    }
}

fn margin_box_area_width(style: &PageStyle, pos: &MarginBoxPosition) -> f64 {
    match pos {
        MarginBoxPosition::TopLeft | MarginBoxPosition::BottomLeft => style.content_width_mm() / 3.0,
        MarginBoxPosition::TopCenter | MarginBoxPosition::BottomCenter => style.content_width_mm(),
        MarginBoxPosition::TopRight | MarginBoxPosition::BottomRight => style.content_width_mm() / 3.0,
        MarginBoxPosition::LeftTop | MarginBoxPosition::LeftMiddle | MarginBoxPosition::LeftBottom => style.margin_left_mm,
        MarginBoxPosition::RightTop | MarginBoxPosition::RightMiddle | MarginBoxPosition::RightBottom => style.margin_right_mm,
        _ => style.content_width_mm(),
    }
}
