//! Table and image layout.

use crate::css::values::*;
use crate::style::{ComputedStyle, StyledContent, StyledNode};

use super::{LayoutItem, ItemKind, LayoutState, InlineStyle};
use super::content::collect_text_content;

// ═══════════════════════════════════════════════════════════════
//  Image layout
// ═══════════════════════════════════════════════════════════════

pub(super) fn lay_out_image(node: &StyledNode, state: &mut LayoutState) {
    let Some(src) = node.attrs.iter().find(|(k, _)| k == "src").map(|(_, v)| v.as_str()) else {
        return;
    };

    if src.ends_with(".svg") {
        if let Some(loaded) = crate::svg::render_svg_file(src, 300.0) {
            embed_loaded_image(loaded, node, state);
        }
        return;
    }

    let Ok(img) = image::open(src) else { return };

    let (img_w, img_h) = (img.width(), img.height());
    let rgb = img.to_rgb8();

    let (display_w, display_h) = scale_to_fit(img_w, img_h, 96.0, state.content_width_mm);

    state.current_y += node.style.margin_top_mm;
    state.ensure_space(display_h);

    let image_id = state.images.len();
    state.images.push(super::LoadedImage { pixels: rgb.into_raw(), width: img_w, height: img_h });
    state.push_item(LayoutItem::image_item(
        (0.0, state.current_y),
        ItemKind::Image { id: image_id, width_mm: display_w, height_mm: display_h },
    ));
    state.current_y += display_h + node.style.margin_bottom_mm;
}

fn embed_loaded_image(loaded: super::LoadedImage, node: &StyledNode, state: &mut LayoutState) {
    let (display_w, display_h) = scale_to_fit(loaded.width, loaded.height, 300.0, state.content_width_mm);

    state.current_y += node.style.margin_top_mm;
    state.ensure_space(display_h);

    let image_id = state.images.len();
    state.images.push(loaded);
    state.push_item(LayoutItem::image_item(
        (0.0, state.current_y),
        ItemKind::Image { id: image_id, width_mm: display_w, height_mm: display_h },
    ));
    state.current_y += display_h + node.style.margin_bottom_mm;
}

fn scale_to_fit(pixel_w: u32, pixel_h: u32, dpi: f64, max_width_mm: f64) -> (f64, f64) {
    let natural_w = pixel_w as f64 / dpi * 25.4;
    let natural_h = pixel_h as f64 / dpi * 25.4;
    let scale = if natural_w > max_width_mm { max_width_mm / natural_w } else { 1.0 };
    (natural_w * scale, natural_h * scale)
}

// ═══════════════════════════════════════════════════════════════
//  Table layout
// ═══════════════════════════════════════════════════════════════

struct TableRowContext<'a> {
    col_width: f64,
    cell_padding: f64,
    style: &'a ComputedStyle,
}

pub(super) fn lay_out_table(node: &StyledNode, state: &mut LayoutState) {
    state.current_y += node.style.margin_top_mm;

    let (rows, is_header) = collect_table_rows(node);
    if rows.is_empty() {
        return;
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(1);
    let col_width = state.content_width_mm / num_cols as f64;
    let cell_padding = 1.5;
    let resolved = state.fm.resolve(&node.style.font_family, node.style.font_weight, node.style.font_style);
    let lh = state.fm.metrics(&resolved).line_height_mm(node.style.font_size_pt, node.style.line_height);
    let row_height = lh + cell_padding * 2.0;
    let total_rows = rows.len();

    let row_ctx = TableRowContext { col_width, cell_padding, style: &node.style };

    for (row_idx, row) in rows.iter().enumerate() {
        state.ensure_space(row_height);
        let is_hdr = is_header.get(row_idx).copied().unwrap_or(false);
        emit_table_row(row, is_hdr, &row_ctx, state);
        state.current_y += row_height;
        emit_table_row_separator(is_hdr, row_idx, total_rows, state);
    }
    state.current_y += node.style.margin_bottom_mm;
}

fn collect_table_rows(node: &StyledNode) -> (Vec<Vec<String>>, Vec<bool>) {
    let mut rows = Vec::new();
    let mut is_header = Vec::new();

    for child in &node.children {
        let StyledContent::Element(child_node) = child else { continue };
        match child_node.tag.as_str() {
            "thead" | "tbody" | "tfoot" => {
                collect_rows_from_section(child_node, &mut rows, &mut is_header);
            }
            "tr" => {
                let (cells, is_hdr) = collect_table_row(child_node);
                rows.push(cells);
                is_header.push(is_hdr);
            }
            _ => {}
        }
    }
    (rows, is_header)
}

fn collect_rows_from_section(section: &StyledNode, rows: &mut Vec<Vec<String>>, is_header: &mut Vec<bool>) {
    for row_child in &section.children {
        let StyledContent::Element(tr) = row_child else { continue };
        if tr.tag != "tr" {
            continue;
        }
        let (cells, is_hdr) = collect_table_row(tr);
        rows.push(cells);
        is_header.push(is_hdr);
    }
}

fn emit_table_row(
    row: &[String],
    is_hdr: bool,
    ctx: &TableRowContext,
    state: &mut LayoutState,
) {
    let font_weight = if is_hdr { FontWeight::Bold } else { ctx.style.font_weight };
    let inline_style = InlineStyle {
        font_size_pt: ctx.style.font_size_pt,
        font_weight,
        font_style: ctx.style.font_style,
        font_family: ctx.style.font_family.clone(),
        color: ctx.style.color,
    };
    for (col_idx, cell_text) in row.iter().enumerate() {
        let x = col_idx as f64 * ctx.col_width + ctx.cell_padding;
        state.push_item(LayoutItem::text_item(
            x, state.current_y + ctx.cell_padding, cell_text.clone(), &inline_style,
        ));
    }
}

fn emit_table_row_separator(is_hdr: bool, row_idx: usize, total_rows: usize, state: &mut LayoutState) {
    if !is_hdr && row_idx != total_rows - 1 {
        return;
    }
    let thickness = if is_hdr { 0.3 } else { 0.15 };
    state.push_item(LayoutItem::hr_item(
        (0.0, state.current_y),
        ItemKind::HorizontalRule { width_mm: state.content_width_mm, thickness_mm: thickness, color: Color::rgb(180, 180, 180) },
    ));
    state.current_y += 0.5;
}

fn collect_table_row(tr: &StyledNode) -> (Vec<String>, bool) {
    let mut cells = Vec::new();
    let mut is_header = false;
    for child in &tr.children {
        let StyledContent::Element(td) = child else { continue };
        if td.tag == "th" { is_header = true; }
        cells.push(collect_text_content(td).trim().to_string());
    }
    (cells, is_header)
}

// ═══════════════════════════════════════════════════════════════
//  Math layout
// ═══════════════════════════════════════════════════════════════

pub(super) fn lay_out_math(node: &StyledNode, state: &mut LayoutState) {
    let items = crate::mathml::render_math(node, node.style.font_size_pt, state.fm);
    let lh = node.style.font_size_pt * 1.6 * 25.4 / 72.0;

    state.current_y += node.style.margin_top_mm;
    state.ensure_space(lh);

    for mut item in items {
        item.y_mm += state.current_y;
        state.push_item(item);
    }

    state.current_y += lh + node.style.margin_bottom_mm;
}
