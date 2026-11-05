use super::{
    Align2, Backstage, Icon, Json, Outcome, Rect, Sense, SheetApp, Stroke, StrokeKind, Tokens, Ui, dim, json, pos2, square_button, theme, vec2,
};

/// Builds the print scope and fit-to-page parameters, caches `file.printPreview` per document revision/options, and draws page thumbnails with navigation plus the print settings and Print action.
pub(super) fn print(app: &mut SheetApp, ui: &mut Ui, bs: &mut Backstage, out: &mut Outcome) {
    let t = Tokens::get(ui.ctx());
    let lang = app.ui.language;
    let Some((uid, revision)) = app.session.active().map(|d| (d.uid, d.revision)) else {
        dim(ui, lang.tr("No workbook is open."));
        return;
    };
    let mut params = json!({"sheets": if bs.print_all { "all" } else { "active" }});
    if bs.fit_to_page {
        params["fitToPage"] = json!(true);
    }
    let key = (uid, revision, bs.print_all, bs.fit_to_page);
    if bs.preview.as_ref().is_none_or(|(k, _)| *k != key) {
        let preview = app.session.run("file.printPreview", params.clone()).unwrap_or(Json::Null);
        bs.preview = Some((key, preview));
    }
    let pages: Vec<Json> = bs.preview.as_ref().and_then(|(_, p)| p.get("pageList")).and_then(Json::as_array).cloned().unwrap_or_default();
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(250.0);
            if let Some(discover) = &app.services.printers {
                crate::printing::show(&mut app.printing, ui, discover, |label| lang.tr(label).to_owned());
            }
            let can_print = app.services.print_native.is_none() || app.printing.ready();
            if ui.add_enabled_ui(can_print, |ui| square_button(ui, Icon::Print, lang.tr("Print"))).inner.clicked()
                && app.commit_edit(0, 0, false, false)
            {
                app.print_now(params.clone());
                out.close = app.services.print_native.is_none();
            }
            ui.add_space(16.0);
            let scope = if bs.print_all { "Print Entire Workbook" } else { "Print Active Sheets" };
            craft_ui::ComboBox::from_id_salt("backstage_print_scope")
                .width(230.0)
                .selected_text(lang.tr(scope))
                .show_ui(ui, |ui| {
                    for (all, label) in [(false, "Print Active Sheets"), (true, "Print Entire Workbook")] {
                        if ui.selectable_label(bs.print_all == all, lang.tr(label)).clicked() {
                            bs.print_all = all;
                            bs.preview_page = 0;
                        }
                    }
                })
                .response
                .accessible_name(lang.tr("Print What"));
            ui.add_space(8.0);
            if ui.checkbox(&mut bs.fit_to_page, lang.tr("Fit Sheet on One Page")).changed() {
                bs.preview_page = 0;
            }
            ui.add_space(8.0);
            if ui.link(lang.tr("Page Setup…")).clicked() {
                app.open_dialog("pageSetup", json!({}));
            }
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            if pages.is_empty() {
                dim(ui, lang.tr("Nothing to print."));
                return;
            }
            let last = pages.len().saturating_sub(1);
            bs.preview_page = bs.preview_page.min(last);
            let Some(page) = pages.get(bs.preview_page) else { return };
            // Paper proportions from the preview (points); hostile sizes fall back to Letter.
            let w = page.get("width").and_then(Json::as_f64).filter(|w| w.is_finite() && *w > 1.0).unwrap_or(612.0) as f32;
            let h = page.get("height").and_then(Json::as_f64).filter(|h| h.is_finite() && *h > 1.0).unwrap_or(792.0) as f32;
            let scale = (520.0 / h).min(440.0 / w);
            let (rect, _) = ui.allocate_exact_size(vec2(w * scale, h * scale), Sense::hover());
            let p = ui.painter();
            p.rect_filled(rect.translate(vec2(2.0, 3.0)), 2.0, t.shadow);
            p.rect_filled(rect, 2.0, t.grid_bg);
            p.rect_stroke(rect, 2.0, Stroke::new(1.0, t.ribbon_border), StrokeKind::Inside);
            if let Some(d) = app.session.active() {
                paint_page(p, rect, w, page, &d.wb, &t);
            }
            let sheet = page.get("sheet").and_then(Json::as_str).unwrap_or("");
            let range = page.get("range").and_then(Json::as_str).unwrap_or("");
            ui.add_space(4.0);
            dim(ui, &format!("{sheet} · {range}"));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(bs.preview_page > 0, egui::Button::new("◀")).on_hover_text(lang.tr("Previous page")).clicked() {
                    bs.preview_page = bs.preview_page.saturating_sub(1);
                }
                let text = lang
                    .tr("Page {page} of {pages}")
                    .replace("{page}", &(bs.preview_page + 1).to_string())
                    .replace("{pages}", &pages.len().to_string());
                ui.label(text);
                if ui.add_enabled(bs.preview_page < last, egui::Button::new("▶")).on_hover_text(lang.tr("Next page")).clicked() {
                    bs.preview_page = (bs.preview_page + 1).min(last);
                }
            });
        });
    });
}

/// Most rows and columns one preview page draws (a page holds far fewer at any readable scale).
const PREVIEW_ROWS: u32 = 200;

const PREVIEW_COLS: u32 = 60;

/// Draws one printed page in miniature: the cells' displayed text, fills, borders and merges at
/// the page's print scale, inside the sheet's margins. `paper_w` is the page width in points.
fn paint_page(p: &egui::Painter, rect: Rect, paper_w: f32, page: &Json, wb: &gridcraft_engine::model::Workbook, t: &Tokens) {
    use gridcraft_engine::core::CellRef;
    use gridcraft_engine::model::{HAlign, PatternType, VAlign};

    let Some(range) = page.get("range").and_then(Json::as_str).and_then(gridcraft_engine::core::RangeRef::parse) else { return };
    let Some(sh) = page.get("sheet").and_then(Json::as_str).and_then(|n| wb.sheet_index(n)).and_then(|i| wb.sheets.get(i)) else { return };
    // Preview pixels per point, and per sheet pixel (sheet geometry is 96 dpi: 0.75 pt per pixel).
    let per_pt = rect.width() / paper_w;
    let print_scale =
        page.get("scale").and_then(Json::as_f64).filter(|s| s.is_finite() && *s > 0.0).map_or(1.0, |s| (s / 100.0) as f32).clamp(0.1, 4.0);
    let per_px = per_pt * 0.75 * print_scale;
    let margin =
        |i: usize, default: f32| sh.print.margins.get(i).copied().filter(|m| m.is_finite()).unwrap_or(default).clamp(0.0, 3.0) * 72.0 * per_pt;
    let body = Rect::from_min_max(
        pos2(rect.left() + margin(0, 0.7), rect.top() + margin(2, 0.75)),
        pos2(rect.right() - margin(1, 0.7), rect.bottom() - margin(3, 0.75)),
    );
    if !body.is_positive() {
        return;
    }
    let last_row = range.end.row.min(range.start.row.saturating_add(PREVIEW_ROWS));
    let last_col = range.end.col.min(range.start.col.saturating_add(PREVIEW_COLS));

    let mut col_x = Vec::with_capacity((last_col - range.start.col + 2) as usize);
    let mut cx = 0.0f32;
    for col in range.start.col..=last_col {
        col_x.push(cx);
        cx += sh.col_width(col) * per_px;
    }
    col_x.push(cx);

    let mut row_y = Vec::with_capacity((last_row - range.start.row + 2) as usize);
    let mut ry = 0.0f32;
    for row in range.start.row..=last_row {
        row_y.push(ry);
        ry += sh.row_height(row) * per_px;
    }
    row_y.push(ry);

    let ox = if sh.print.center_h && cx < body.width() { body.left() + (body.width() - cx) * 0.5 } else { body.left() };
    let oy = if sh.print.center_v && ry < body.height() { body.top() + (body.height() - ry) * 0.5 } else { body.top() };
    let cp = p.with_clip_rect(body);

    let cell_rect = |r0: u32, c0: u32, r1: u32, c1: u32| -> Rect {
        let ri0 = r0.saturating_sub(range.start.row) as usize;
        let ri1 = (r1.min(last_row) - range.start.row + 1) as usize;
        let ci0 = c0.saturating_sub(range.start.col) as usize;
        let ci1 = (c1.min(last_col) - range.start.col + 1) as usize;
        let x0 = ox + col_x.get(ci0).copied().unwrap_or(0.0);
        let x1 = ox + col_x.get(ci1).copied().unwrap_or(cx);
        let y0 = oy + row_y.get(ri0).copied().unwrap_or(0.0);
        let y1 = oy + row_y.get(ri1).copied().unwrap_or(ry);
        Rect::from_min_max(pos2(x0, y0), pos2(x1, y1))
    };

    if sh.print.gridlines {
        let gstroke = Stroke::new(0.5, t.gridline);
        let gx1 = (ox + cx).min(body.right());
        let gy1 = (oy + ry).min(body.bottom());
        for &y in &row_y {
            let py = oy + y;
            if py <= body.bottom() {
                cp.line_segment([pos2(ox, py), pos2(gx1, py)], gstroke);
            }
        }
        for &x in &col_x {
            let px = ox + x;
            if px <= body.right() {
                cp.line_segment([pos2(px, oy), pos2(px, gy1)], gstroke);
            }
        }
    }

    for row in range.start.row..=last_row {
        let ri = (row - range.start.row) as usize;
        let y0 = oy + row_y[ri];
        let h = row_y[ri + 1] - row_y[ri];
        if h <= 0.0 {
            continue;
        }
        if y0 >= body.bottom() {
            break;
        }
        for col in range.start.col..=last_col {
            let ci = (col - range.start.col) as usize;
            let x0 = ox + col_x[ci];
            let w = col_x[ci + 1] - col_x[ci];
            if w <= 0.0 {
                continue;
            }
            if x0 >= body.right() {
                break;
            }
            let pos = CellRef::new(row, col);
            let merge = sh.merge_at(pos);
            if let Some(m) = merge
                && m.start != pos
            {
                continue;
            }
            let draw_rect = match merge {
                Some(m) => cell_rect(m.start.row, m.start.col, m.end.row, m.end.col),
                None => Rect::from_min_size(pos2(x0, y0), vec2(w, h)),
            };

            let style = wb.styles.get(sh.style_id(pos));
            if style.fill.pattern != PatternType::None
                && let Some(c) = style.fill.fg.resolve(&wb.theme).or_else(|| style.fill.bg.resolve(&wb.theme))
            {
                let fill_c = theme::color32(c);
                if fill_c.a() > 0 {
                    cp.rect_filled(draw_rect.intersect(body), 0.0, fill_c);
                }
            }

            let text = gridcraft_engine::display::cell_text(wb, sh, pos);
            if !text.is_empty() {
                let val = sh.value(pos);
                let numeric = val.as_f64().is_some();
                let halign = match style.align.h {
                    HAlign::General => {
                        if numeric {
                            HAlign::Right
                        } else if matches!(val, gridcraft_engine::core::Value::Bool(_) | gridcraft_engine::core::Value::Error(_)) {
                            HAlign::Center
                        } else {
                            HAlign::Left
                        }
                    }
                    other => other,
                };
                let mut text_rect = draw_rect;
                if merge.is_none() && !numeric && !style.align.wrap && matches!(halign, HAlign::Left) {
                    let mut next = col.saturating_add(1);
                    while next <= last_col
                        && text_rect.right() < body.right()
                        && sh.value(CellRef::new(row, next)).is_empty()
                        && sh.merge_at(CellRef::new(row, next)).is_none()
                    {
                        text_rect.max.x += sh.col_width(next) * per_px;
                        next = next.saturating_add(1);
                    }
                }
                let fsize = (style.font.size * per_pt * print_scale).clamp(2.5, 16.0);
                let font_id = egui::FontId::new(fsize, theme::cell_family(&style.font.name, style.font.bold, style.font.italic));
                let fg = style.font.color.resolve(&wb.theme).map_or(t.cell_text, theme::color32);
                let clip = text_rect.intersect(body);
                if clip.is_positive() {
                    if style.align.wrap && text_rect.width() > 2.0 {
                        let galley = cp.layout(text, font_id, fg, (text_rect.width() - 2.0).max(1.0));
                        let gx = match halign {
                            HAlign::Center | HAlign::CenterAcross => text_rect.center().x - galley.size().x * 0.5,
                            HAlign::Right => text_rect.right() - 1.0 - galley.size().x,
                            _ => text_rect.left() + 1.0,
                        };
                        let gy = match style.align.v {
                            VAlign::Top => text_rect.top() + 0.5,
                            VAlign::Center => text_rect.center().y - galley.size().y * 0.5,
                            _ => (text_rect.bottom() - 0.5 - galley.size().y).max(text_rect.top()),
                        };
                        cp.with_clip_rect(clip).galley(pos2(gx, gy), galley, fg);
                    } else {
                        let (anchor, at) = match halign {
                            HAlign::Center | HAlign::CenterAcross => (Align2::CENTER_CENTER, text_rect.center()),
                            HAlign::Right => (Align2::RIGHT_CENTER, pos2(text_rect.right() - 1.0, text_rect.center().y)),
                            _ => (Align2::LEFT_CENTER, pos2(text_rect.left() + 1.0, text_rect.center().y)),
                        };
                        cp.with_clip_rect(clip).text(at, anchor, text, font_id, fg);
                    }
                }
            }

            let b = &wb.styles.get(sh.style_id(pos)).borders;
            let bstroke = |bl: &gridcraft_engine::model::BorderLine| -> Option<Stroke> {
                if bl.is_none() {
                    return None;
                }
                let c = bl.color.resolve(&wb.theme).map_or(egui::Color32::BLACK, theme::color32);
                Some(Stroke::new((bl.style.width() * per_pt * print_scale * 0.6).clamp(0.5, 2.0), c))
            };
            let x1 = x0 + w;
            let y1 = y0 + h;
            if let Some(s) = bstroke(&b.top) {
                cp.line_segment([pos2(x0, y0), pos2(x1, y0)], s);
            }
            if let Some(s) = bstroke(&b.bottom) {
                cp.line_segment([pos2(x0, y1), pos2(x1, y1)], s);
            }
            if let Some(s) = bstroke(&b.left) {
                cp.line_segment([pos2(x0, y0), pos2(x0, y1)], s);
            }
            if let Some(s) = bstroke(&b.right) {
                cp.line_segment([pos2(x1, y0), pos2(x1, y1)], s);
            }
        }
    }
}
