use std::sync::Arc;

use egui::{Color32, FontId, Pos2, Rect, pos2};
use gridcraft_engine::cf::CfCache;
use gridcraft_engine::core::{CellRef, RangeRef, Value};
use gridcraft_engine::model::{BorderLine, BorderStyle, HAlign, Sheet, Underline, Workbook};

use super::SheetApp;
use crate::grid::{
    DISPLAY_SCALE, Look, cell_text_y, col32, display_text, galley_baseline_bounds, look, painted_halign, wrapped_cell_text_y,
};
use crate::theme;

const MAX_CLIP_ROWS: usize = 100;
const MAX_CLIP_COLS: usize = 30;
const MAX_IMAGE_SIDE: u32 = 4096;
const TARGET_SCALE: f32 = 2.0;
const MARGIN_PT: f32 = 1.0;

struct RangeLayout {
    rows: Vec<u32>,
    cols: Vec<u32>,
    row_y: Vec<f32>,
    col_x: Vec<f32>,
    ppp: f32,
    z: f32,
}

impl RangeLayout {
    fn build(sh: &Sheet, range: RangeRef, ppp: f32) -> Option<Self> {
        let rows: Vec<u32> = (range.start.row..=range.end.row)
            .filter(|&r| !sh.is_row_hidden(r) && sh.row_height(r) > 0.0)
            .take(MAX_CLIP_ROWS + 1)
            .collect();
        if rows.is_empty() || rows.len() > MAX_CLIP_ROWS {
            return None;
        }
        let cols: Vec<u32> = (range.start.col..=range.end.col)
            .filter(|&c| !sh.is_col_hidden(c) && sh.col_width(c) > 0.0)
            .take(MAX_CLIP_COLS + 1)
            .collect();
        if cols.is_empty() || cols.len() > MAX_CLIP_COLS {
            return None;
        }
        let z = (TARGET_SCALE / ppp).max(1.0);
        let mut row_y = Vec::with_capacity(rows.len() + 1);
        let mut y = MARGIN_PT * z;
        row_y.push(y);
        for &r in &rows {
            y += (sh.row_height(r) * DISPLAY_SCALE * z).round().max(1.0);
            row_y.push(y);
        }
        let mut col_x = Vec::with_capacity(cols.len() + 1);
        let mut x = MARGIN_PT * z;
        col_x.push(x);
        for &c in &cols {
            x += (sh.col_width(c) * DISPLAY_SCALE * z).round().max(1.0);
            col_x.push(x);
        }
        Some(Self { rows, cols, row_y, col_x, ppp, z })
    }

    fn total_width_px(&self) -> u32 {
        let right = self.col_x.last().copied().unwrap_or(0.0) + MARGIN_PT * self.z;
        (right * self.ppp).ceil().max(1.0) as u32
    }

    fn total_height_px(&self) -> u32 {
        let bottom = self.row_y.last().copied().unwrap_or(0.0) + MARGIN_PT * self.z;
        (bottom * self.ppp).ceil().max(1.0) as u32
    }

    fn cell_rect(&self, r_idx: usize, c_idx: usize) -> Rect {
        Rect::from_min_max(
            pos2(self.col_x[c_idx], self.row_y[r_idx]),
            pos2(self.col_x[c_idx + 1], self.row_y[r_idx + 1]),
        )
    }

    fn span_rect(&self, m: RangeRef) -> Option<Rect> {
        let r0 = self.rows.partition_point(|&r| r < m.start.row);
        let r1 = self.rows.partition_point(|&r| r <= m.end.row);
        let c0 = self.cols.partition_point(|&c| c < m.start.col);
        let c1 = self.cols.partition_point(|&c| c <= m.end.col);
        if r0 >= r1 || c0 >= c1 {
            return None;
        }
        Some(Rect::from_min_max(pos2(self.col_x[c0], self.row_y[r0]), pos2(self.col_x[c1], self.row_y[r1])))
    }
}

struct PaintedCell {
    cell: CellRef,
    rect: Rect,
    look: Look,
}

struct PlacedText {
    pos: Pos2,
    clip: Rect,
    galley: Arc<egui::Galley>,
    color: Color32,
    underline: Underline,
    strike: bool,
}

impl SheetApp {
    /// Render the currently copied range to PNG bytes when it spans more than one cell.
    pub(super) fn render_clipboard_png(&self, ctx: &egui::Context) -> Option<Vec<u8>> {
        let clip = self.session.clipboard.as_ref()?;
        render_range_to_png(ctx, &clip.wb, clip.sheet, clip.range)
    }
}

/// Rasterize a multi-cell range into a crisp 2x PNG image for system clipboard sharing.
/// Returns `None` for single-cell selections (`range.start == range.end`) or oversized ranges.
pub(crate) fn render_range_to_png(
    ctx: &egui::Context,
    wb: &Workbook,
    sheet_idx: usize,
    range: RangeRef,
) -> Option<Vec<u8>> {
    if range.start == range.end {
        return None;
    }
    let sh = wb.sheet(sheet_idx)?;
    let ppp = ctx.pixels_per_point().clamp(1.0, 4.0);
    let layout = RangeLayout::build(sh, range, ppp)?;
    let (w, h) = (layout.total_width_px(), layout.total_height_px());
    if w < 4 || h < 4 || w > MAX_IMAGE_SIDE || h > MAX_IMAGE_SIDE {
        return None;
    }
    let mut img = image::RgbaImage::from_pixel(w, h, image::Rgba([255, 255, 255, 255]));
    if sh.show_gridlines {
        draw_gridlines(&mut img, &layout);
    }
    let merges: Vec<RangeRef> = sh.merges.iter().filter_map(|m| m.intersection(&range)).collect();
    let cells = collect_cells_and_paint_fills(&mut img, wb, sheet_idx, sh, &layout, &merges);
    paint_all_cell_texts(&mut img, ctx, wb, sh, &layout, &cells);
    paint_all_borders(&mut img, wb, sheet_idx, sh, &layout);
    super::clipboard::encode_png(&img).ok()
}

fn draw_gridlines(img: &mut image::RgbaImage, layout: &RangeLayout) {
    let grid_col = Color32::from_rgb(0xD4, 0xD4, 0xD4);
    let top = (layout.row_y.first().copied().unwrap_or(0.0) * layout.ppp).round() as i32;
    let bottom = (layout.row_y.last().copied().unwrap_or(0.0) * layout.ppp).round() as i32;
    let left = (layout.col_x.first().copied().unwrap_or(0.0) * layout.ppp).round() as i32;
    let right = (layout.col_x.last().copied().unwrap_or(0.0) * layout.ppp).round() as i32;
    let thickness = (layout.z * layout.ppp * 0.5).round().max(1.0) as i32;
    for &x_pt in &layout.col_x {
        let x = (x_pt * layout.ppp).round() as i32;
        fill_rect_px(img, x, top, x + thickness, bottom, grid_col);
    }
    for &y_pt in &layout.row_y {
        let y = (y_pt * layout.ppp).round() as i32;
        fill_rect_px(img, left, y, right, y + thickness, grid_col);
    }
}

fn collect_cells_and_paint_fills(
    img: &mut image::RgbaImage,
    wb: &Workbook,
    sheet_idx: usize,
    sh: &Sheet,
    layout: &RangeLayout,
    merges: &[RangeRef],
) -> Vec<PaintedCell> {
    let mut cf = CfCache::default();
    let mut out = Vec::with_capacity(layout.rows.len() * layout.cols.len());
    for (r_idx, &row) in layout.rows.iter().enumerate() {
        for (c_idx, &col) in layout.cols.iter().enumerate() {
            let c = CellRef::new(row, col);
            let merged = merges.iter().find(|m| m.contains(c)).copied();
            let (rect, anchor) = match merged {
                Some(m) => {
                    let first_row = layout.rows.iter().copied().find(|&r| r >= m.start.row && r <= m.end.row);
                    let first_col = layout.cols.iter().copied().find(|&col_i| col_i >= m.start.col && col_i <= m.end.col);
                    if first_row != Some(row) || first_col != Some(col) {
                        continue;
                    }
                    let Some(r_rect) = layout.span_rect(m) else { continue };
                    (r_rect, c)
                }
                None => (layout.cell_rect(r_idx, c_idx), c),
            };
            let (lk, _) = look(wb, sheet_idx, sh, anchor, &mut cf);
            if let Some(fill) = lk.fill {
                fill_rect_pt(img, rect, layout.ppp, fill);
            } else if merged.is_some() {
                fill_rect_pt(img, rect.shrink(0.5 * layout.z), layout.ppp, Color32::WHITE);
            }
            out.push(PaintedCell { cell: anchor, rect, look: lk });
        }
    }
    out
}

fn paint_all_cell_texts(
    img: &mut image::RgbaImage,
    ctx: &egui::Context,
    wb: &Workbook,
    sh: &Sheet,
    layout: &RangeLayout,
    cells: &[PaintedCell],
) {
    let resolved_families: Vec<egui::FontFamily> = cells
        .iter()
        .map(|pc| {
            let f = &pc.look.style.font;
            theme::cell_family_in(ctx, &f.name, f.bold, f.italic)
        })
        .collect();

    let (placed, atlas) = ctx.fonts_mut(|fonts| {
        let mut list = Vec::new();
        for (pc, fam) in cells.iter().zip(resolved_families) {
            if let Some(item) = layout_cell_text(fonts, wb, sh, layout, pc, fam) {
                list.push(item);
            }
        }
        (list, fonts.image())
    });

    for item in &placed {
        blit_galley(img, &atlas, item, layout.ppp, layout.z);
    }
}

fn layout_cell_text(
    fonts: &mut egui::epaint::text::FontsView<'_>,
    wb: &Workbook,
    sh: &Sheet,
    layout: &RangeLayout,
    pc: &PaintedCell,
    fam: egui::FontFamily,
) -> Option<PlacedText> {
    let v = sh.value(pc.cell);
    if v.is_empty() {
        return None;
    }
    let st = &pc.look.style;
    let z = layout.z;
    let size = st.font.size * 96.0 / 72.0 * z;
    let font = FontId::new(size, fam);
    let char_w = fonts.layout_no_wrap("0".to_string(), font.clone(), Color32::PLACEHOLDER).size().x.max(1.0);
    let avail_w = pc.rect.width().max(1.0);
    let (text, ncolor, numeric, _) = display_text(wb, sh, pc.cell, &v, st, avail_w, char_w);
    if text.is_empty() {
        return None;
    }
    let color = ncolor.unwrap_or(pc.look.text_color);
    let direction = crate::text_direction::cell_direction(st.align.reading_order, &text);
    let halign = resolve_halign(st.align.h, &v, numeric, direction, sh.right_to_left);
    let indent = f32::from(st.align.indent) * 9.0 * z;
    let pad = 2.5 * z;
    let wrap = st.align.wrap || st.align.h == HAlign::Justify;
    let wrap_w = if wrap { (avail_w - 2.0 * pad - indent).max(4.0) } else { f32::INFINITY };
    let clean: String = if wrap { text } else { text.chars().filter(|&ch| ch != '\n' && ch != '\r').collect() };
    let mut job = egui::text::LayoutJob::simple(clean, font, color, wrap_w);
    job.direction = direction;
    job.halign_min_is_start = false;
    if wrap {
        job.halign = match halign {
            HAlign::Center | HAlign::CenterAcross => egui::Align::Center,
            HAlign::Right => egui::Align::RIGHT,
            _ => egui::Align::LEFT,
        };
    }
    let galley = fonts.layout_job(job);
    let x = match halign {
        HAlign::Center | HAlign::CenterAcross => pc.rect.center().x - galley.rect.center().x,
        HAlign::Right => pc.rect.right() - pad - indent - galley.rect.right(),
        _ => pc.rect.left() + pad + indent - galley.rect.left(),
    };
    let (text_top, text_bottom) = galley_baseline_bounds(&galley);
    let gh = text_bottom - text_top;
    let y = if wrap {
        wrapped_cell_text_y(st.align.v, pc.rect, gh, z) - text_top
    } else {
        cell_text_y(st.align.v, pc.rect, gh, z) - text_top
    };
    Some(PlacedText {
        pos: pos2(x, y),
        clip: pc.rect,
        galley,
        color,
        underline: st.font.underline,
        strike: st.font.strike,
    })
}

fn resolve_halign(
    h: HAlign,
    v: &Value,
    numeric: bool,
    direction: egui::epaint::text::TextDirection,
    sheet_rtl: bool,
) -> HAlign {
    match h {
        HAlign::General => match v {
            Value::Number(_) => HAlign::Right,
            Value::Bool(_) | Value::Error(_) => HAlign::Center,
            _ if numeric => HAlign::Right,
            _ if direction == egui::epaint::text::TextDirection::RightToLeft && !sheet_rtl => HAlign::Right,
            _ => HAlign::Left,
        },
        other => painted_halign(other, sheet_rtl),
    }
}

fn blit_galley(
    img: &mut image::RgbaImage,
    atlas: &egui::ColorImage,
    item: &PlacedText,
    ppp: f32,
    z: f32,
) {
    let clip_min_x = (item.clip.min.x * ppp).floor() as i32;
    let clip_min_y = (item.clip.min.y * ppp).floor() as i32;
    let clip_max_x = (item.clip.max.x * ppp).ceil() as i32;
    let clip_max_y = (item.clip.max.y * ppp).ceil() as i32;
    let [atlas_w, atlas_h] = atlas.size;

    for row in &item.galley.rows {
        for glyph in &row.glyphs {
            let uv = glyph.uv_rect;
            if uv.is_nothing() {
                continue;
            }
            let top_left = item.pos + row.pos.to_vec2() + glyph.pos.to_vec2() + uv.offset;
            let dst_x0 = (top_left.x * ppp).round() as i32;
            let dst_y0 = (top_left.y * ppp).round() as i32;
            let u0 = usize::from(uv.min[0]);
            let v0 = usize::from(uv.min[1]);
            let gw = usize::from(uv.max[0].saturating_sub(uv.min[0]));
            let gh = usize::from(uv.max[1].saturating_sub(uv.min[1]));
            for dy in 0..gh {
                let py = dst_y0 + dy as i32;
                if py < clip_min_y || py >= clip_max_y || v0 + dy >= atlas_h {
                    continue;
                }
                for dx in 0..gw {
                    let px = dst_x0 + dx as i32;
                    if px < clip_min_x || px >= clip_max_x || u0 + dx >= atlas_w {
                        continue;
                    }
                    let texel = atlas.pixels[(v0 + dy) * atlas_w + (u0 + dx)];
                    blend_glyph_pixel(img, px, py, texel, item.color);
                }
            }
        }
        draw_row_decorations(img, item, row, ppp, z);
    }
}

fn draw_row_decorations(
    img: &mut image::RgbaImage,
    item: &PlacedText,
    row: &egui::epaint::text::PlacedRow,
    ppp: f32,
    z: f32,
) {
    let r = row.rect().translate(item.pos.to_vec2()).intersect(item.clip);
    if !r.is_positive() {
        return;
    }
    let x0 = (r.left() * ppp).round() as i32;
    let x1 = (r.right() * ppp).round() as i32;
    let stroke = (z * ppp * 0.6).round().max(1.0) as i32;
    if item.underline != Underline::None {
        let y = ((r.bottom() - 1.5 * z) * ppp).round() as i32;
        fill_rect_px(img, x0, y, x1, y + stroke, item.color);
    }
    if item.strike {
        let y = (r.center().y * ppp).round() as i32;
        fill_rect_px(img, x0, y, x1, y + stroke, item.color);
    }
}

fn paint_all_borders(
    img: &mut image::RgbaImage,
    wb: &Workbook,
    sheet_idx: usize,
    sh: &Sheet,
    layout: &RangeLayout,
) {
    let mut cf = CfCache::default();
    for (r_idx, &row) in layout.rows.iter().enumerate() {
        for (c_idx, &col) in layout.cols.iter().enumerate() {
            let c = CellRef::new(row, col);
            let rect = layout.cell_rect(r_idx, c_idx);
            let borders = look(wb, sheet_idx, sh, c, &mut cf).0.style.borders;
            draw_border_edge(img, wb, borders.top, [rect.left_top(), rect.right_top()], layout);
            draw_border_edge(img, wb, borders.bottom, [rect.left_bottom(), rect.right_bottom()], layout);
            draw_border_edge(img, wb, borders.left, [rect.left_top(), rect.left_bottom()], layout);
            draw_border_edge(img, wb, borders.right, [rect.right_top(), rect.right_bottom()], layout);
        }
    }
}

fn draw_border_edge(
    img: &mut image::RgbaImage,
    wb: &Workbook,
    line: BorderLine,
    pts: [Pos2; 2],
    layout: &RangeLayout,
) {
    if line.style == BorderStyle::None {
        return;
    }
    let color = col32(line.color, wb, Color32::BLACK);
    let scale = layout.z * layout.ppp;
    let half = (line.style.width() * scale * 0.5).round().max(1.0) as i32;
    let x0 = (pts[0].x.min(pts[1].x) * layout.ppp).round() as i32;
    let x1 = (pts[0].x.max(pts[1].x) * layout.ppp).round() as i32;
    let y0 = (pts[0].y.min(pts[1].y) * layout.ppp).round() as i32;
    let y1 = (pts[0].y.max(pts[1].y) * layout.ppp).round() as i32;
    if (y0 - y1).abs() <= 1 {
        fill_rect_px(img, x0 - half, y0 - half, x1 + half, y0 + half, color);
    } else {
        fill_rect_px(img, x0 - half, y0 - half, x0 + half, y1 + half, color);
    }
}

fn fill_rect_pt(img: &mut image::RgbaImage, rect: Rect, ppp: f32, color: Color32) {
    let x0 = (rect.min.x * ppp).round() as i32;
    let y0 = (rect.min.y * ppp).round() as i32;
    let x1 = (rect.max.x * ppp).round() as i32;
    let y1 = (rect.max.y * ppp).round() as i32;
    fill_rect_px(img, x0, y0, x1, y1, color);
}

fn fill_rect_px(img: &mut image::RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Color32) {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let sx = x0.clamp(0, w) as u32;
    let ex = x1.clamp(0, w) as u32;
    let sy = y0.clamp(0, h) as u32;
    let ey = y1.clamp(0, h) as u32;
    let rgba = image::Rgba([color.r(), color.g(), color.b(), 255]);
    for y in sy..ey {
        for x in sx..ex {
            img.put_pixel(x, y, rgba);
        }
    }
}

fn blend_glyph_pixel(img: &mut image::RgbaImage, x: i32, y: i32, texel: Color32, color: Color32) {
    if x < 0 || y < 0 || x >= img.width() as i32 || y >= img.height() as i32 {
        return;
    }
    let cov = u16::from(texel.a());
    if cov == 0 {
        return;
    }
    let alpha = (cov * u16::from(color.a()) + 127) / 255;
    if alpha == 0 {
        return;
    }
    let (ux, uy) = (x as u32, y as u32);
    let bg = img.get_pixel(ux, uy).0;
    let inv = 255 - alpha;
    let r = ((u16::from(color.r()) * alpha + u16::from(bg[0]) * inv + 127) / 255) as u8;
    let g = ((u16::from(color.g()) * alpha + u16::from(bg[1]) * inv + 127) / 255) as u8;
    let b = ((u16::from(color.b()) * alpha + u16::from(bg[2]) * inv + 127) / 255) as u8;
    img.put_pixel(ux, uy, image::Rgba([r, g, b, 255]));
}

#[cfg(test)]
mod tests {
    use super::SheetApp;
    use crate::Services;
    use gridcraft_engine::Session;
    use serde_json::json;

    #[test]
    fn single_cell_copy_does_not_render_png() {
        let mut session = Session::new();
        session.new_workbook();
        let _ = session.execute("cell.set", json!({"cell": "A1", "input": "Định Sơn"}));
        let _ = session.execute("selection.set", json!({"cell": "A1"}));
        let _ = session.execute("edit.copy", json!({"html": true}));
        let app = SheetApp::new(session, Services::default());
        let ctx = egui::Context::default();
        SheetApp::setup_context(&ctx, false);
        assert!(app.render_clipboard_png(&ctx).is_none());
    }

    #[test]
    fn multi_cell_copy_renders_valid_png_with_vietnamese_text() {
        let mut session = Session::new();
        session.new_workbook();
        let _ = session.execute(
            "range.setValues",
            json!({
                "range": "A1",
                "values": [
                    ["Hạng mục", "Khối lượng", "Thành tiền"],
                    ["Dọn dẹp mặt bằng nhân công", 125.5, 15000000]
                ]
            }),
        );
        let _ = session.execute("selection.set", json!({"range": "A1:C2"}));
        let _ = session.execute("edit.copy", json!({"html": true}));
        let app = SheetApp::new(session, Services::default());
        let ctx = egui::Context::default();
        SheetApp::setup_context(&ctx, false);
        let mut output = ctx.run_ui(Default::default(), |_| {});
        output.textures_delta.clear();
        let png = app.render_clipboard_png(&ctx);
        assert!(png.is_some(), "multi-cell range should render PNG bytes");
        let bytes = png.unwrap_or_default();
        assert!(bytes.starts_with(b"\x89PNG\r\n\x1a\n"), "valid PNG signature");
        let decoded = image::load_from_memory(&bytes).ok().map(|i| i.to_rgba8());
        assert!(decoded.is_some());
        let img = decoded.unwrap_or_default();
        assert!(img.width() > 100 && img.height() > 40);
        // Verify non-white ink pixels exist from gridlines and Vietnamese text glyphs.
        let has_ink = img.pixels().any(|p| p.0[0] < 200 || p.0[1] < 200 || p.0[2] < 200);
        assert!(has_ink, "rendered PNG must contain visible text/gridline pixels");
    }
}
